//! settle_expired / claim / unstake_early — cykl życia pozycji (WP v1.0 §7).
//!
//! `settle_expired` (PERMISSIONLESS): po `end_ts` zamraża XNT pozycji i zdejmuje
//! shares z koszyka — pozycja przestaje uczestniczyć w dziennej dystrybucji.
//! Wywoływane przez bota operacyjnego PRZED dziennym `fund_xnt` (README §Ops).
//!
//! `claim`: po `end_ts` — jedna transakcja wypłaca nagrodę ANL + naliczone XNT
//! + principal i zamyka pozycję (guard ClaimFirst z natury konstrukcji).
//!
//! `unstake_early`: przed `end_ts` — principal wraca w całości; CAŁOŚĆ nagród
//! przepada: rezerwacja ANL zwolniona (tokeny nigdy nie opuściły Reward Vault),
//! naliczone XNT wracają do puli dystrybucji koszyka (WP §7).

use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::errors::AnlError;
use crate::state::*;

// ------------------------------------------------------------ settle_expired

#[derive(Accounts)]
pub struct SettleExpired<'info> {
    /// Permissionless — settle może wykonać każdy (bot operacyjny, sam user).
    pub cranker: Signer<'info>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [
            USER_POSITION_SEED,
            user_position.owner.as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Account<'info, UserPosition>,
}

pub fn settle_expired(ctx: Context<SettleExpired>) -> Result<()> {
    let pos = &mut ctx.accounts.user_position;
    require!(pos.status == PositionStatus::Active, AnlError::PositionClosed);
    require!(!pos.settled, AnlError::AlreadySettled);
    let now = Clock::get()?.unix_timestamp;
    require!(now >= pos.end_ts, AnlError::PeriodNotEnded);

    let frozen = ctx
        .accounts
        .pool_config
        .settle_position(pos.shares, pos.xnt_debt_index)
        .map_err(AnlError::from)?;
    pos.xnt_accrued = frozen;
    pos.settled = true;

    emit!(PositionSettled {
        owner: pos.owner,
        position_index: pos.position_index,
        xnt_accrued: frozen,
        timestamp: now,
    });
    Ok(())
}

#[event]
pub struct PositionSettled {
    pub owner: Pubkey,
    pub position_index: u64,
    pub xnt_accrued: u64,
    pub timestamp: i64,
}

// ------------------------------------------------------------ claim

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut, constraint = owner.key() == user_position.owner @ AnlError::PositionOwnerMismatch)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        close = owner,
        seeds = [
            USER_POSITION_SEED,
            owner.key().as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Account<'info, UserPosition>,

    /// CHECK: PDA-authority skarbców (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: InterfaceAccount<'info, Mint>,

    #[account(address = global_config.xnt_mint @ AnlError::InvalidMint)]
    pub xnt_mint: InterfaceAccount<'info, Mint>,

    #[account(mut, seeds = [PRINCIPAL_VAULT_SEED], bump)]
    pub principal_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [REWARD_VAULT_SEED], bump)]
    pub reward_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [XNT_VAULT_SEED], bump)]
    pub xnt_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = owner,
        token::token_program = anl_token_program
    )]
    pub owner_anl: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = xnt_mint,
        token::authority = owner,
        token::token_program = xnt_token_program
    )]
    pub owner_xnt: InterfaceAccount<'info, TokenAccount>,

    pub anl_token_program: Program<'info, Token2022>,
    pub xnt_token_program: Program<'info, Token>,
}

pub fn claim(ctx: Context<Claim>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.user_position.status == PositionStatus::Active,
        AnlError::PositionClosed
    );
    // WP §7: nagrody wymagalne wyłącznie po dotrzymaniu okresu (granica inkluzywna).
    require!(
        now >= ctx.accounts.user_position.end_ts,
        AnlError::PeriodNotEnded
    );

    // Inline-settle, jeśli bot nie zdążył (WP §8: shares schodzą z koszyka).
    if !ctx.accounts.user_position.settled {
        let (shares, debt) = (
            ctx.accounts.user_position.shares,
            ctx.accounts.user_position.xnt_debt_index,
        );
        let frozen = ctx
            .accounts
            .pool_config
            .settle_position(shares, debt)
            .map_err(AnlError::from)?;
        let pos = &mut ctx.accounts.user_position;
        pos.xnt_accrued = frozen;
        pos.settled = true;
    }

    let amount = ctx.accounts.user_position.amount;
    let anl_reward = ctx.accounts.user_position.anl_reward;
    let xnt_accrued = ctx.accounts.user_position.xnt_accrued;

    require!(
        ctx.accounts.reward_vault.amount >= anl_reward,
        AnlError::InsufficientRewardVault
    );
    require!(
        ctx.accounts.xnt_vault.amount >= xnt_accrued,
        AnlError::InsufficientXntVault
    );

    let bump = ctx.accounts.global_config.vault_authority_bump;
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[bump]];
    let signer: &[&[&[u8]]] = &[seeds];

    // 1) nagroda ANL z Reward Vault (I: nigdy z Principal Vault)
    if anl_reward > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.anl_token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.reward_vault.to_account_info(),
                    mint: ctx.accounts.anl_mint.to_account_info(),
                    to: ctx.accounts.owner_anl.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer,
            ),
            anl_reward,
            ctx.accounts.anl_mint.decimals,
        )?;
    }
    // 2) naliczone XNT z XNT Vault (WP §8: razem z ANL, w dniu zakończenia)
    if xnt_accrued > 0 {
        token_interface::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.xnt_token_program.to_account_info(),
                TransferChecked {
                    from: ctx.accounts.xnt_vault.to_account_info(),
                    mint: ctx.accounts.xnt_mint.to_account_info(),
                    to: ctx.accounts.owner_xnt.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer,
            ),
            xnt_accrued,
            ctx.accounts.xnt_mint.decimals,
        )?;
    }
    // 3) principal z Principal Vault (I: nigdy ze skarbca nagród)
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.anl_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.principal_vault.to_account_info(),
                mint: ctx.accounts.anl_mint.to_account_info(),
                to: ctx.accounts.owner_anl.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.anl_mint.decimals,
    )?;

    // ---- księgowanie ----
    let cfg = &mut ctx.accounts.global_config;
    cfg.anl_reward_reserved = cfg
        .anl_reward_reserved
        .checked_sub(anl_reward)
        .ok_or(AnlError::MathOverflow)?;
    let pool = &mut ctx.accounts.pool_config;
    pool.total_staked = pool
        .total_staked
        .checked_sub(amount)
        .ok_or(AnlError::MathOverflow)?;
    pool.position_count = pool
        .position_count
        .checked_sub(1)
        .ok_or(AnlError::MathOverflow)?;
    ctx.accounts.user_position.status = PositionStatus::Closed;

    emit!(PositionClaimed {
        owner: ctx.accounts.user_position.owner,
        position_index: ctx.accounts.user_position.position_index,
        principal: amount,
        anl_reward,
        xnt_reward: xnt_accrued,
        timestamp: now,
    });
    Ok(())
}

