//! Konta programu (spec v1.0, sekcja 3). Wersjonowane pod migracje (10F §12/§21).

use anchor_lang::prelude::*;

pub const ACCOUNT_VERSION: u8 = 1;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoolType {
    Flexible = 0,
    Genesis = 1,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PoolStatus {
    Active = 0,
    Paused = 1,
    Closed = 2,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PositionStatus {
    Active = 0,
    Closed = 1,
}

#[account]
pub struct GlobalConfig {
    pub version: u8,
    /// Multisig administracyjny.
    pub authority: Pubkey,
    /// ANL — Token-2022 (D-14).
    pub anl_mint: Pubkey,
    /// XNT — wrapped native X1, legacy SPL Token (D-14).
    pub xnt_mint: Pubkey,
    pub paused: bool,
    /// T0 okien APY; przy controlled rollout = planowany go-live (D-11, sekcja 7).
    pub genesis_start_ts: i64,
    /// Suma nagród ANL zarezerwowanych dla otwartych pozycji (WP §11:
    /// pokrycie w Reward Vault). Stake rezerwuje, claim/zerwanie zwalnia.
    pub anl_reward_reserved: u64,
    /// Operator (gorący klucz bota dziennego) — uprawniony WYŁĄCZNIE do
    /// fund_rewards / fund_xnt. Ustawiany przez authority (set_operator).
    /// Kompromitacja operatora nie zagraża środkom: może tylko wpłacać.
    pub operator: Pubkey,
    pub bump: u8,
    pub vault_authority_bump: u8,
    pub reserved: [u8; 24],
}

impl GlobalConfig {
    pub const LEN: usize = 8 + 1 + 32 * 3 + 1 + 8 + 8 + 32 + 1 + 1 + 24;
}

#[account]
pub struct PoolConfig {
    pub version: u8,
    pub pool_type: PoolType,
    pub status: PoolStatus,
    /// 6500 (Genesis) lub 3500 (Flexible).
    pub xnt_share_bps: u16,
    /// TVL puli — wartości NETTO po ewentualnych opłatach tokenowych.
    pub total_staked: u64,
    /// shares == total_staked (1:1, sekcja 6.1).
    pub total_shares: u64,
    /// Skumulowany indeks XNT × PRECISION.
    pub xnt_reward_index: u128,
    /// XNT przydzielone puli, gdy total_shares == 0 (D-5).
    pub xnt_undistributed: u64,
    pub position_count: u64,
    /// Epoka OSTATNIEGO fundingu tej puli (NO_EPOCH = nigdy). Audyt #2:
    /// indeks zmienia się wyłącznie w fund_xnt, więc snapshot końca każdej
    /// epoki E równa się indeksowi po ostatnim fundingu o epoce ≤ E.
    pub last_funded_epoch: u64,
    /// Epoka PIERWSZEGO fundingu (NO_EPOCH = nigdy) — dowód "zero fundingu
    /// ≤ end_epoch" bez konta checkpointu.
    pub first_funded_epoch: u64,
    pub bump: u8,
    pub reserved: [u8; 48],
}

impl PoolConfig {
    pub const LEN: usize = 8 + 1 + 1 + 1 + 2 + 8 + 8 + 16 + 8 + 8 + 8 + 8 + 1 + 48;

    // ----- silnik indeksu XNT — lustro `anl_core::XntPool` (10F §29) -----

    /// Dzienny funding części tej puli (WP v1.0 §8). Przy `total_shares == 0`
    /// środki czekają w `xnt_undistributed` — zasada pustego koszyka.
    pub fn fund_xnt_part(&mut self, part: u64) -> std::result::Result<(), anl_math::MathError> {
        let part_total = self
            .xnt_undistributed
            .checked_add(part)
            .ok_or(anl_math::MathError::Overflow)?;
        if self.total_shares == 0 {
            self.xnt_undistributed = part_total;
            return Ok(());
        }
        self.xnt_reward_index =
            anl_math::update_xnt_index(self.xnt_reward_index, part_total, self.total_shares)?;
        self.xnt_undistributed = 0;
        Ok(())
    }

    /// XNT należne pozycji przy bieżącym indeksie.
    pub fn pending_xnt(
        &self,
        shares: u64,
        debt_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        anl_math::pending_xnt(shares, self.xnt_reward_index, debt_index)
    }

    /// Settle po końcu okresu (WP §8): zamraża należność, zdejmuje shares —
    /// pozycja przestaje uczestniczyć w dziennej dystrybucji.
    pub fn settle_position(
        &mut self,
        shares: u64,
        debt_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        let idx = self.xnt_reward_index;
        self.settle_position_at(shares, debt_index, idx)
    }

    /// Settlement względem HISTORYCZNEGO indeksu (checkpoint końca
    /// end_epoch) — fundamentalna poprawka audytu #1: funding po
    /// end_epoch nie może zwiększyć wypłaty pozycji.
    pub fn settle_position_at(
        &mut self,
        shares: u64,
        debt_index: u128,
        cap_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        let delta = cap_index
            .checked_sub(debt_index)
            .ok_or(anl_math::MathError::Overflow)?;
        let pending_u128 = delta
            .checked_mul(shares as u128)
            .ok_or(anl_math::MathError::Overflow)?
            / anl_math::PRECISION;
        let pending: u64 = pending_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)?;
        self.total_shares = self
            .total_shares
            .checked_sub(shares)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(pending)
    }

    /// Genesis okna (WP okna 30-dniowe): policz XNT naliczony do `cap_index`
    /// BEZ zdejmowania shares — pozycja żyje dalej i nalicza w kolejnych oknach.
    /// Zwraca skumulowaną należność do progu (nie „do teraz"). Kwota do wypłaty
    /// w danym oknie = ta wartość MINUS `xnt_window_claimed` pozycji.
    /// `&self` — nic nie mutuje (w przeciwieństwie do settle_position_at).
    pub fn accrued_to_cap(
        &self,
        shares: u64,
        debt_index: u128,
        cap_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        let delta = cap_index
            .checked_sub(debt_index)
            .ok_or(anl_math::MathError::Overflow)?;
        let acc_u128 = delta
            .checked_mul(shares as u128)
            .ok_or(anl_math::MathError::Overflow)?
            / anl_math::PRECISION;
        acc_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)
    }

    /// Wcześniejsze zerwanie (WP §7): naliczone XNT wracają do puli
    /// dystrybucji koszyka (`xnt_undistributed`), shares schodzą.
    pub fn forfeit_position(
        &mut self,
        shares: u64,
        debt_index: u128,
    ) -> std::result::Result<u64, anl_math::MathError> {
        let pending = self.settle_position(shares, debt_index)?;
        self.xnt_undistributed = self
            .xnt_undistributed
            .checked_add(pending)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(pending)
    }
}

