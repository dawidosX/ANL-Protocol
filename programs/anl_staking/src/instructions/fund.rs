//! fund_rewards / fund_xnt — zasilanie skarbców (WP v1.0 §3, §8).
//!
//! `fund_rewards`: depozyt ANL do Reward Vault (pula 200M — rezerwuar).
//! `fund_xnt`: DZIENNY wpływ przychodu walidatora; kwota NETTO dzielona
//! 65% Genesis / 35% Flexible i wprowadzana do indeksów koszyków.
//! Zasada pustego koszyka: część bez aktywnych pozycji czeka
//! w `xnt_undistributed` i wchodzi przy najbliższym fundingu (WP §8).

use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TransferChecked};

use crate::constants::*;
use crate::errors::AnlError;
use crate::state::*;

// ------------------------------------------------------------ fund_rewards

#[derive(Accounts)]
pub struct FundRewards<'info> {
    #[account(mut, constraint = funder.key() == global_config.authority @ AnlError::InvalidAuthority)]
    pub funder: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = funder,
        token::token_program = anl_token_program
    )]
    pub funder_anl: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [REWARD_VAULT_SEED], bump)]
    pub reward_vault: InterfaceAccount<'info, TokenAccount>,

    pub anl_token_program: Program<'info, Token2022>,
}

pub fn fund_rewards(ctx: Context<FundRewards>, amount: u64) -> Result<()> {
    require!(amount > 0, AnlError::ZeroAmount);
    let before = ctx.accounts.reward_vault.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.anl_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.funder_anl.to_account_info(),
                mint: ctx.accounts.anl_mint.to_account_info(),
                to: ctx.accounts.reward_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.anl_mint.decimals,
    )?;
    ctx.accounts.reward_vault.reload()?;
    let net = ctx
        .accounts
        .reward_vault
        .amount
        .checked_sub(before)
        .ok_or(AnlError::MathOverflow)?;
    emit!(RewardsFunded {
        amount_net: net,
        vault_balance: ctx.accounts.reward_vault.amount,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

#[event]
pub struct RewardsFunded {
    pub amount_net: u64,
    pub vault_balance: u64,
    pub timestamp: i64,
}

// ------------------------------------------------------------ fund_xnt

#[derive(Accounts)]
pub struct FundXnt<'info> {
    #[account(mut, constraint = funder.key() == global_config.authority @ AnlError::InvalidAuthority)]
    pub funder: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump)]
    pub global_config: Account<'info, GlobalConfig>,

    #[account(address = global_config.xnt_mint @ AnlError::InvalidMint)]
    pub xnt_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        token::mint = xnt_mint,
        token::authority = funder,
        token::token_program = xnt_token_program
    )]
    pub funder_xnt: InterfaceAccount<'info, TokenAccount>,

    #[account(mut, seeds = [XNT_VAULT_SEED], bump)]
    pub xnt_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[PoolType::Genesis as u8]],
        bump = genesis_pool.bump,
        constraint = genesis_pool.pool_type == PoolType::Genesis @ AnlError::InvalidVault
    )]
    pub genesis_pool: Account<'info, PoolConfig>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[PoolType::Flexible as u8]],
        bump = flexible_pool.bump,
        constraint = flexible_pool.pool_type == PoolType::Flexible @ AnlError::InvalidVault
    )]
    pub flexible_pool: Account<'info, PoolConfig>,

    pub xnt_token_program: Program<'info, Token>,
}

pub fn fund_xnt(ctx: Context<FundXnt>, amount: u64) -> Result<()> {
    require!(amount > 0, AnlError::ZeroAmount);
    let before = ctx.accounts.xnt_vault.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.xnt_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.funder_xnt.to_account_info(),
                mint: ctx.accounts.xnt_mint.to_account_info(),
                to: ctx.accounts.xnt_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.xnt_mint.decimals,
    )?;
    ctx.accounts.xnt_vault.reload()?;
    let net = ctx
        .accounts
        .xnt_vault
        .amount
        .checked_sub(before)
        .ok_or(AnlError::MathOverflow)?;

    // WP §8: podział dziennej puli 65/35 — suma części zawsze równa całości.
    let (genesis_part, flexible_part) = anl_math::split_xnt(net);
    ctx.accounts
        .genesis_pool
        .fund_xnt_part(genesis_part)
        .map_err(AnlError::from)?;
    ctx.accounts
        .flexible_pool
        .fund_xnt_part(flexible_part)
        .map_err(AnlError::from)?;

    emit!(XntFunded {
        amount_net: net,
        genesis_part,
        flexible_part,
        genesis_index: ctx.accounts.genesis_pool.xnt_reward_index,
        flexible_index: ctx.accounts.flexible_pool.xnt_reward_index,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

#[event]
pub struct XntFunded {
    pub amount_net: u64,
    pub genesis_part: u64,
    pub flexible_part: u64,
    pub genesis_index: u128,
    pub flexible_index: u128,
    pub timestamp: i64,
}
