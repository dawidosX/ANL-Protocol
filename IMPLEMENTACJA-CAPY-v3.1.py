#!/usr/bin/env python3
"""
IMPLEMENTACJA CAPY v3.1 - rozdzielony claim + poprawki 3. audytu (GPT+Kimi).
Uruchom RAZ w czystym repo: python3 IMPLEMENTACJA-CAPY-v3.py

ARCHITEKTURA v3:
- CORE claim (ANL+XNT): NALICZA pending_capy, NIE transferuje CAPY.
  Awaria CAPY nie moze zablokowac ANL/XNT (problem rozwiazany u zrodla).
- OSOBNA instrukcja claim_capy: fizyczny transfer pending_capy.

POPRAWKI AUDYTU (wszystkie):
[C1] remaining_anl = ANL_REWARD_POOL - total_anl_paid (proporcja 0.10 stala)
[C2] CAPY POZA core claim (architektura, nie best-effort)
[H] decimals==9 + walidacja Token-2022 extensions (odrzuc PermanentDelegate/fee/hook)
[H] u128 min przed cast
[M] available_capy = vault - capy_reserved (brak podw. liczenia)
[M] invariant total_anl_paid <= POOL (guard)
"""
import sys, os
BASE = "programs/anl_staking/src"

def patch(path, old, new, opis, count=1):
    full = os.path.join(BASE, path)
    src = open(full, encoding="utf-8").read()
    n = src.count(old)
    if n != count:
        print(f"  !!! BLAD [{opis}]: wzorzec {n}x (oczekiwano {count}) w {path}")
        sys.exit(1)
    open(full, "w", encoding="utf-8").write(src.replace(old, new))
    print(f"  OK: {opis}")

def append(path, text, opis):
    with open(os.path.join(BASE, path), "a", encoding="utf-8") as f:
        f.write(text)
    print(f"  OK: {opis}")

def insert_after_struct(path, anchor, text, opis):
    full = os.path.join(BASE, path)
    src = open(full, encoding="utf-8").read()
    idx = src.find(anchor)
    if idx < 0:
        print(f"  !!! BLAD [{opis}]: nie znaleziono {anchor}"); sys.exit(1)
    end = src.find('}', idx) + 1
    open(full, "w", encoding="utf-8").write(src[:end] + "\n" + text + src[end:])
    print(f"  OK: {opis}")

print("=== IMPLEMENTACJA CAPY v3 (rozdzielony claim) ===\n")

# ============ 1. constants.rs ============
print("[1] constants.rs")
patch("constants.rs",
    'pub const XNT_VAULT_SEED: &[u8] = b"xnt_vault";',
    'pub const XNT_VAULT_SEED: &[u8] = b"xnt_vault";\npub const CAPY_VAULT_SEED: &[u8] = b"capy_vault";',
    "CAPY_VAULT_SEED")
append("constants.rs",
    '''
/// CAPY: calkowita planowana pula nagrod ANL w jednostkach bazowych (200M x 10^9).
/// remaining_anl = ANL_REWARD_POOL - total_anl_paid
/// entitlement = anl_reward * available_capy / remaining_anl
pub const ANL_REWARD_POOL: u128 = 200_000_000_000_000_000;
''',
    "ANL_REWARD_POOL")

# ============ 2. state ============
print("[2] state/mod.rs")
patch("state/mod.rs",
    '''    pub total_xnt_funded: u64,
    pub reserved: [u8; 16],
}''',
    '''    pub total_xnt_funded: u64,
    /// CAPY (v3) - Token-2022, trzecia waluta (rozdzielony claim).
    pub capy_mint: Pubkey,
    pub capy_vault_bump: u8,
    /// Suma wyplaconych nagrod ANL (monotoniczna, tylko udany claim).
    pub total_anl_paid: u64,
    /// Suma naliczonego, niewyplaconego CAPY (pending userow).
    pub capy_reserved: u64,
    pub reserved: [u8; 16],
}''',
    "GlobalConfig +capy")
patch("state/mod.rs",
    "pub const LEN: usize = 8 + 1 + 32 * 3 + 1 + 8 + 8 + 32 + 1 + 1 + 8 + 16;",
    "pub const LEN: usize = 8 + 1 + 32 * 3 + 1 + 8 + 8 + 32 + 1 + 1 + 8 + 32 + 1 + 8 + 8 + 16;",
    "GlobalConfig LEN")
