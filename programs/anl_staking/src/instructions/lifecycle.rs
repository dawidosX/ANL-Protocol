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

#[derive(Accounts)]
pub struct SettleExpired<'info> {
    /// Permissionless — settle może wykonać każdy (bot operacyjny, sam user).
    pub cranker: Signer<'info>,

    /// AUDYT R4 (H-01/M-01): potrzebny genesis_start_ts — cap liczony od
    /// epoki ZEGARA (po roll_day), nie od leniwego pool.current_day.
    #[account(seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault,
        constraint = pool_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    #[account(
        mut,
        constraint = user_position.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
        seeds = [
            USER_POSITION_SEED,
            user_position.owner.as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Box<Account<'info, UserPosition>>,

    /// CHECK: checkpoint ostatniej epoki fundingu ≤ end_epoch pozycji;
    /// PDA + łańcuch (next) weryfikowane w handlerze. None dozwolone tylko,
    /// gdy pula nie miała fundingu ≤ end_epoch.
    pub xnt_checkpoint: Option<UncheckedAccount<'info>>,

    /// CHECK: AUDYT R4 — checkpoint doby domykanej przez roll_day (wymagany
    /// TYLKO gdy settle faktycznie domyka dobę: current_day != epoka zegara
    /// i koszyk > 0). PDA + dane weryfikowane w helperze. Może być tym samym
    /// kontem co xnt_checkpoint (zapis przed odczytem, borrowy sekwencyjne).
    #[account(mut)]
    pub prev_day_ckpt: Option<UncheckedAccount<'info>>,
}

/// Indeks-granica dla settlementu pozycji: snapshot ostatniej epoki
/// fundingu ≤ end_epoch (audyt #2). Gdy pula nie miała fundingu ≤ end_epoch,
/// pending = 0 (indeks = debt pozycji) i checkpoint nie jest wymagany.
fn settlement_cap_index(
    pool: &PoolConfig,
    pos: &UserPosition,
    ckpt: Option<&UncheckedAccount>,
    program_id: &Pubkey,
) -> Result<u128> {
    let last_closed = if pool.current_day_basket > 0 {
        pool.current_day.saturating_sub(1)
    } else {
        pool.current_day
    };
    let target = pos.end_epoch.min(last_closed);
    cap_index_at(pool, pos, target, ckpt, program_id)
}

/// Indeks-granica dla DOWOLNEJ epoki docelowej `target_epoch` (audyt #2:
/// snapshot ostatniej epoki fundingu ≤ target_epoch). Wspólny rdzeń dla
/// końcowego settlementu (target = end_epoch) ORAZ okien Genesis
/// (target = koniec ostatniego pełnego bloku 30-dniowego). Gdy pula nie
/// miała fundingu ≤ target_epoch → pending = 0 (indeks = debt), checkpoint
/// niewymagany.
fn cap_index_at(
    pool: &PoolConfig,
    pos: &UserPosition,
    target_epoch: u64,
    ckpt: Option<&UncheckedAccount>,
    program_id: &Pubkey,
) -> Result<u128> {
    if pool.last_funded_epoch == NO_EPOCH || pool.first_funded_epoch > target_epoch {
        return Ok(pos.xnt_debt_index);
    }
    let ai = ckpt.ok_or(AnlError::CheckpointRequired)?;
    let info = ai.to_account_info();
    require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
    let ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
    require!(
        ck.version == ACCOUNT_VERSION && ck.pool_type == pos.pool_type,
        AnlError::CheckpointMismatch
    );
    require!(ck.epoch <= target_epoch, AnlError::CheckpointMismatch);
    require!(
        ck.next_funded_epoch == NO_EPOCH || ck.next_funded_epoch > target_epoch,
        AnlError::CheckpointMismatch
    );
    let (pda, _) = Pubkey::find_program_address(
        &[
            XNT_CKPT_SEED,
            &[pos.pool_type as u8],
            &ck.epoch.to_le_bytes(),
        ],
        program_id,
    );
    require_keys_eq!(ai.key(), pda, AnlError::CheckpointMismatch);
    Ok(ck.index)
}