#[event]
pub struct PositionClaimed {
    pub owner: Pubkey,
    pub position_index: u64,
    pub principal: u64,
    pub anl_reward: u64,
    pub xnt_reward: u64,
    pub timestamp: i64,
}

// ------------------------------------------------------------ unstake_early

#[derive(Accounts)]
pub struct UnstakeEarly<'info> {
    #[account(mut, constraint = owner.key() == user_position.owner @ AnlError::PositionOwnerMismatch)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault
    )]
    pub pool_config: Account<'info, PoolConfig>,

    #[account(
        mut,
        close = owner,
        seeds = [
            USER_POSITION_SEED,
            owner.key().as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Account<'info, UserPosition>,

    /// CHECK: PDA-authority skarbców (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: InterfaceAccount<'info, Mint>,

    #[account(mut, seeds = [PRINCIPAL_VAULT_SEED], bump)]
    pub principal_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = owner,
        token::token_program = anl_token_program
    )]
    pub owner_anl: InterfaceAccount<'info, TokenAccount>,

    pub anl_token_program: Program<'info, Token2022>,
}

pub fn unstake_early(ctx: Context<UnstakeEarly>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.user_position.status == PositionStatus::Active,
        AnlError::PositionClosed
    );
    // Po końcu okresu zerwanie nie istnieje — właściwa ścieżka to claim.
    require!(
        now < ctx.accounts.user_position.end_ts,
        AnlError::PeriodAlreadyEnded
    );

    let (shares, debt, amount, anl_reward) = (
        ctx.accounts.user_position.shares,
        ctx.accounts.user_position.xnt_debt_index,
        ctx.accounts.user_position.amount,
        ctx.accounts.user_position.anl_reward,
    );

    // WP §7: naliczone XNT wracają do puli dystrybucji koszyka.
    let forfeited_xnt = ctx
        .accounts
        .pool_config
        .forfeit_position(shares, debt)
        .map_err(AnlError::from)?;

    // Principal wraca natychmiast i w całości — wyłącznie z Principal Vault.
    let bump = ctx.accounts.global_config.vault_authority_bump;
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[bump]];
    let signer: &[&[&[u8]]] = &[seeds];
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.anl_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.principal_vault.to_account_info(),
                mint: ctx.accounts.anl_mint.to_account_info(),
                to: ctx.accounts.owner_anl.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        amount,
        ctx.accounts.anl_mint.decimals,
    )?;

    // Rezerwacja ANL zwolniona — nagroda przepada, tokeny zostały w Reward Vault.
    let cfg = &mut ctx.accounts.global_config;
    cfg.anl_reward_reserved = cfg
        .anl_reward_reserved
        .checked_sub(anl_reward)
        .ok_or(AnlError::MathOverflow)?;
    let pool = &mut ctx.accounts.pool_config;
    pool.total_staked = pool
        .total_staked
        .checked_sub(amount)
        .ok_or(AnlError::MathOverflow)?;
    pool.position_count = pool
        .position_count
        .checked_sub(1)
        .ok_or(AnlError::MathOverflow)?;
    ctx.accounts.user_position.status = PositionStatus::Closed;

    emit!(PositionUnstakedEarly {
        owner: ctx.accounts.user_position.owner,
        position_index: ctx.accounts.user_position.position_index,
        principal_returned: amount,
        anl_reward_forfeited: anl_reward,
        xnt_forfeited: forfeited_xnt,
        timestamp: now,
    });
    Ok(())
}

#[event]
pub struct PositionUnstakedEarly {
    pub owner: Pubkey,
    pub position_index: u64,
    pub principal_returned: u64,
    pub anl_reward_forfeited: u64,
    pub xnt_forfeited: u64,
    pub timestamp: i64,
}
