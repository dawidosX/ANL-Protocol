//! initialize + init_vaults — TC-001…TC-006.
//! ROZBICIE (fix stack-overflow SBF): `initialize` tworzy tylko GlobalConfig i
//! waliduje minty; `init_vaults` tworzy trzy skarbce osobno. Każda instrukcja
//! ma małą ramkę stosu (pojedyncze `init` zamiast trzech naraz). Logika i
//! inwarianty bez zmian — jedynie podział na dwa etapy jednej operacji setup.
//! Zabezpieczenie stanu pośredniego: init_vaults wymaga `has_one = authority`
//! z już istniejącego GlobalConfig — obcy nie podłoży własnych vaultów (front-run
//! niemożliwy; PDA i tak wykluczają duplikaty).
//! D-14: ANL = Token-2022, XNT = legacy SPL Token (dwa programy tokenowe).
//! D-11: `start_paused` — controlled rollout; `genesis_start_ts` = planowany go-live.

use anchor_lang::prelude::*;
use anchor_spl::token::Token;
use anchor_spl::token_2022::spl_token_2022::extension::{
    BaseStateWithExtensions, ExtensionType, StateWithExtensions,
};
use anchor_spl::token_2022::spl_token_2022::state::Mint as MintState;
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
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority wszystkich skarbców; walidacja seeds+bump (TC-145).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump)]
    pub vault_authority: UncheckedAccount<'info>,

    /// ANL musi należeć do Token-2022 (TC-004, D-14).
    #[account(mint::token_program = anl_token_program)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    /// XNT (wrapped native X1) musi należeć do legacy SPL Token (TC-005, D-14).
    #[account(mint::token_program = xnt_token_program)]
    pub xnt_mint: Box<InterfaceAccount<'info, Mint>>,

    pub anl_token_program: Program<'info, Token2022>,
    pub xnt_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_handler(
    ctx: Context<Initialize>,
    genesis_start_ts: i64,
    start_paused: bool,
) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    #[cfg(not(feature = "test-periods"))]
    require_keys_eq!(
        ctx.accounts.xnt_mint.key(),
        EXPECTED_XNT_MINT,
        AnlError::InvalidXntMint
    );
    // AUDYT R6 (I-01): inicjalizator przypięty w buildach produkcyjnych —
    // koniec z first-come authority po deployu.
    #[cfg(not(feature = "test-periods"))]
    require_keys_eq!(
        ctx.accounts.authority.key(),
        EXPECTED_INIT_AUTHORITY,
        AnlError::InvalidAuthority
    );

    // AUDYT R4 (L-02 raport B): stałe bazowe (MIN_STAKE_AMOUNT,
    // ANL_REWARD_POOL, mianownik CAPY) zakładają 9 miejsc dziesiętnych —
    // wymuszamy to na mincie, jak przy CAPY.
    require!(ctx.accounts.anl_mint.decimals == 9, AnlError::InvalidMint);

    {
        let ai = ctx.accounts.anl_mint.to_account_info();
        let data = ai.try_borrow_data()?;
        let state = StateWithExtensions::<MintState>::unpack(&data)
            .map_err(|_| error!(AnlError::InvalidMint))?;
        require!(
            state.base.freeze_authority.is_none(),
            AnlError::MintHasFreezeAuthority
        );
        require!(
            state.base.mint_authority.is_none(),
            AnlError::MintHasMintAuthority
        );
        for ext in state
            .get_extension_types()
            .map_err(|_| error!(AnlError::InvalidMint))?
        {
            match ext {
                ExtensionType::MetadataPointer | ExtensionType::TokenMetadata => {}
                _ => return err!(AnlError::ForbiddenMintExtension),
            }
        }
    }

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
    cfg.operator = ctx.accounts.authority.key();
    cfg.bump = ctx.bumps.global_config;
    cfg.vault_authority_bump = ctx.bumps.vault_authority;
    cfg.total_xnt_funded = 0;
    cfg.reserved = [0; 16];

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

/// Wspólne konta bazowe dla init pojedynczego vaulta ANL (principal/reward).
#[derive(Accounts)]
pub struct InitPrincipalVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        has_one = authority @ AnlError::InvalidAuthority,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbców (seeds + bump z configu).
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = authority,
        seeds = [PRINCIPAL_VAULT_SEED],
        bump,
        token::mint = anl_mint,
        token::authority = vault_authority,
        token::token_program = anl_token_program
    )]
    pub principal_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub anl_token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn init_principal_vault_handler(ctx: Context<InitPrincipalVault>) -> Result<()> {
    msg!("principal_vault: {}", ctx.accounts.principal_vault.key());
    Ok(())
}