pub fn settle_expired(ctx: Context<SettleExpired>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    {
        let pos = &ctx.accounts.user_position;
        require!(
            pos.status == PositionStatus::Active,
            AnlError::PositionClosed
        );
        require!(!pos.settled, AnlError::AlreadySettled);
        require!(now >= pos.end_ts, AnlError::PeriodNotEnded);
    }

    // AUDYT R4 (H-01/M-01): dokładnie jak w stake — najpierw przekręć dobę
    // wg ZEGARA (domyka ewentualny stale koszyk i finalizuje jego checkpoint),
    // dopiero potem licz cap. Bez tego cap od leniwego current_day zaniżał
    // wypłatę o zafundowane, niedomknięte doby i zamrażał ją na stałe.
    let cur_epoch = epoch_of(now, ctx.accounts.global_config.genesis_start_ts)
        .ok_or(AnlError::BeforeGenesis)?;
    let program_id = *ctx.program_id;
    roll_day_and_write_checkpoint(
        &mut ctx.accounts.pool_config,
        cur_epoch,
        ctx.accounts.prev_day_ckpt.as_ref(),
        &program_id,
    )?;

    let cap = settlement_cap_index(
        &ctx.accounts.pool_config,
        &ctx.accounts.user_position,
        ctx.accounts.xnt_checkpoint.as_ref(),
        ctx.program_id,
    )?;
    let pos = &mut ctx.accounts.user_position;

    let frozen = ctx
        .accounts
        .pool_config
        .settle_position_at(pos.shares, pos.xnt_debt_index, cap)
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

#[derive(Accounts)]
pub struct Claim<'info> {
    #[account(mut, constraint = owner.key() == user_position.owner @ AnlError::PositionOwnerMismatch)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault,
        constraint = pool_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    #[account(
        mut,
        close = owner,
        constraint = user_position.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
        seeds = [
            USER_POSITION_SEED,
            owner.key().as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Box<Account<'info, UserPosition>>,

    /// CHECK: PDA-authority skarbców (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(address = global_config.xnt_mint @ AnlError::InvalidMint)]
    pub xnt_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [PRINCIPAL_VAULT_SEED], bump,
        token::mint = anl_mint, token::authority = vault_authority,
        token::token_program = anl_token_program)]
    pub principal_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [REWARD_VAULT_SEED], bump,
        token::mint = anl_mint, token::authority = vault_authority,
        token::token_program = anl_token_program)]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [XNT_VAULT_SEED], bump,
        token::mint = xnt_mint, token::authority = vault_authority,
        token::token_program = xnt_token_program)]
    pub xnt_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = owner,
        token::token_program = anl_token_program
    )]
    pub owner_anl: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = xnt_mint,
        token::authority = owner,
        token::token_program = xnt_token_program
    )]
    pub owner_xnt: Box<InterfaceAccount<'info, TokenAccount>>,

    pub anl_token_program: Program<'info, Token2022>,
    pub xnt_token_program: Program<'info, Token>,

    #[account(mut, seeds = [CAPY_VAULT_SEED], bump = global_config.capy_vault_bump)]
    pub capy_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [USER_PROFILE_SEED, owner.key().as_ref()],
        bump = user_profile.bump)]
    pub user_profile: Box<Account<'info, UserProfile>>,

    /// CHECK: jak w SettleExpired — checkpoint końca end_epoch pozycji.
    pub xnt_checkpoint: Option<UncheckedAccount<'info>>,

    /// CHECK: AUDYT R4 — checkpoint doby domykanej przez roll_day (wymagany
    /// TYLKO gdy claim faktycznie domyka dobę). PDA + dane w helperze.
    #[account(mut)]
    pub prev_day_ckpt: Option<UncheckedAccount<'info>>,
}