#[account]
pub struct UserPosition {
    pub version: u8,
    pub owner: Pubkey,
    pub pool_type: PoolType,
    pub status: PositionStatus,
    pub position_index: u64,
    /// Principal NETTO (actual received — sekcja 9).
    pub amount: u64,
    pub shares: u64,
    /// Immutable APY (TC-049) — przypisane w chwili otwarcia, na zawsze.
    pub apy_bps: u16,
    /// Okres deklarowany przez uczestnika — OBA programy (WP v1.0 §7).
    pub declared_days: u32,
    pub start_ts: i64,
    /// Koniec zadeklarowanego okresu; po tej chwili naliczanie stoi.
    pub end_ts: i64,
    /// Nagroda ANL pozycji, wyliczona i zarezerwowana przy otwarciu
    /// (Immutable APY ⇒ kwota znana z góry; WP §7).
    pub anl_reward: u64,
    /// XNT zamrożone przy settle po końcu okresu (WP §8).
    pub xnt_accrued: u64,
    /// Pozycja rozliczona z koszyka XNT (shares zdjęte po end_ts).
    pub settled: bool,
    /// Snapshot xnt_reward_index z chwili wejścia.
    pub xnt_debt_index: u128,
    pub bump: u8,
    /// Epoka XNT zawierająca ostatnią naliczaną sekundę pozycji
    /// (epoch_of(end_ts - 1)). Settlement używa checkpointu ≤ end_epoch.
    pub end_epoch: u64,
    /// Genesis: skumulowana suma XNT już wypłacona w oknach 30-dniowych.
    /// Zapewnia kumulację i chroni przed podwójną wypłatą (WP okna Genesis).
    pub xnt_window_claimed: u64,
    /// Genesis: timestamp ostatniej wypłaty okienkowej (0 = nigdy).
    pub last_window_ts: i64,
    pub reserved: [u8; 8],
}

impl UserPosition {
    // ...end_epoch(8) + xnt_window_claimed(8) + last_window_ts(8) + reserved(8) = 32
    pub const LEN: usize =
        8 + 1 + 32 + 1 + 1 + 8 + 8 + 8 + 2 + 4 + 8 + 8 + 8 + 8 + 1 + 16 + 1 + 8 + 8 + 8 + 8;
}

#[account]
pub struct UserProfile {
    pub owner: Pubkey,
    pub next_position_index: u64,
    pub bump: u8,
    pub reserved: [u8; 7],
}

impl UserProfile {
    pub const LEN: usize = 8 + 32 + 8 + 1 + 7;
}

/// Snapshot indeksu puli po epoce, w której wystąpił funding.
/// PDA: [XNT_CKPT_SEED, pool_type, epoch.to_le_bytes()].
/// `next_funded_epoch` (NO_EPOCH = brak) tworzy łańcuch dowodowy:
/// checkpoint K jest ostatnim fundingiem ≤ E ⟺ K.epoch ≤ E oraz
/// (K.next == NO_EPOCH ∨ K.next > E).
#[account]
pub struct XntCheckpoint {
    pub version: u8,
    pub pool_type: PoolType,
    pub epoch: u64,
    /// xnt_reward_index puli po WSZYSTKICH fundingach tej epoki.
    pub index: u128,
    pub next_funded_epoch: u64,
    pub bump: u8,
    pub reserved: [u8; 13],
}

impl XntCheckpoint {
    pub const LEN: usize = 8 + 1 + 1 + 8 + 16 + 8 + 1 + 13;
}

/// Numer epoki XNT dla chwili `ts` względem genesis (epoka = 1 dzień,
/// granice zsynchronizowane z oknami Genesis o 02:00 UTC).
pub fn epoch_of(ts: i64, genesis_start_ts: i64) -> Option<u64> {
    if ts < genesis_start_ts {
        return None;
    }
    Some(((ts - genesis_start_ts) as u64) / (anl_math::SECONDS_PER_DAY as u64))
}