patch("state/mod.rs",
    '''pub struct UserProfile {
    pub owner: Pubkey,
    pub next_position_index: u64,
    pub bump: u8,
    pub reserved: [u8; 7],
}''',
    '''pub struct UserProfile {
    pub owner: Pubkey,
    pub next_position_index: u64,
    pub bump: u8,
    /// CAPY (v3): naliczone, nieodebrane CAPY. Przezywa close pozycji.
    pub pending_capy: u64,
    pub reserved: [u8; 7],
}''',
    "UserProfile +pending_capy")
patch("state/mod.rs",
    "pub const LEN: usize = 8 + 32 + 8 + 1 + 7;",
    "pub const LEN: usize = 8 + 32 + 8 + 1 + 8 + 7;",
    "UserProfile LEN")

# ============ 3. init_capy_vault + walidacja Token-2022 ============
print("[3] initialize.rs - init_capy_vault (+Token-2022 policy)")
INIT_CAPY = '''
// ============================================================================
// init_capy_vault - skarbiec CAPY (Token-2022). Audyt: decimals==9 +
// walidacja rozszerzen (odrzuc PermanentDelegate/TransferFee/TransferHook).
// ============================================================================
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
    // decimals == 9 (skala jak ANL)
    require!(ctx.accounts.capy_mint.decimals == 9, AnlError::InvalidMint);

    // Walidacja Token-2022 (audyt v3.1): ALLOWLIST - tylko jawnie bezpieczne
    // rozszerzenia; cokolwiek innego (w tym przyszle) -> odrzut. + freeze==None.
    {
        use anchor_spl::token_2022::spl_token_2022::extension::BaseStateWithExtensions;
        use anchor_spl::token_2022::spl_token_2022::extension::ExtensionType;
        let mint_ai = ctx.accounts.capy_mint.to_account_info();
        let data = mint_ai.try_borrow_data()?;
        let state = StateWithExtensions::<MintState>::unpack(&data)
            .map_err(|_| AnlError::InvalidMint)?;
        let exts = state.get_extension_types().map_err(|_| AnlError::InvalidMint)?;
        for e in exts {
            let dozwolone = matches!(e,
                ExtensionType::MetadataPointer | ExtensionType::TokenMetadata);
            if !dozwolone {
                msg!("CAPY mint: niedozwolone rozszerzenie {:?} (allowlist)", e);
                return err!(AnlError::InvalidMint);
            }
        }
        // freeze_authority: bazowe pole mintu (audyt HIGH). Aktywna -> partner
        // moze zamrozic vault -> claim_capy blokowany na zawsze. Wymagamy None.
        if state.base.freeze_authority.is_some() {
            msg!("CAPY mint ma aktywna freeze_authority - odrzucone");
            return err!(AnlError::InvalidMint);
        }
        // (Droga B) mint_authority NIE sprawdzane - CAPY z natury moze miec
        // otwarty mint (partner); nie zagraza skarbcowi (dodruk != oproznienie).
    }

    let cfg = &mut ctx.accounts.global_config;
    cfg.capy_mint = ctx.accounts.capy_mint.key();
    cfg.capy_vault_bump = ctx.bumps.capy_vault;
    msg!("capy_vault: {} capy_mint: {}", ctx.accounts.capy_vault.key(), cfg.capy_mint);
    Ok(())
}
'''
patch("instructions/initialize.rs",
    '#[event]\npub struct ProtocolInitialized {',
    INIT_CAPY + "\n#[event]\npub struct ProtocolInitialized {",
    "init_capy_vault")

# ============ 4. fund_capy ============
print("[4] fund.rs - fund_capy")
FUND_CAPY = '''
// ------------------------------------------------------------ fund_capy
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
        CpiContext::new(ctx.accounts.capy_token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.funder_capy.to_account_info(),
                mint: ctx.accounts.capy_mint.to_account_info(),
                to: ctx.accounts.capy_vault.to_account_info(),
                authority: ctx.accounts.funder.to_account_info(),
            }),
        amount, ctx.accounts.capy_mint.decimals)?;
    ctx.accounts.capy_vault.reload()?;
    let net = ctx.accounts.capy_vault.amount.checked_sub(before)
        .ok_or(AnlError::MathOverflow)?;
    emit!(CapyFunded { amount_net: net, vault_balance: ctx.accounts.capy_vault.amount,
        timestamp: Clock::get()?.unix_timestamp });
    Ok(())
}

#[event]
pub struct CapyFunded { pub amount_net: u64, pub vault_balance: u64, pub timestamp: i64 }
'''
insert_after_struct("instructions/fund.rs", 'pub struct RewardsFunded {', FUND_CAPY, "fund_capy")