pub fn claim(ctx: Context<Claim>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.user_position.status == PositionStatus::Active,
        AnlError::PositionClosed
    );
    require!(
        now >= ctx.accounts.user_position.end_ts,
        AnlError::PeriodNotEnded
    );

    if !ctx.accounts.user_position.settled {
        // AUDYT R4 (H-01/M-01): roll doby wg ZEGARA przed liczeniem capu —
        // patrz komentarz w settle_expired. Po rollu wypłata dojrzałej
        // pozycji jest deterministyczna: zawsze pełne doby do end_epoch,
        // niezależnie od kolejności close_day / settle / claim.
        let cur_epoch = epoch_of(now, ctx.accounts.global_config.genesis_start_ts)
            .ok_or(AnlError::BeforeGenesis)?;
        let program_id = *ctx.program_id;
        roll_day_and_write_checkpoint(
            &mut ctx.accounts.pool_config,
            cur_epoch,
            ctx.accounts.prev_day_ckpt.as_ref(),
            &program_id,
        )?;
        let cap = settlement_cap_index(
            &ctx.accounts.pool_config,
            &ctx.accounts.user_position,
            ctx.accounts.xnt_checkpoint.as_ref(),
            ctx.program_id,
        )?;
        let (shares, debt) = (
            ctx.accounts.user_position.shares,
            ctx.accounts.user_position.xnt_debt_index,
        );
        let frozen = ctx
            .accounts
            .pool_config
            .settle_position_at(shares, debt, cap)
            .map_err(AnlError::from)?;
        let pos = &mut ctx.accounts.user_position;
        pos.xnt_accrued = frozen;
        pos.settled = true;
    }

    let amount = ctx.accounts.user_position.amount;
    let anl_reward = ctx.accounts.user_position.anl_reward;
    let xnt_accrued = ctx
        .accounts
        .user_position
        .xnt_accrued
        .checked_sub(ctx.accounts.user_position.xnt_window_claimed)
        .ok_or(AnlError::MathOverflow)?;

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
    let total_anl_paid = ctx.accounts.global_config.total_anl_paid;
    let capy_reserved = ctx.accounts.global_config.capy_reserved;
    let remaining_anl = ANL_REWARD_POOL.saturating_sub(total_anl_paid as u128);
    let capy_entitlement: u64 = if remaining_anl == 0 || anl_reward == 0 {
        0
    } else {
        let vault_bal = ctx.accounts.capy_vault.amount as u128;
        let reserved = capy_reserved as u128;
        let available = vault_bal.saturating_sub(reserved);
        let full = (anl_reward as u128)
            .checked_mul(available)
            .ok_or(AnlError::MathOverflow)?
            .checked_div(remaining_anl)
            .ok_or(AnlError::MathOverflow)?;
        let capped = core::cmp::min(full, available);
        u64::try_from(capped).map_err(|_| AnlError::MathOverflow)?
    };

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

    let cfg = &mut ctx.accounts.global_config;
    cfg.anl_reward_reserved = cfg
        .anl_reward_reserved
        .checked_sub(anl_reward)
        .ok_or(AnlError::MathOverflow)?;
    cfg.total_anl_paid = cfg
        .total_anl_paid
        .checked_add(anl_reward)
        .ok_or(AnlError::MathOverflow)?;
    if (cfg.total_anl_paid as u128) > ANL_REWARD_POOL {
        emit!(InvariantAlarm {
            kind: 1,
            value: cfg.total_anl_paid,
            limit_hi: (ANL_REWARD_POOL >> 64) as u64,
            limit_lo: ANL_REWARD_POOL as u64,
            timestamp: now,
        });
    }
    if capy_entitlement > 0 {
        cfg.capy_reserved = cfg
            .capy_reserved
            .checked_add(capy_entitlement)
            .ok_or(AnlError::MathOverflow)?;
        ctx.accounts.user_profile.pending_capy = ctx
            .accounts
            .user_profile
            .pending_capy
            .checked_add(capy_entitlement)
            .ok_or(AnlError::MathOverflow)?;
    }
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
        capy_reward: capy_entitlement,
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
    pub capy_reward: u64,
    pub timestamp: i64,
}

/// Alarm ksiegowy (miekki czujnik v3.1) - nie blokuje operacji.
#[event]
pub struct InvariantAlarm {
    pub kind: u8,
    pub value: u64,
    pub limit_hi: u64,
    pub limit_lo: u64,
    pub timestamp: i64,
}

