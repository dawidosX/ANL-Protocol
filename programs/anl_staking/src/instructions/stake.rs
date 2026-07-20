//! stake — otwarcie pozycji (WP v1.0 §5–7).
//!
//! Oba programy: okres deklaruje uczestnik (7..=3650 dni). APY przypisywane
//! w chwili otwarcia i niezmienne (Immutable APY, TC-049): Genesis — wg okna
//! wejścia (0–30 → 20%, 31–90 → 15%, od 91 → 8%), Flexible — zawsze 8%.
//! Nagroda ANL znana z góry i REZERWOWANA w Reward Vault (WP §11 — pokrycie);
//! principal księgowany jako actual received (Token-2022 transfer fee, §9).

use anchor_lang::prelude::*;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::errors::AnlError;
use crate::state::*;

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    /// CHECK: PDA-authority skarbcow (seeds + bump) - do constraints vaultow.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = owner,
        token::token_program = anl_token_program
    )]
    pub owner_anl: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [PRINCIPAL_VAULT_SEED], bump,
        token::mint = anl_mint, token::authority = vault_authority,
        token::token_program = anl_token_program)]
    pub principal_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    /// Pokrycie nagrody sprawdzane względem salda tego skarbca (WP §11).
    #[account(seeds = [REWARD_VAULT_SEED], bump,
        token::mint = anl_mint, token::authority = vault_authority,
        token::token_program = anl_token_program)]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        init_if_needed,
        payer = owner,
        space = UserProfile::LEN,
        seeds = [USER_PROFILE_SEED, owner.key().as_ref()],
        bump
    )]
    pub user_profile: Box<Account<'info, UserProfile>>,

    #[account(
        init,
        payer = owner,
        space = UserPosition::LEN,
        seeds = [
            USER_POSITION_SEED,
            owner.key().as_ref(),
            &user_profile.next_position_index.to_le_bytes()
        ],
        bump
    )]
    pub user_position: Box<Account<'info, UserPosition>>,

    pub anl_token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn stake_handler(ctx: Context<Stake>, amount: u64, declared_days: u32) -> Result<()> {
    let cfg = &ctx.accounts.global_config;
    let pool = &ctx.accounts.pool_config;
    require!(!cfg.paused, AnlError::Paused);
    require!(pool.status == PoolStatus::Active, AnlError::PoolPaused);
    require!(amount > 0, AnlError::ZeroAmount);
    require!(
        (MIN_PERIOD_DAYS..=MAX_PERIOD_DAYS).contains(&declared_days),
        AnlError::InvalidPeriod
    );

    let now = Clock::get()?.unix_timestamp;
    let elapsed = now
        .checked_sub(cfg.genesis_start_ts)
        .ok_or(AnlError::MathOverflow)?;
    // Przed publicznym startem nie ma stakingu (T0 okien, D-11).
    require!(elapsed >= 0, AnlError::NotStarted);

    // ---- transfer principalu: actual received (sekcja 9) ----
    let before = ctx.accounts.principal_vault.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.anl_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.owner_anl.to_account_info(),
                mint: ctx.accounts.anl_mint.to_account_info(),
                to: ctx.accounts.principal_vault.to_account_info(),
                authority: ctx.accounts.owner.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.anl_mint.decimals,
    )?;
    ctx.accounts.principal_vault.reload()?;
    let net = ctx
        .accounts
        .principal_vault
        .amount
        .checked_sub(before)
        .ok_or(AnlError::MathOverflow)?;
    require!(net >= MIN_STAKE_AMOUNT, AnlError::BelowMinimumStake);

    // ---- Immutable APY wg programu i okna wejścia (WP §5/§6) ----
    let apy_bps = match ctx.accounts.pool_config.pool_type {
        PoolType::Genesis => anl_math::genesis_apy_bps(elapsed).map_err(AnlError::from)?,
        PoolType::Flexible => anl_math::APY_FLEXIBLE_BPS,
    };

    let period_secs = (declared_days as i64)
        .checked_mul(anl_math::SECONDS_PER_DAY)
        .ok_or(AnlError::MathOverflow)?;
    let end_ts = anl_math::period_end_ts(now, period_secs).map_err(AnlError::from)?;
    let anl_reward = anl_math::period_reward(net, apy_bps, period_secs).map_err(AnlError::from)?;

    // ---- pokrycie nagrody w Reward Vault (WP §11) ----
    let cfg = &mut ctx.accounts.global_config;
    let new_reserved = cfg
        .anl_reward_reserved
        .checked_add(anl_reward)
        .ok_or(AnlError::MathOverflow)?;
    require!(
        ctx.accounts.reward_vault.amount >= new_reserved,
        AnlError::RewardCoverageExceeded
    );
    cfg.anl_reward_reserved = new_reserved;

    // ---- księgowanie puli i pozycji ----
    let pool = &mut ctx.accounts.pool_config;
    pool.total_staked = pool
        .total_staked
        .checked_add(net)
        .ok_or(AnlError::MathOverflow)?;
    pool.total_shares = pool
        .total_shares
        .checked_add(net)
        .ok_or(AnlError::MathOverflow)?;
    pool.position_count = pool
        .position_count
        .checked_add(1)
        .ok_or(AnlError::MathOverflow)?;

    let profile = &mut ctx.accounts.user_profile;
    if profile.owner == Pubkey::default() {
        profile.owner = ctx.accounts.owner.key();
        profile.bump = ctx.bumps.user_profile;
        profile.reserved = [0; 7];
    }
    let position_index = profile.next_position_index;
    profile.next_position_index = position_index
        .checked_add(1)
        .ok_or(AnlError::MathOverflow)?;

    let pos = &mut ctx.accounts.user_position;
    pos.version = ACCOUNT_VERSION;
    pos.owner = ctx.accounts.owner.key();
    pos.pool_type = pool.pool_type;
    pos.status = PositionStatus::Active;
    pos.position_index = position_index;
    pos.amount = net;
    pos.shares = net; // 1:1 (sekcja 6.1)
    pos.apy_bps = apy_bps;
    pos.declared_days = declared_days;
    pos.start_ts = now;
    pos.end_ts = end_ts;
    pos.anl_reward = anl_reward;
    pos.xnt_accrued = 0;
    pos.settled = false;
    pos.xnt_debt_index = pool.xnt_reward_index; // TC-121/124: zero historii
    pos.bump = ctx.bumps.user_position;
    pos.end_epoch = epoch_of(pos.end_ts.saturating_sub(1), cfg.genesis_start_ts)
        .ok_or(AnlError::BeforeGenesis)?;
    pos.reserved = [0; 24];

    emit!(PositionOpened {
        owner: pos.owner,
        pool_type: pos.pool_type as u8,
        position_index,
        amount_net: net,
        apy_bps,
        declared_days,
        start_ts: now,
        end_ts,
        anl_reward,
    });
    Ok(())
}

#[event]
pub struct PositionOpened {
    pub owner: Pubkey,
    pub pool_type: u8,
    pub position_index: u64,
    pub amount_net: u64,
    pub apy_bps: u16,
    pub declared_days: u32,
    pub start_ts: i64,
    pub end_ts: i64,
    pub anl_reward: u64,
}
