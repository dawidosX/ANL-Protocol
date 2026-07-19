//! create_pool — TC-010…TC-016. Dokładnie dwie pule (PDA per typ).

use anchor_lang::prelude::*;

use crate::constants::*;
use crate::errors::AnlError;
use crate::state::*;

#[derive(Accounts)]
#[instruction(pool_type: PoolType)]
pub struct CreatePool<'info> {
    #[account(mut, constraint = authority.key() == global_config.authority @ AnlError::InvalidAuthority)]
    pub authority: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        init,
        payer = authority,
        space = PoolConfig::LEN,
        seeds = [POOL_SEED, &[pool_type as u8]],
        bump
    )]
    pub pool_config: Account<'info, PoolConfig>,

    pub system_program: Program<'info, System>,
}

pub fn create_pool_handler(ctx: Context<CreatePool>, pool_type: PoolType) -> Result<()> {
    let pool = &mut ctx.accounts.pool_config;
    pool.version = ACCOUNT_VERSION;
    pool.pool_type = pool_type;
    pool.status = PoolStatus::Active;
    pool.xnt_share_bps = match pool_type {
        PoolType::Genesis => XNT_SHARE_GENESIS_BPS,
        PoolType::Flexible => XNT_SHARE_FLEXIBLE_BPS,
    };
    pool.total_staked = 0;
    pool.total_shares = 0;
    pool.xnt_reward_index = 0;
    pool.xnt_undistributed = 0;
    pool.position_count = 0;
    pool.bump = ctx.bumps.pool_config;
    pool.last_funded_epoch = NO_EPOCH;
    pool.first_funded_epoch = NO_EPOCH;
    pool.reserved = [0; 48];

    emit!(PoolCreated {
        pool_type: pool_type as u8,
        xnt_share_bps: pool.xnt_share_bps,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

#[event]
pub struct PoolCreated {
    pub pool_type: u8,
    pub xnt_share_bps: u16,
    pub timestamp: i64,
}