# ============ 5. Claim - dodaj konta (user_profile + capy_vault, BEZ transferu) ============
print("[5] lifecycle.rs - Claim: user_profile + capy_vault (odczyt), naliczanie")
patch("instructions/lifecycle.rs",
    '''    pub anl_token_program: Program<'info, Token2022>,
    pub xnt_token_program: Program<'info, Token>,

    /// CHECK: jak w SettleExpired \u2014 checkpoint ko\u0144ca end_epoch pozycji.
    pub xnt_checkpoint: Option<UncheckedAccount<'info>>,
}''',
    '''    pub anl_token_program: Program<'info, Token2022>,
    pub xnt_token_program: Program<'info, Token>,

    // --- CAPY v3: tylko ODCZYT salda + naliczenie do profilu (BEZ transferu) ---
    #[account(mut, seeds = [CAPY_VAULT_SEED], bump = global_config.capy_vault_bump)]
    pub capy_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, seeds = [USER_PROFILE_SEED, owner.key().as_ref()],
        bump = user_profile.bump)]
    pub user_profile: Box<Account<'info, UserProfile>>,

    /// CHECK: jak w SettleExpired \u2014 checkpoint ko\u0144ca end_epoch pozycji.
    pub xnt_checkpoint: Option<UncheckedAccount<'info>>,
}''',
    "Claim konta CAPY v3")

# ============ 6. Claim - logika naliczania pending_capy (BEZ transferu) ============
print("[6] lifecycle.rs - naliczanie pending_capy w claim")
patch("instructions/lifecycle.rs",
    '    // 3) principal z Principal Vault (I: nigdy ze skarbca nagr\u00f3d)',
    '''    // 2b) CAPY v3: NALICZ pending_capy (BEZ transferu - CAPY poza core claim).
    //     available = vault - capy_reserved (brak podw. liczenia already-obiecanego)
    //     entitlement = anl_reward * available / remaining_anl
    let total_anl_paid = ctx.accounts.global_config.total_anl_paid;
    let capy_reserved = ctx.accounts.global_config.capy_reserved;
    let remaining_anl = ANL_REWARD_POOL.checked_sub(total_anl_paid as u128).unwrap_or(0);
    let capy_entitlement: u64 = if remaining_anl == 0 || anl_reward == 0 {
        0
    } else {
        let vault_bal = ctx.accounts.capy_vault.amount as u128;
        let reserved = capy_reserved as u128;
        let available = vault_bal.saturating_sub(reserved);
        let full = (anl_reward as u128)
            .checked_mul(available).ok_or(AnlError::MathOverflow)?
            .checked_div(remaining_anl).ok_or(AnlError::MathOverflow)?;
        let capped = core::cmp::min(full, available);
        u64::try_from(capped).map_err(|_| AnlError::MathOverflow)?
    };

    // 3) principal z Principal Vault (I: nigdy ze skarbca nagr\u00f3d)''',
    "naliczanie pending_capy")

# inkrement total_anl_paid + capy_reserved + pending_capy (ksiegowanie claim)
patch("instructions/lifecycle.rs",
    '''    // ---- ksi\u0119gowanie ----
    let cfg = &mut ctx.accounts.global_config;
    cfg.anl_reward_reserved = cfg
        .anl_reward_reserved
        .checked_sub(anl_reward)
        .ok_or(AnlError::MathOverflow)?;''',
    '''    // ---- ksi\u0119gowanie ----
    let cfg = &mut ctx.accounts.global_config;
    cfg.anl_reward_reserved = cfg
        .anl_reward_reserved
        .checked_sub(anl_reward)
        .ok_or(AnlError::MathOverflow)?;
    // CAPY v3: zaksieguj naliczone CAPY (total_anl_paid + reserved + pending)
    cfg.total_anl_paid = cfg.total_anl_paid
        .checked_add(anl_reward).ok_or(AnlError::MathOverflow)?;
    // Tripwire (v3.1): MIEKKI czujnik. Gdyby invariant sie zlamal, NIE blokuj
    // core claim (ANL/XNT/principal userow) - tylko alarm. Immutable: zaden
    // bezpiecznik nie moze uwiezic srodkow userow.
    if (cfg.total_anl_paid as u128) > ANL_REWARD_POOL {
        emit!(InvariantAlarm {
            kind: 1, value: cfg.total_anl_paid,
            limit_hi: (ANL_REWARD_POOL >> 64) as u64, limit_lo: ANL_REWARD_POOL as u64,
            timestamp: now,
        });
    }
    if capy_entitlement > 0 {
        cfg.capy_reserved = cfg.capy_reserved
            .checked_add(capy_entitlement).ok_or(AnlError::MathOverflow)?;
        ctx.accounts.user_profile.pending_capy = ctx.accounts.user_profile.pending_capy
            .checked_add(capy_entitlement).ok_or(AnlError::MathOverflow)?;
    }''',
    "ksiegowanie CAPY v3")

