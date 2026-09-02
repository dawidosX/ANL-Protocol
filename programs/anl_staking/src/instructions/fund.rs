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

#[derive(Accounts)]
pub struct FundRewards<'info> {
    #[account(mut, constraint = funder.key() == global_config.authority
        || funder.key() == global_config.operator @ AnlError::InvalidAuthority)]
    pub funder: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbcow (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = funder,
        token::token_program = anl_token_program
    )]
    pub funder_anl: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [REWARD_VAULT_SEED], bump,
        token::mint = anl_mint, token::authority = vault_authority,
        token::token_program = anl_token_program)]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,

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

#[derive(Accounts)]
pub struct FundCapy<'info> {
    #[account(mut)]
    pub funder: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbcow.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.capy_mint @ AnlError::InvalidMint)]
    pub capy_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, token::mint = capy_mint, token::authority = funder,
        token::token_program = capy_token_program)]
    pub funder_capy: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [CAPY_VAULT_SEED], bump = global_config.capy_vault_bump,
        token::mint = capy_mint, token::authority = vault_authority,
        token::token_program = capy_token_program)]
    pub capy_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub capy_token_program: Program<'info, Token2022>,
}

pub fn fund_capy(ctx: Context<FundCapy>, amount: u64) -> Result<()> {
    require!(amount > 0, AnlError::ZeroAmount);
    let before = ctx.accounts.capy_vault.amount;
    token_interface::transfer_checked(
        CpiContext::new(
            ctx.accounts.capy_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.funder_capy.to_account_info(),
                mint: ctx.accounts.capy_mint.to_account_info(),
                to: ctx.accounts.capy_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.capy_mint.decimals,
    )?;
    ctx.accounts.capy_vault.reload()?;
    let net = ctx
        .accounts
        .capy_vault
        .amount
        .checked_sub(before)
        .ok_or(AnlError::MathOverflow)?;
    emit!(CapyFunded {
        amount_net: net,
        vault_balance: ctx.accounts.capy_vault.amount,
        timestamp: Clock::get()?.unix_timestamp
    });
    Ok(())
}

#[event]
pub struct CapyFunded {
    pub amount_net: u64,
    pub vault_balance: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
#[instruction(amount: u64, epoch: u64)]
pub struct FundXnt<'info> {
    #[account(mut, constraint = funder.key() == global_config.authority
        || funder.key() == global_config.operator @ AnlError::InvalidAuthority)]
    pub funder: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbcow (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.xnt_mint @ AnlError::InvalidMint)]
    pub xnt_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        mut,
        token::mint = xnt_mint,
        token::authority = funder,
        token::token_program = xnt_token_program
    )]
    pub funder_xnt: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [XNT_VAULT_SEED], bump,
        token::mint = xnt_mint, token::authority = vault_authority,
        token::token_program = xnt_token_program)]
    pub xnt_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[PoolType::Genesis as u8]],
        bump = genesis_pool.bump,
        constraint = genesis_pool.pool_type == PoolType::Genesis @ AnlError::InvalidVault,
        constraint = genesis_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub genesis_pool: Box<Account<'info, PoolConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[PoolType::Flexible as u8]],
        bump = flexible_pool.bump,
        constraint = flexible_pool.pool_type == PoolType::Flexible @ AnlError::InvalidVault,
        constraint = flexible_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub flexible_pool: Box<Account<'info, PoolConfig>>,

    pub xnt_token_program: Program<'info, Token>,

    /// Checkpoint BIEŻĄCEJ epoki fundingu — Genesis.
    #[account(init_if_needed, payer = funder, space = XntCheckpoint::LEN,
        seeds = [XNT_CKPT_SEED, &[PoolType::Genesis as u8], &epoch.to_le_bytes()],
        bump)]
    pub genesis_ckpt: Box<Account<'info, XntCheckpoint>>,

    /// Checkpoint BIEŻĄCEJ epoki fundingu — Flexible.
    #[account(init_if_needed, payer = funder, space = XntCheckpoint::LEN,
        seeds = [XNT_CKPT_SEED, &[PoolType::Flexible as u8], &epoch.to_le_bytes()],
        bump)]
    pub flexible_ckpt: Box<Account<'info, XntCheckpoint>>,

    /// CHECK: checkpoint POPRZEDNIEJ epoki fundingu Genesis (wymagany, gdy
    /// last_funded_epoch ∉ {NO_EPOCH, epoch}); PDA weryfikowane w handlerze.
    #[account(mut)]
    pub genesis_prev_ckpt: Option<UncheckedAccount<'info>>,

    /// CHECK: jw. dla Flexible.
    #[account(mut)]
    pub flexible_prev_ckpt: Option<UncheckedAccount<'info>>,

    pub system_program: Program<'info, System>,
}

