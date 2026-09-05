//! Seeds PDA (spec v1.0, sekcja 4) i limity.

pub const GLOBAL_CONFIG_SEED: &[u8] = b"global_config";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault_authority";
pub const POOL_SEED: &[u8] = b"pool";
pub const PRINCIPAL_VAULT_SEED: &[u8] = b"principal_vault";
pub const REWARD_VAULT_SEED: &[u8] = b"reward_vault";
pub const XNT_VAULT_SEED: &[u8] = b"xnt_vault";
pub const CAPY_VAULT_SEED: &[u8] = b"capy_vault";
pub const USER_PROFILE_SEED: &[u8] = b"profile";
pub const USER_POSITION_SEED: &[u8] = b"position";

/// Min. stake = 1 ANL w jednostkach bazowych (D-7 ✅; decimals ustalane przy wdrożeniu).
pub const MIN_STAKE_AMOUNT: u64 = 1_000_000_000;
/// Okres pozycji deklarowany przez uczestnika — OBA programy (WP v1.0 §7).
/// Źródłem prawdy jest `anl-math` (tam działa feature `test-periods`).
pub const MIN_PERIOD_DAYS: u32 = anl_math::MIN_PERIOD_DAYS as u32;
pub const MAX_PERIOD_DAYS: u32 = anl_math::MAX_PERIOD_DAYS as u32;
pub const MAX_PERIOD_DAYS_FLEXIBLE: u32 = anl_math::MAX_PERIOD_DAYS_FLEXIBLE as u32;
pub const EARLY_EXIT_COOLDOWN_SECS: i64 = anl_math::EARLY_EXIT_COOLDOWN_SECS;

pub const XNT_SHARE_GENESIS_BPS: u16 = 6_500;
pub const XNT_SHARE_FLEXIBLE_BPS: u16 = 3_500;

/// Checkpointy epok XNT (audyt #1/#2): snapshot indeksu puli po każdej
/// epoce, w której nastąpił funding. Seeds: [SEED, pool_type, epoch_le].
pub const XNT_CKPT_SEED: &[u8] = b"xnt_ckpt";
/// Sentinel "brak epoki" (pool nigdy nie fundowany / checkpoint ostatni).
pub const NO_EPOCH: u64 = u64::MAX;
/// Oczekiwany mint XNT (wrapped native X1) — twarda kotwica produkcyjna.
/// W buildzie test-periods kontrola wyłączona (testy używają mintów lokalnych).
/// AUDYT R6 (I-01 / HIGH raport C): `initialize` był first-come — pierwszy
/// wołający po deployu zostawał authority. W buildach produkcyjnych (bez
/// test-periods) inicjalizator MUSI być tym kluczem. MAINNET: podmienić na
/// klucz multisig w ceremonii deployu (zmiana stałej = nowy sha binarki,
/// zapisany w manifeście). Testy używają kluczy lokalnych (kontrola wyłączona).
pub const EXPECTED_INIT_AUTHORITY: anchor_lang::prelude::Pubkey =
    anchor_lang::prelude::Pubkey::new_from_array([
        192, 101, 91, 47, 117, 27, 123, 67, 38, 66, 131, 241, 109, 94, 194, 5, 23, 144, 50, 133,
        45, 200, 40, 252, 149, 251, 250, 2, 89, 29, 151, 170,
    ]);
pub const EXPECTED_XNT_MINT: anchor_lang::prelude::Pubkey =
    anchor_lang::prelude::Pubkey::new_from_array([
        6, 155, 136, 87, 254, 171, 129, 132, 251, 104, 127, 99, 70, 24, 192, 53, 218, 196, 57, 220,
        26, 235, 59, 85, 152, 160, 240, 0, 0, 0, 0, 1,
    ]);

/// CAPY: calkowita planowana pula nagrod ANL w jednostkach bazowych (200M x 10^9).
/// remaining_anl = ANL_REWARD_POOL - total_anl_paid
/// entitlement = anl_reward * available_capy / remaining_anl
pub const ANL_REWARD_POOL: u128 = 200_000_000_000_000_000;

/// AUDYT R7.2 (C): tokenomika WP jako inwariant on-chain — całkowita podaż CAPY
/// = 20 000 000 × 10^9 (decimals 9). Sprawdzana w `init_capy_vault` WYŁĄCZNIE w
/// buildzie `network-mainnet` (authority wypalona ⇒ podaż niezmienna, więc
/// jednorazowa kontrola przy setupie wystarcza). Testnet (1B CAPY testowych)
/// bez tej bramki.
pub const CAPY_TOTAL_SUPPLY: u64 = 20_000_000 * 1_000_000_000;

/// AUDYT R7 — strażniki kompilowane z feature sieciowym (CI: `cargo test -p
/// anl_staking --lib --features network-testnet,test-periods` oraz
/// `--features network-mainnet`). Test pinu `initialize` nie jest możliwy w
/// suite integracyjnej (bramka działa tylko w buildach sieciowych), więc
/// dowodem jest asercja na WARTOŚĆ stałej: zmiana klucza w ceremonii mainnet
/// (multisig) MUSI zmienić oczekiwany string poniżej w tym samym commicie.
#[cfg(all(test, feature = "network-testnet"))]
mod init_authority_guard_testnet {
    #[test]
    fn init_authority_pinned_testnet() {
        assert_eq!(
            super::EXPECTED_INIT_AUTHORITY.to_string(),
            "Dx2vEpVdMh2qScz4vEHAXquTm6QYocKbmPRdcXHLzvEm",
            "testnet: EXPECTED_INIT_AUTHORITY = klucz deployera testnetu"
        );
    }
}

#[cfg(all(test, feature = "network-mainnet"))]
mod capy_supply_guard_mainnet {
    #[test]
    fn capy_supply_constant() {
        assert_eq!(
            super::CAPY_TOTAL_SUPPLY,
            20_000_000_000_000_000,
            "mainnet: podaz CAPY 20M x 10^9 (WP)"
        );
    }
}

#[cfg(all(test, feature = "network-mainnet"))]
mod init_authority_guard_mainnet {
    /// MAINNET: dziś ta sama stała co testnet — ceremonia deployu podmienia
    /// ją na multisig i aktualizuje ten string (sha binarki w manifeście).
    #[test]
    fn init_authority_pinned_mainnet() {
        assert_eq!(
            super::EXPECTED_INIT_AUTHORITY.to_string(),
            "Dx2vEpVdMh2qScz4vEHAXquTm6QYocKbmPRdcXHLzvEm",
            "mainnet: przed ceremonia podmien na multisig i zaktualizuj straznika"
        );
    }
}
