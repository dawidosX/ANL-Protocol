//! Seeds PDA (spec v1.0, sekcja 4) i limity.

pub const GLOBAL_CONFIG_SEED: &[u8] = b"global_config";
pub const VAULT_AUTHORITY_SEED: &[u8] = b"vault_authority";
pub const POOL_SEED: &[u8] = b"pool";
pub const PRINCIPAL_VAULT_SEED: &[u8] = b"principal_vault";
pub const REWARD_VAULT_SEED: &[u8] = b"reward_vault";
pub const XNT_VAULT_SEED: &[u8] = b"xnt_vault";
pub const USER_PROFILE_SEED: &[u8] = b"profile";
pub const USER_POSITION_SEED: &[u8] = b"position";

/// Min. stake = 1 ANL w jednostkach bazowych (D-7 ✅; decimals ustalane przy wdrożeniu).
pub const MIN_STAKE_AMOUNT: u64 = 1_000_000_000;
/// Okres pozycji deklarowany przez uczestnika — OBA programy (WP v1.0 §7).
/// Źródłem prawdy jest `anl-math` (tam działa feature `test-periods`).
pub const MIN_PERIOD_DAYS: u32 = anl_math::MIN_PERIOD_DAYS as u32;
pub const MAX_PERIOD_DAYS: u32 = anl_math::MAX_PERIOD_DAYS as u32;

pub const XNT_SHARE_GENESIS_BPS: u16 = 6_500;
pub const XNT_SHARE_FLEXIBLE_BPS: u16 = 3_500;