# capy_reward w evencie (naliczone)
patch("instructions/lifecycle.rs",
    '''        anl_reward,
        xnt_reward: xnt_accrued,
        timestamp: now,
    });''',
    '''        anl_reward,
        xnt_reward: xnt_accrued,
        capy_reward: capy_entitlement,
        timestamp: now,
    });''',
    "capy_reward w emit")
patch("instructions/lifecycle.rs",
    '''    pub anl_reward: u64,
    pub xnt_reward: u64,
    pub timestamp: i64,
}''',
    '''    pub anl_reward: u64,
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
}''',
    "capy_reward + InvariantAlarm w evencie")

print("\n(v3 czesc 1/2 zaaplikowana - dalej claim_capy + lib.rs)")

# ============ 7. claim_capy - NOWA instrukcja (osobny transfer) ============
print("[7] lifecycle.rs - claim_capy (osobna instrukcja transferu)")
CLAIM_CAPY = '''
// ============================================================================
// claim_capy (v3) - OSOBNA instrukcja: fizyczny transfer naliczonego CAPY.
// Oddzielona od core claim -> awaria CAPY (frozen/hook) NIE dotyka ANL/XNT.
// Jak faila, cala claim_capy sie cofa, pending_capy zostaje (retry pozniej).
// ============================================================================
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

    // guard: nie wyplac wiecej niz jest fizycznie w skarbcu
    let vault_bal = ctx.accounts.capy_vault.amount;
    let to_pay = core::cmp::min(pending, vault_bal);
    require!(to_pay > 0, AnlError::ZeroAmount);

    let bump = ctx.accounts.global_config.vault_authority_bump;
    let seeds: &[&[u8]] = &[VAULT_AUTHORITY_SEED, &[bump]];
    let signer: &[&[&[u8]]] = &[seeds];

    // transfer CAPY (normalny ? - jak faila, cala instrukcja sie cofa,
    // pending_capy NIE zmienione, ANL/XNT juz dawno bezpieczne)
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

    // ksiegowanie PO udanym transferze
    ctx.accounts.user_profile.pending_capy = ctx.accounts.user_profile.pending_capy
        .checked_sub(to_pay).ok_or(AnlError::MathOverflow)?;
    ctx.accounts.global_config.capy_reserved = ctx.accounts.global_config.capy_reserved
        .checked_sub(to_pay).ok_or(AnlError::MathOverflow)?;

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
'''
append("instructions/lifecycle.rs", CLAIM_CAPY, "claim_capy")

# ============ 8. lib.rs delegacje ============
print("[8] lib.rs - delegacje")
patch("lib.rs",
    '''    pub fn init_xnt_vault(ctx: Context<InitXntVault>) -> Result<()> {
        instructions::initialize::init_xnt_vault_handler(ctx)
    }''',
    '''    pub fn init_xnt_vault(ctx: Context<InitXntVault>) -> Result<()> {
        instructions::initialize::init_xnt_vault_handler(ctx)
    }

    pub fn init_capy_vault(ctx: Context<InitCapyVault>) -> Result<()> {
        instructions::initialize::init_capy_vault_handler(ctx)
    }''',
    "init_capy_vault delegacja")
patch("lib.rs",
    '''    pub fn fund_rewards(ctx: Context<FundRewards>, amount: u64) -> Result<()> {
        instructions::fund::fund_rewards(ctx, amount)
    }''',
    '''    pub fn fund_rewards(ctx: Context<FundRewards>, amount: u64) -> Result<()> {
        instructions::fund::fund_rewards(ctx, amount)
    }

    pub fn fund_capy(ctx: Context<FundCapy>, amount: u64) -> Result<()> {
        instructions::fund::fund_capy(ctx, amount)
    }''',
    "fund_capy delegacja")
patch("lib.rs",
    '''    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        instructions::lifecycle::claim(ctx)
    }''',
    '''    pub fn claim(ctx: Context<Claim>) -> Result<()> {
        instructions::lifecycle::claim(ctx)
    }

    pub fn claim_capy(ctx: Context<ClaimCapy>) -> Result<()> {
        instructions::lifecycle::claim_capy(ctx)
    }''',
    "claim_capy delegacja")

print("\n=== CAPY v3 ZAAPLIKOWANE (rozdzielony claim, wszystkie poprawki) ===")
print("Nastepnie: cargo-build-sbf --features network-testnet")
