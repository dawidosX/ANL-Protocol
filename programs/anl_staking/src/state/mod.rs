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
    pub bump: u8,
    pub vault_authority_bump: u8,
    pub reserved: [u8; 56],
}

impl GlobalConfig {
    pub const LEN: usize = 8 + 1 + 32 * 3 + 1 + 8 + 8 + 1 + 1 + 56;
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
    pub bump: u8,
    pub reserved: [u8; 64],
}

impl PoolConfig {
    pub const LEN: usize = 8 + 1 + 1 + 1 + 2 + 8 + 8 + 16 + 8 + 8 + 1 + 64;

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
        let pending = self.pending_xnt(shares, debt_index)?;
        self.total_shares = self
            .total_shares
            .checked_sub(shares)
            .ok_or(anl_math::MathError::Overflow)?;
        Ok(pending)
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
    pub reserved: [u8; 32],
}

impl UserPosition {
    pub const LEN: usize =
        8 + 1 + 32 + 1 + 1 + 8 + 8 + 8 + 2 + 4 + 8 + 8 + 8 + 8 + 1 + 16 + 1 + 32;
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