#[derive(Accounts)]
pub struct InitRewardVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        has_one = authority @ AnlError::InvalidAuthority,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbców.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.anl_mint @ AnlError::InvalidMint)]
    pub anl_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = authority,
        seeds = [REWARD_VAULT_SEED],
        bump,
        token::mint = anl_mint,
        token::authority = vault_authority,
        token::token_program = anl_token_program
    )]
    pub reward_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub anl_token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn init_reward_vault_handler(ctx: Context<InitRewardVault>) -> Result<()> {
    msg!("reward_vault: {}", ctx.accounts.reward_vault.key());
    Ok(())
}

#[derive(Accounts)]
pub struct InitXntVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [GLOBAL_CONFIG_SEED],
        bump = global_config.bump,
        has_one = authority @ AnlError::InvalidAuthority,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion,
    )]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbców.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    #[account(address = global_config.xnt_mint @ AnlError::InvalidMint)]
    pub xnt_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        init,
        payer = authority,
        seeds = [XNT_VAULT_SEED],
        bump,
        token::mint = xnt_mint,
        token::authority = vault_authority,
        token::token_program = xnt_token_program
    )]
    pub xnt_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub xnt_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn init_xnt_vault_handler(ctx: Context<InitXntVault>) -> Result<()> {
    msg!("xnt_vault: {}", ctx.accounts.xnt_vault.key());
    Ok(())
}

#[derive(Accounts)]
pub struct InitCapyVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(mut, seeds = [GLOBAL_CONFIG_SEED], bump = global_config.bump,
        has_one = authority @ AnlError::InvalidAuthority,
        constraint = global_config.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion)]
    pub global_config: Box<Account<'info, GlobalConfig>>,

    /// CHECK: PDA-authority skarbcow.
    #[account(seeds = [VAULT_AUTHORITY_SEED], bump = global_config.vault_authority_bump)]
    pub vault_authority: UncheckedAccount<'info>,

    pub capy_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init, payer = authority, seeds = [CAPY_VAULT_SEED], bump,
        token::mint = capy_mint, token::authority = vault_authority,
        token::token_program = capy_token_program)]
    pub capy_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    pub capy_token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

pub fn init_capy_vault_handler(ctx: Context<InitCapyVault>) -> Result<()> {
    require!(ctx.accounts.capy_mint.decimals == 9, AnlError::InvalidMint);

    {
        let mint_ai = ctx.accounts.capy_mint.to_account_info();
        let data = mint_ai.try_borrow_data()?;
        let state =
            StateWithExtensions::<MintState>::unpack(&data).map_err(|_| AnlError::InvalidMint)?;
        let exts = state
            .get_extension_types()
            .map_err(|_| AnlError::InvalidMint)?;
        for e in exts {
            let dozwolone = matches!(
                e,
                ExtensionType::MetadataPointer | ExtensionType::TokenMetadata
            );
            if !dozwolone {
                msg!("CAPY mint: niedozwolone rozszerzenie {:?} (allowlist)", e);
                return err!(AnlError::InvalidMint);
            }
        }
        if state.base.freeze_authority.is_some() {
            msg!("CAPY mint ma aktywna freeze_authority - odrzucone");
            return err!(AnlError::InvalidMint);
        }
        // AUDYT R4 (M-02 raport C / L-03 raport B): "skończona pula
        // 20 000 000 CAPY" ma być inwariantem on-chain, nie obietnicą
        // off-chain — mint authority musi być wypalona, jak przy ANL.
        // Inaczej posiadacz authority dodrukowuje CAPY, permissionless
        // fund_capy pompuje available i rozwadnia entitlement claimów.
        if state.base.mint_authority.is_some() {
            msg!("CAPY mint ma aktywna mint_authority - odrzucone (pula 20M)");
            return err!(AnlError::MintHasMintAuthority);
        }
    }

    let cfg = &mut ctx.accounts.global_config;
    cfg.capy_mint = ctx.accounts.capy_mint.key();
    cfg.capy_vault_bump = ctx.bumps.capy_vault;
    msg!(
        "capy_vault: {} capy_mint: {}",
        ctx.accounts.capy_vault.key(),
        cfg.capy_mint
    );
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
