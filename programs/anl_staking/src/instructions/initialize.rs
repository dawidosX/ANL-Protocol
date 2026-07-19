//! initialize — TC-001…TC-006. Tworzy GlobalConfig, VaultAuthority i trzy skarbce.
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

pub fn initialize_handler(ctx: Context<Initialize>, genesis_start_ts: i64, start_paused: bool) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    // Twarda kotwica minta XNT (wrapped native). W buildzie testowym
    // wyłączona — testy używają mintów lokalnych; osobny Program ID.
    #[cfg(not(feature = "test-periods"))]
    require_keys_eq!(
        ctx.accounts.xnt_mint.key(),
        EXPECTED_XNT_MINT,
        AnlError::InvalidXntMint
    );

    // ---- Audyt pkt 4: allowlista rozszerzeń Token-2022 minta ANL ----
    // Odrzucamy wszystko, co daje administracyjną ścieżkę do tokenów w vaultach
    // (PermanentDelegate) lub wpina nieaudytowany kod w transfery (TransferHook),
    // lub zmienia semantykę wypłat. Dozwolone wyłącznie rozszerzenia pasywne.
    {
        let ai = ctx.accounts.anl_mint.to_account_info();
        let data = ai.try_borrow_data()?;
        let state = StateWithExtensions::<MintState>::unpack(&data)
            .map_err(|_| error!(AnlError::InvalidMint))?;
        require!(
            state.base.freeze_authority.is_none(),
            AnlError::MintHasFreezeAuthority
        );
        // Fixed supply: mint authority musi być odwołane (audyt #2)
        require!(
            state.base.mint_authority.is_none(),
            AnlError::MintHasMintAuthority
        );
        for ext in state
            .get_extension_types()
            .map_err(|_| error!(AnlError::InvalidMint))?
        {
            match ext {
                // pasywne metadane — bez wpływu na transfery i salda
                ExtensionType::MetadataPointer | ExtensionType::TokenMetadata => {}
                // wszystko inne (PermanentDelegate, TransferHook, TransferFeeConfig,
                // DefaultAccountState, NonTransferable, przyszłe) — odrzucone
                _ => return err!(AnlError::ForbiddenMintExtension),
            }
        }
    }

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
    cfg.operator = ctx.accounts.authority.key();
    cfg.bump = ctx.bumps.global_config;
    cfg.vault_authority_bump = ctx.bumps.vault_authority;
    cfg.reserved = [0; 24];

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