/// Przesuwa łańcuch checkpointów puli na epokę `epoch`:
/// - pierwszy funding w tej epoce ≠ last: domyka poprzedni checkpoint
///   (next_funded_epoch = epoch) i inicjalizuje bieżący,
/// - kolejny funding w tej samej epoce: bez zmian łańcucha.
fn roll_checkpoint<'info>(
    pool: &mut Account<'info, PoolConfig>,
    cur: &mut Account<'info, XntCheckpoint>,
    cur_bump: u8,
    prev: Option<&UncheckedAccount<'info>>,
    epoch: u64,
    program_id: &Pubkey,
) -> Result<()> {
    if pool.last_funded_epoch == epoch {
        require!(
            cur.version == ACCOUNT_VERSION && cur.epoch == epoch && cur.pool_type == pool.pool_type,
            AnlError::CheckpointMismatch
        );
        return Ok(());
    }
    require!(
        pool.last_funded_epoch == NO_EPOCH || epoch > pool.last_funded_epoch,
        AnlError::EpochMismatch
    );
    if pool.last_funded_epoch == NO_EPOCH {
        pool.first_funded_epoch = epoch;
    } else {
        let prev_ai = prev.ok_or(AnlError::CheckpointRequired)?;
        let (pda, _) = Pubkey::find_program_address(
            &[
                XNT_CKPT_SEED,
                &[pool.pool_type as u8],
                &pool.last_funded_epoch.to_le_bytes(),
            ],
            program_id,
        );
        require_keys_eq!(prev_ai.key(), pda, AnlError::CheckpointMismatch);
        let info = prev_ai.to_account_info();
        require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
        let mut ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
        require!(
            ck.version == ACCOUNT_VERSION
                && ck.epoch == pool.last_funded_epoch
                && ck.pool_type == pool.pool_type
                && ck.next_funded_epoch == NO_EPOCH,
            AnlError::CheckpointMismatch
        );
        ck.next_funded_epoch = epoch;
        ck.try_serialize(&mut &mut info.data.borrow_mut()[..])?;
    }
    cur.version = ACCOUNT_VERSION;
    cur.pool_type = pool.pool_type;
    cur.epoch = epoch;
    cur.next_funded_epoch = NO_EPOCH;
    cur.bump = cur_bump;
    cur.reserved = [0; 13];
    Ok(())
}

/// Wariant A (Sposob 2): zapisuje finalny index do checkpointu domknietej doby.
/// Wolane po add_to_basket, gdy pool.xnt_reward_index zawiera juz domkniecie
/// poprzedniej doby. prev to checkpoint tej domknietej doby (Option — None gdy
/// brak poprzedniej doby, placeholder gdy last_funded_epoch=NO_EPOCH). Zapis
/// tylko gdy owner == program_id (realny checkpoint, nie placeholder/None).
fn write_final_index<'info>(
    prev: Option<&UncheckedAccount<'info>>,
    pool_type: PoolType,
    prev_epoch: u64,
    final_index: u128,
    program_id: &Pubkey,
) -> Result<()> {
    let Some(prev_ai) = prev else {
        return Ok(());
    };
    if prev_epoch == NO_EPOCH {
        return Ok(());
    }
    let info = prev_ai.to_account_info();
    if *info.owner != *program_id {
        return Ok(());
    }
    let (pda, _) = Pubkey::find_program_address(
        &[XNT_CKPT_SEED, &[pool_type as u8], &prev_epoch.to_le_bytes()],
        program_id,
    );
    require_keys_eq!(info.key(), pda, AnlError::CheckpointMismatch);
    let mut ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
    require!(
        ck.version == ACCOUNT_VERSION && ck.epoch == prev_epoch && ck.pool_type == pool_type,
        AnlError::CheckpointMismatch
    );
    ck.index = final_index;
    ck.try_serialize(&mut &mut info.data.borrow_mut()[..])?;
    Ok(())
}

