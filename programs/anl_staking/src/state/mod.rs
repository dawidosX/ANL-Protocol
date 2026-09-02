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
    /// Skumulowana suma XNT wpłacona przez fund_xnt (amount_net, po opłatach).
    /// Rośnie monotonicznie, nie maleje przy wypłatach — źródło prawdy dla
    /// metryki "XNT rozdane" bez skanowania historii transakcji.
    pub total_xnt_funded: u64,
    /// CAPY (v3) - Token-2022, trzecia waluta (rozdzielony claim).
    pub capy_mint: Pubkey,
    pub capy_vault_bump: u8,
    /// Suma wyplaconych nagrod ANL (monotoniczna, tylko udany claim).
    pub total_anl_paid: u64,
    /// Suma naliczonego, niewyplaconego CAPY (pending userow).
    pub capy_reserved: u64,
    pub reserved: [u8; 16],
}

impl GlobalConfig {
    pub const LEN: usize = 8 + 1 + 32 * 3 + 1 + 8 + 8 + 32 + 1 + 1 + 8 + 32 + 1 + 8 + 8 + 16;
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
    /// Wariant A (koszyk dobowy): XNT skumulowany w BIEŻĄCEJ dobie, jeszcze
    /// nierozdzielony. close_day() dzieli go wg total_shares na KONIEC doby.
    pub current_day_basket: u64,
    /// Doba (epoka), do której należy current_day_basket.
    pub current_day: u64,
    pub reserved: [u8; 32],
}

impl PoolConfig {
    pub const LEN: usize = 8 + 1 + 1 + 1 + 2 + 8 + 8 + 16 + 8 + 8 + 8 + 8 + 1 + 8 + 8 + 32;

    /// Dzienny funding części tej puli (WP v1.0 §8). Przy `total_shares == 0`
    /// środki czekają w `xnt_undistributed` — zasada pustego koszyka.
    /// Wariant A — ZAMKNIĘCIE DOBY (leniwe). Rozdziela koszyk bieżącej doby
    /// wg total_shares NA KONIEC doby: index rośnie o basket/total_shares.
    /// Pusta pula → koszyk do bufora (nie ginie).
    pub fn close_day(&mut self) -> std::result::Result<(), anl_math::MathError> {
        if self.current_day_basket == 0 {
            return Ok(());
        }
        let basket = self.current_day_basket;
        if self.total_shares == 0 {
            self.xnt_undistributed = self
                .xnt_undistributed
                .checked_add(basket)
                .ok_or(anl_math::MathError::Overflow)?;
            self.current_day_basket = 0;
            return Ok(());
        }
        let part_total = self
            .xnt_undistributed
            .checked_add(basket)
            .ok_or(anl_math::MathError::Overflow)?;
        self.xnt_reward_index =
            anl_math::update_xnt_index(self.xnt_reward_index, part_total, self.total_shares)?;
        self.xnt_undistributed = 0;
        self.current_day_basket = 0;
        Ok(())
    }

    /// Wariant A — dodaje XNT do koszyka BIEŻĄCEJ doby. Nowa doba → najpierw
    /// zamyka poprzednią (rozdziela jej koszyk). NIE rusza index natychmiast.
    pub fn add_to_basket(
        &mut self,
        part: u64,
        epoch: u64,
    ) -> std::result::Result<(), anl_math::MathError> {
        if self.current_day != epoch {
            self.close_day()?;
            self.current_day = epoch;
        }
        self.current_day_basket = self
            .current_day_basket
            .checked_add(part)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(())
    }

    /// Wariant A — wymusza zamknięcie doby, jeśli epoch jest nowszy niż doba
    /// koszyka. Wołane przez stake/settle PRZED zmianą total_shares.
    /// Wariant A (B) — zwraca Some(domknieta_doba), gdy faktycznie domknela
    /// koszyk. Wolajacy MUSI wtedy zapisac finalny index do checkpointu tej doby.
    pub fn roll_day_if_needed(
        &mut self,
        epoch: u64,
    ) -> std::result::Result<Option<u64>, anl_math::MathError> {
        if self.current_day != epoch && self.current_day_basket > 0 {
            let closed = self.current_day;
            self.close_day()?;
            self.current_day = epoch;
            return Ok(Some(closed));
        }
        Ok(None)
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
    /// Zamyka pozycję na `cap_index` (ograniczony do jej end_epoch). Zwraca
    /// `pending` (XNT należny pozycji). Osierocony udział — to, co pula naliczyła
    /// tej pozycji ZA EPOKI PO jej end_epoch (bo index puli urósł wyżej niż cap
    /// po fundingach, których pozycja nie dożyła) — wraca do `xnt_undistributed`,
    /// żeby przy następnym fundingu rozdał się żywym. Bez tego osiadałby w skarbcu
    /// jako niewypłacalny nadmiar.
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
        let orphan_delta = self.xnt_reward_index.saturating_sub(cap_index);
        let orphan_u128 = orphan_delta
            .checked_mul(shares as u128)
            .ok_or(anl_math::MathError::Overflow)?
            / anl_math::PRECISION;
        let orphan: u64 = orphan_u128
            .try_into()
            .map_err(|_| anl_math::MathError::Overflow)?;
        self.xnt_undistributed = self.xnt_undistributed.saturating_add(orphan);
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
    pub const LEN: usize =
        8 + 1 + 32 + 1 + 1 + 8 + 8 + 8 + 2 + 4 + 8 + 8 + 8 + 8 + 1 + 16 + 1 + 8 + 8 + 8 + 8;
}

#[account]
pub struct UserProfile {
    pub owner: Pubkey,
    pub next_position_index: u64,
    pub bump: u8,
    /// CAPY (v3): naliczone, nieodebrane CAPY. Przezywa close pozycji.
    pub pending_capy: u64,
    pub reserved: [u8; 7],
}

impl UserProfile {
    pub const LEN: usize = 8 + 32 + 8 + 1 + 8 + 7;
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

#[cfg(test)]
mod wariant_a_tests {
    use super::*;
    use crate::constants::NO_EPOCH;

