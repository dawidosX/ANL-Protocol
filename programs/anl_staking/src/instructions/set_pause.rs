//! pause / resume — TC-100…TC-105 (hamulec awaryjny, D-4/D-11).

use anchor_lang::prelude::*;

use crate::constants::GLOBAL_CONFIG_SEED;
use crate::errors::AnlError;
use crate::state::GlobalConfig;

#[derive(Accounts)]
pub struct SetPause<'info> {
    #[account(constraint = authority.key() == global_config.authority @ AnlError::InvalidAuthority)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,
}

pub fn pause(ctx: Context<SetPause>) -> Result<()> {
    let cfg = &mut ctx.accounts.global_config;
    require!(!cfg.paused, AnlError::AlreadyPaused); // TC-102: odrzucenie, nie idempotencja
    cfg.paused = true;
    emit!(PauseChanged { paused: true, timestamp: Clock::get()?.unix_timestamp });
    Ok(())
}

pub fn resume(ctx: Context<SetPause>) -> Result<()> {
    let cfg = &mut ctx.accounts.global_config;
    require!(cfg.paused, AnlError::NotPaused); // TC-105
    cfg.paused = false;
    emit!(PauseChanged { paused: false, timestamp: Clock::get()?.unix_timestamp });
    Ok(())
}

#[event]
pub struct PauseChanged {
    pub paused: bool,
    pub timestamp: i64,
}