pub fn fund_xnt(ctx: Context<FundXnt>, amount: u64, epoch: u64) -> Result<()> {
    require!(amount > 0, AnlError::ZeroAmount);
    let now = Clock::get()?.unix_timestamp;
    let cur_epoch = epoch_of(now, ctx.accounts.global_config.genesis_start_ts)
        .ok_or(AnlError::BeforeGenesis)?;
    require!(epoch == cur_epoch, AnlError::EpochMismatch);
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

    let g_empty = ctx.accounts.genesis_pool.total_shares == 0;
    let f_empty = ctx.accounts.flexible_pool.total_shares == 0;
    let (genesis_part, flexible_part) = if g_empty && f_empty {
        anl_math::split_xnt(net)
    } else if f_empty {
        (net, 0)
    } else if g_empty {
        (0, net)
    } else {
        anl_math::split_xnt(net)
    };

    let g_bump = ctx.bumps.genesis_ckpt;
    let f_bump = ctx.bumps.flexible_ckpt;
    roll_checkpoint(
        &mut ctx.accounts.genesis_pool,
        &mut ctx.accounts.genesis_ckpt,
        g_bump,
        ctx.accounts.genesis_prev_ckpt.as_ref(),
        epoch,
        ctx.program_id,
    )?;
    roll_checkpoint(
        &mut ctx.accounts.flexible_pool,
        &mut ctx.accounts.flexible_ckpt,
        f_bump,
        ctx.accounts.flexible_prev_ckpt.as_ref(),
        epoch,
        ctx.program_id,
    )?;

    ctx.accounts
        .genesis_pool
        .add_to_basket(genesis_part, epoch)
        .map_err(AnlError::from)?;
    ctx.accounts
        .flexible_pool
        .add_to_basket(flexible_part, epoch)
        .map_err(AnlError::from)?;

    write_final_index(
        ctx.accounts.genesis_prev_ckpt.as_ref(),
        PoolType::Genesis,
        ctx.accounts.genesis_pool.last_funded_epoch,
        ctx.accounts.genesis_pool.xnt_reward_index,
        ctx.program_id,
    )?;
    write_final_index(
        ctx.accounts.flexible_prev_ckpt.as_ref(),
        PoolType::Flexible,
        ctx.accounts.flexible_pool.last_funded_epoch,
        ctx.accounts.flexible_pool.xnt_reward_index,
        ctx.program_id,
    )?;

    ctx.accounts.genesis_ckpt.index = ctx.accounts.genesis_pool.xnt_reward_index;
    ctx.accounts.flexible_ckpt.index = ctx.accounts.flexible_pool.xnt_reward_index;
    ctx.accounts.genesis_pool.last_funded_epoch = epoch;
    ctx.accounts.flexible_pool.last_funded_epoch = epoch;

    ctx.accounts.global_config.total_xnt_funded = ctx
        .accounts
        .global_config
        .total_xnt_funded
        .saturating_add(net);

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

#[derive(Accounts)]
pub struct SetOperator<'info> {
    #[account(constraint = authority.key() == global_config.authority @ AnlError::InvalidAuthority)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,
}

/// Ustawia gorący klucz operatora bota dziennego (tylko authority).
/// Operator może wyłącznie zasilać skarbce (fund_rewards / fund_xnt) —
/// nie ma dostępu do wypłat, pauzy ani parametrów.
pub fn set_operator(ctx: Context<SetOperator>, new_operator: Pubkey) -> Result<()> {
    require!(new_operator != Pubkey::default(), AnlError::InvalidOperator);
    let old = ctx.accounts.global_config.operator;
    require!(new_operator != old, AnlError::InvalidOperator);
    ctx.accounts.global_config.operator = new_operator;
    emit!(OperatorChanged {
        old_operator: old,
        new_operator,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

#[event]
pub struct OperatorChanged {
    pub old_operator: Pubkey,
    pub new_operator: Pubkey,
    pub timestamp: i64,
}

/// Wariant A (Sposob 2): PUBLICZNE domkniecie doby. Rozdziela koszyk dnia
/// (podnosi index o koszyk/total_shares) i zapisuje finalny index do checkpointu
/// tej doby. Ktokolwiek moze wywolac (deterministyczne) — zwykle bot o ~2:00.
/// Domyka JEDNA dobe (pool.current_day), tylko jesli juz minela.
/// Idempotentne: jesli koszyk pusty (juz domkniete) — no-op.
pub fn close_day(ctx: Context<CloseDay>, epoch: u64) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let cur_epoch = epoch_of(now, ctx.accounts.global_config.genesis_start_ts)
        .ok_or(AnlError::BeforeGenesis)?;
    require!(epoch < cur_epoch, AnlError::EpochMismatch);
    let pool = &mut ctx.accounts.pool_config;
    require!(pool.current_day == epoch, AnlError::EpochMismatch);
    if pool.current_day_basket == 0 {
        return Ok(());
    }
    pool.close_day().map_err(AnlError::from)?;
    pool.current_day = cur_epoch;
    let final_index = pool.xnt_reward_index;
    let info = ctx.accounts.day_ckpt.to_account_info();
    let mut ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
    require!(
        ck.version == ACCOUNT_VERSION && ck.epoch == epoch && ck.pool_type == pool.pool_type,
        AnlError::CheckpointMismatch
    );
    ck.index = final_index;
    ck.try_serialize(&mut &mut info.data.borrow_mut()[..])?;
    emit!(DayClosed {
        pool_type: pool.pool_type as u8,
        epoch,
        index: final_index,
        timestamp: now,
    });
    Ok(())
}

#[derive(Accounts)]
#[instruction(epoch: u64)]
pub struct CloseDay<'info> {
    pub caller: Signer<'info>,

    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    /// CHECK: checkpoint domykanej doby; PDA + dane weryfikowane w handlerze.
    #[account(mut,
        seeds = [XNT_CKPT_SEED, &[pool_config.pool_type as u8], &epoch.to_le_bytes()],
        bump)]
    pub day_ckpt: UncheckedAccount<'info>,
}

#[event]
pub struct DayClosed {
    pub pool_type: u8,
    pub epoch: u64,
    pub index: u128,
    pub timestamp: i64,
}
