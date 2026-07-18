//! ANL Staking Protocol — program on-chain (X1 Network).
//! Faza 1: initialize + create_pool + pause/resume.
//! Faza 2 (WP v1.0): stake + fund_rewards/fund_xnt + settle_expired + claim + unstake_early.

use anchor_lang::prelude::*;

pub mod constants;
pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;
use state::PoolType;

// Placeholder — właściwy Program ID wygenerowany przy `anchor keys sync`.
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod anl_staking {
    use super::*;

    /// TC-001…006. `genesis_start_ts` = planowany go-live (pełna godzina, D-2/D-11);
    /// `start_paused = true` na Mainnet (controlled rollout, 10F §61).
    pub fn initialize(
        ctx: Context<Initialize>,
        genesis_start_ts: i64,
        start_paused: bool,
    ) -> Result<()> {
        instructions::initialize::handler(ctx, genesis_start_ts, start_paused)
    }

    /// TC-010…016. Dokładnie jedna pula per typ (PDA wyklucza duplikaty).
    pub fn create_pool(ctx: Context<CreatePool>, pool_type: PoolType) -> Result<()> {
        instructions::create_pool::handler(ctx, pool_type)
    }

    /// TC-100/101/102.
    pub fn pause(ctx: Context<SetPause>) -> Result<()> {
        instructions::set_pause::pause(ctx)
    }

    /// TC-103/104/105.
    pub fn resume(ctx: Context<SetPause>) -> Result<()> {
        instructions::set_pause::resume(ctx)
    }

    /// WP §5–7: otwarcie pozycji — Immutable APY, okres 7..=3650 dni,
    /// rezerwacja nagrody ANL (pokrycie w Reward Vault).
    pub fn stake(ctx: Context<Stake>, amount: u64, declared_days: u32) -> Result<()> {
        instructions::stake::handler(ctx, amount, declared_days)
    }

    /// Depozyt puli nagród ANL (rezerwuar 200M).
    pub fn fund_rewards(ctx: Context<FundRewards>, amount: u64) -> Result<()> {
        instructions::fund::fund_rewards(ctx, amount)
    }

    /// WP §8: dzienny wpływ XNT z walidatora; podział 65/35 do indeksów koszyków.
    pub fn fund_xnt(ctx: Context<FundXnt>, amount: u64) -> Result<()> {
        instructions::fund::fund_xnt(ctx, amount)
    }

    /// WP §8 (permissionless): po end_ts zamraża XNT i zdejmuje shares z koszyka.
    pub fn settle_expired(ctx: Context<SettleExpired>) -> Result<()> {
        instructions::lifecycle::settle_expired(ctx)
    }

    /// WP §7: po end_ts — ANL + XNT + principal w jednej transakcji, pozycja zamknięta.
    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        instructions::lifecycle::claim(ctx)
    }

    /// WP §7: przed end_ts — principal wraca w całości, całość nagród przepada.
    pub fn unstake_early(ctx: Context<UnstakeEarly>) -> Result<()> {
        instructions::lifecycle::unstake_early(ctx)
    }
}