/// Okienkowa wypłata XNT dla pozycji Genesis (WP okna 30-dniowe). Wypłaca
/// XNT naliczony do końca ostatniego PEŁNEGO bloku 30-dniowego, minus to,
/// co już wypłacono w poprzednich oknach (kumulacja). NIE zdejmuje shares,
/// NIE zamyka pozycji — kapitał zablokowany do end_ts (zwykły claim). Konta
/// jak podzbiór Claim: bez principal_vault/reward_vault (wypłacamy tylko XNT)
/// i BEZ close=owner (pozycja żyje).
#[derive(Accounts)]
pub struct ClaimGenesisWindow<'info> {
    #[account(mut, constraint = owner.key() == user_position.owner @ AnlError::PositionOwnerMismatch)]
    pub owner: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault,
        constraint = pool_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    #[account(
        mut,
        constraint = user_position.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
        seeds = [
            USER_POSITION_SEED,
            owner.key().as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Box<Account<'info, UserPosition>>,

    /// CHECK: PDA-authority skarbców (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.xnt_mint @ AnlError::InvalidMint)]
    pub xnt_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [XNT_VAULT_SEED], bump,
        token::mint = xnt_mint, token::authority = vault_authority,
        token::token_program = xnt_token_program)]
    pub xnt_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = xnt_mint,
        token::authority = owner,
        token::token_program = xnt_token_program
    )]
    pub owner_xnt: Box<InterfaceAccount<'info, TokenAccount>>,

    pub xnt_token_program: Program<'info, Token>,

    /// CHECK: checkpoint ≤ prog_epoch (koniec ostatniego pełnego bloku).
    /// PDA + łańcuch (next) weryfikowane w handlerze. None dozwolone tylko,
    /// gdy pula nie miała fundingu ≤ prog_epoch.
    pub xnt_checkpoint: Option<UncheckedAccount<'info>>,
}

/// Liczba dni w bloku okienkowym Genesis — z anl-math (reaguje na test-periods).
pub use anl_math::GENESIS_WINDOW_DAYS;

pub fn claim_genesis_window(ctx: Context<ClaimGenesisWindow>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let genesis_start_ts = ctx.accounts.global_config.genesis_start_ts;

    require!(
        ctx.accounts.user_position.status == PositionStatus::Active,
        AnlError::PositionClosed
    );
    require!(
        ctx.accounts.user_position.pool_type == PoolType::Genesis,
        AnlError::NotGenesisPool
    );
    require!(
        now < ctx.accounts.user_position.end_ts,
        AnlError::PeriodAlreadyEnded
    );

    let cur_epoch = epoch_of(now, genesis_start_ts).ok_or(AnlError::BeforeGenesis)?;
    let full_blocks = cur_epoch / GENESIS_WINDOW_DAYS;
    require!(full_blocks >= 1, AnlError::WindowNotReached);
    let prog_epoch = full_blocks * GENESIS_WINDOW_DAYS - 1;

    let cap = cap_index_at(
        &ctx.accounts.pool_config,
        &ctx.accounts.user_position,
        prog_epoch,
        ctx.accounts.xnt_checkpoint.as_ref(),
        ctx.program_id,
    )?;

    let shares = ctx.accounts.user_position.shares;
    let debt = ctx.accounts.user_position.xnt_debt_index;
    let accrued_to_prog = ctx
        .accounts
        .pool_config
        .accrued_to_cap(shares, debt, cap)
        .map_err(AnlError::from)?;

    let already = ctx.accounts.user_position.xnt_window_claimed;
    let to_pay = accrued_to_prog
        .checked_sub(already)
        .ok_or(AnlError::MathOverflow)?;
    require!(to_pay > 0, AnlError::NothingToClaim);
    require!(
        ctx.accounts.xnt_vault.amount >= to_pay,
        AnlError::InsufficientXntVault
    );

    let bump = ctx.accounts.global_config.vault_authority_bump;
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[bump]];
    let signer: &[&[&[u8]]] = &[seeds];

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
        to_pay,
        ctx.accounts.xnt_mint.decimals,
    )?;

    let pos = &mut ctx.accounts.user_position;
    pos.xnt_window_claimed = already.checked_add(to_pay).ok_or(AnlError::MathOverflow)?;
    pos.last_window_ts = now;

    emit!(GenesisWindowClaimed {
        owner: pos.owner,
        position_index: pos.position_index,
        xnt_paid: to_pay,
        cumulative_claimed: pos.xnt_window_claimed,
        timestamp: now,
    });
    Ok(())
}

#[event]
pub struct GenesisWindowClaimed {
    pub owner: Pubkey,
    pub position_index: u64,
    pub xnt_paid: u64,
    pub cumulative_claimed: u64,
    pub timestamp: i64,
}