    fn empty_pool(pool_type: PoolType) -> PoolConfig {
        PoolConfig {
            version: ACCOUNT_VERSION,
            pool_type,
            status: PoolStatus::Active,
            xnt_share_bps: 6500,
            total_staked: 0,
            total_shares: 0,
            xnt_reward_index: 0,
            xnt_undistributed: 0,
            position_count: 0,
            last_funded_epoch: NO_EPOCH,
            first_funded_epoch: NO_EPOCH,
            bump: 0,
            current_day_basket: 0,
            current_day: 0,
            reserved: [0; 32],
        }
    }

    fn pending(pool: &PoolConfig, shares: u64, debt: u128) -> u64 {
        ((pool.xnt_reward_index - debt) * (shares as u128) / anl_math::PRECISION) as u64
    }

    #[test]
    fn test_podzial_wg_finalnych_shares() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 1_000_000;
        let debt1: u128 = pool.xnt_reward_index;
        pool.current_day = 0;
        pool.add_to_basket(10_000_000_000, 0).unwrap();
        let debt2: u128 = pool.xnt_reward_index;
        pool.total_shares += 1_000_000;
        pool.roll_day_if_needed(1).unwrap();
        let xnt1 = pending(&pool, 1_000_000, debt1);
        let xnt2 = pending(&pool, 1_000_000, debt2);
        assert_eq!(xnt1, 5_000_000_000, "#1 powinien dostac 5 XNT");
        assert_eq!(xnt2, 5_000_000_000, "#2 powinien dostac 5 XNT");
    }

    #[test]
    fn test_kumulacja_przez_dwie_doby() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 1_000_000;
        let debt: u128 = pool.xnt_reward_index;
        pool.current_day = 0;
        pool.add_to_basket(6_000_000_000, 0).unwrap();
        pool.add_to_basket(4_000_000_000, 1).unwrap();
        pool.roll_day_if_needed(2).unwrap();
        let xnt = pending(&pool, 1_000_000, debt);
        assert_eq!(xnt, 10_000_000_000, "pozycja przez 2 doby dostaje 10 XNT");
    }

    #[test]
    fn test_pusta_pula_koszyk_do_bufora() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.current_day = 0;
        pool.add_to_basket(10_000_000_000, 0).unwrap();
        pool.roll_day_if_needed(1).unwrap();
        assert_eq!(
            pool.xnt_undistributed, 10_000_000_000,
            "pusta pula -> bufor"
        );
        assert_eq!(
            pool.xnt_reward_index, 0,
            "index nie rosnie przy pustej puli"
        );
        let debt: u128 = pool.xnt_reward_index;
        pool.total_shares = 1_000_000;
        pool.add_to_basket(5_000_000_000, 1).unwrap();
        pool.roll_day_if_needed(2).unwrap();
        let xnt = pending(&pool, 1_000_000, debt);
        assert_eq!(
            xnt, 15_000_000_000,
            "staker dostaje bufor 10 + koszyk 5 = 15 XNT"
        );
    }

    #[test]
    fn test_pozny_staker_nie_lapie_zamknietej_doby() {
        let mut pool = empty_pool(PoolType::Genesis);
        pool.total_shares = 1_000_000;
        let debt1: u128 = pool.xnt_reward_index;
        pool.current_day = 0;
        pool.add_to_basket(10_000_000_000, 0).unwrap();
        pool.roll_day_if_needed(1).unwrap();
        let debt2: u128 = pool.xnt_reward_index;
        pool.total_shares += 1_000_000;
        let xnt1 = pending(&pool, 1_000_000, debt1);
        let xnt2 = pending(&pool, 1_000_000, debt2);
        assert_eq!(
            xnt1, 10_000_000_000,
            "#1 dostaje cale 10 (byl sam w dobie 0)"
        );
        assert_eq!(xnt2, 0, "#2 nie lapie doby 0 (wszedl w dobie 1)");
    }
}
