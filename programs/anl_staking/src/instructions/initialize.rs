//! initialize — TC-001…TC-006. Tworzy GlobalConfig, VaultAuthority i trzy skarbce.
//! D-14: ANL = Token-2022, XNT = legacy SPL Token (dwa programy tokenowe).
//! D-11: `start_paused` — controlled rollout; `genesis_start_ts` = planowany go-live.

use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::constants::*;
use crate::errors::AnlError;
use crate::state::*;

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = GlobalConfig::LEN,
        seeds = [GLOBAL_CONFIG_SEED],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    /// CHECK: PDA-authority wszystkich skarbców; bez danych, walidacja seeds+bump (TC-145).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    /// ANL musi należeć do Token-2022 (TC-004, D-14).
    #[account(mint::token_program = anl_token_program)]
    pub anl_mint: InterfaceAccount<'info, Mint>,

    /// XNT (wrapped native X1) musi należeć do legacy SPL Token (TC-005, D-14).
    #[account(mint::token_program = xnt_token_program)]
    pub xnt_mint: InterfaceAccount<'info, Mint>,

    #[account(
        init,
        payer = authority,
        seeds = [PRINCIPAL_VAULT_SEED],
        bump,
        token::mint = anl_mint,
        token::authority = vault_authority,
        token::token_program = anl_token_program
    )]
    pub principal_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        seeds = [REWARD_VAULT_SEED],
        bump,
        token::mint = anl_mint,
        token::authority = vault_authority,
        token::token_program = anl_token_program
    )]
    pub reward_vault: InterfaceAccount<'info, TokenAccount>,

    #[account(
        init,
        payer = authority,
        seeds = [XNT_VAULT_SEED],
        bump,
        token::mint = xnt_mint,
        token::authority = vault_authority,
        token::token_program = xnt_token_program
    )]
    pub xnt_vault: InterfaceAccount<'info, TokenAccount>,

    pub anl_token_program: Program<'info, Token2022>,
    pub xnt_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<Initialize>, genesis_start_ts: i64, start_paused: bool) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    // Go-live nie może być w przeszłości: pauza nie zjada okna 20% (sekcja 7).
    require!(genesis_start_ts >= now, AnlError::GenesisStartInPast);

    #[cfg(feature = "test-periods")]
    msg!("!!! BUILD test-periods: okna 3/9 dni, min okres 1 dzien - NIE WDRAZAC NA MAINNET !!!");

    let cfg = &mut ctx.accounts.global_config;
    cfg.version = ACCOUNT_VERSION;
    cfg.authority = ctx.accounts.authority.key();
    cfg.anl_mint = ctx.accounts.anl_mint.key();
    cfg.xnt_mint = ctx.accounts.xnt_mint.key();
    cfg.paused = start_paused;
    cfg.genesis_start_ts = genesis_start_ts;
    cfg.anl_reward_reserved = 0;
    cfg.bump = ctx.bumps.global_config;
    cfg.vault_authority_bump = ctx.bumps.vault_authority;
    cfg.reserved = [0; 56];

    emit!(ProtocolInitialized {
        authority: cfg.authority,
        anl_mint: cfg.anl_mint,
        xnt_mint: cfg.xnt_mint,
        genesis_start_ts,
        paused: start_paused,
        timestamp: now,
    });
    Ok(())
}

#[event]
pub struct ProtocolInitialized {
    pub authority: Pubkey,
    pub anl_mint: Pubkey,
    pub xnt_mint: Pubkey,
    pub genesis_start_ts: i64,
    pub paused: bool,
    pub timestamp: i64,
}