#[derive(Accounts)]
pub struct UnstakeEarly<'info> {
    #[account(mut, constraint = owner.key() == user_position.owner @ AnlError::PositionOwnerMismatch)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    #[account(
        mut,
        seeds = [POOL_SEED, &[pool_config.pool_type as u8]],
        bump = pool_config.bump,
        constraint = pool_config.pool_type == user_position.pool_type @ AnlError::InvalidVault,
        constraint = pool_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
    )]
    pub pool_config: Box<Account<'info, PoolConfig>>,

    #[account(
        mut,
        close = owner,
        constraint = user_position.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
        seeds = [
            USER_POSITION_SEED,
            owner.key().as_ref(),
            &user_position.position_index.to_le_bytes()
        ],
        bump = user_position.bump
    )]
    pub user_position: Box<Account<'info, UserPosition>>,

    /// CHECK: PDA-authority skarbców (seeds + bump).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [PRINCIPAL_VAULT_SEED], bump,
        token::mint = anl_mint, token::authority = vault_authority,
        token::token_program = anl_token_program)]
    pub principal_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mut,
        token::mint = anl_mint,
        token::authority = owner,
        token::token_program = anl_token_program
    )]
    pub owner_anl: Box<InterfaceAccount<'info, TokenAccount>>,

    pub anl_token_program: Program<'info, Token2022>,
}

pub fn unstake_early(ctx: Context<UnstakeEarly>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    require!(
        ctx.accounts.user_position.pool_type == PoolType::Flexible,
        AnlError::GenesisLocked
    );
    require!(
        ctx.accounts.user_position.status == PositionStatus::Active,
        AnlError::PositionClosed
    );
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

    let forfeited_xnt = ctx
        .accounts
        .pool_config
        .forfeit_position(shares, debt)
        .map_err(AnlError::from)?;

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

#[derive(Accounts)]
pub struct ClaimCapy<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbcow.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(mut, seeds = [USER_PROFILE_SEED, owner.key().as_ref()],
        bump = user_profile.bump,
        constraint = user_profile.owner == owner.key() @ AnlError::PositionOwnerMismatch)]
    pub user_profile: Box<Account<'info, UserProfile>>,

    #[account(address = global_config.capy_mint @ AnlError::InvalidMint)]
    pub capy_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, seeds = [CAPY_VAULT_SEED], bump = global_config.capy_vault_bump,
        token::mint = capy_mint, token::authority = vault_authority,
        token::token_program = capy_token_program)]
    pub capy_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, token::mint = capy_mint, token::authority = owner,
        token::token_program = capy_token_program)]
    pub owner_capy: Box<InterfaceAccount<'info, TokenAccount>>,

    pub capy_token_program: Program<'info, Token2022>,
}

pub fn claim_capy(ctx: Context<ClaimCapy>) -> Result<()> {
    let pending = ctx.accounts.user_profile.pending_capy;
    require!(pending > 0, AnlError::ZeroAmount);

    let vault_bal = ctx.accounts.capy_vault.amount;
    let to_pay = core::cmp::min(pending, vault_bal);
    require!(to_pay > 0, AnlError::ZeroAmount);

    let bump = ctx.accounts.global_config.vault_authority_bump;
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[bump]];
    let signer: &[&[&[u8]]] = &[seeds];

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.capy_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.capy_vault.to_account_info(),
                mint: ctx.accounts.capy_mint.to_account_info(),
                to: ctx.accounts.owner_capy.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        to_pay,
        ctx.accounts.capy_mint.decimals,
    )?;

    ctx.accounts.user_profile.pending_capy = ctx
        .accounts
        .user_profile
        .pending_capy
        .checked_sub(to_pay)
        .ok_or(AnlError::MathOverflow)?;
    ctx.accounts.global_config.capy_reserved = ctx
        .accounts
        .global_config
        .capy_reserved
        .checked_sub(to_pay)
        .ok_or(AnlError::MathOverflow)?;

    emit!(CapyClaimed {
        owner: ctx.accounts.owner.key(),
        amount: to_pay,
        pending_remaining: ctx.accounts.user_profile.pending_capy,
        timestamp: Clock::get()?.unix_timestamp,
    });
    Ok(())
}

#[event]
pub struct CapyClaimed {
    pub owner: Pubkey,
    pub amount: u64,
    pub pending_remaining: u64,
    pub timestamp: i64,
}
