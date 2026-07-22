//! ANL Staking Protocol — deterministyczna matematyka nagród (WP v1.0: dzienne XNT, okres deklarowany).
//!
//! Crate bez zależności: te same funkcje wykorzystuje program on-chain
//! oraz model referencyjny do differential testingu (Volume 10F §29).
//! Wszystkie dzielenia zaokrąglają W DÓŁ; dust pozostaje w vaultach (TC-126).

#![deny(unsafe_code)]

pub const PRECISION: u128 = 1_000_000_000_000;
pub const BPS_DENOMINATOR: u128 = 10_000;
pub const SECONDS_PER_DAY: i64 = 86_400;
pub const SECONDS_PER_YEAR: i64 = 365 * SECONDS_PER_DAY; // D-3

pub const APY_FLEXIBLE_BPS: u16 = 800;
pub const APY_GENESIS_W1_BPS: u16 = 2_000;
pub const APY_GENESIS_W2_BPS: u16 = 1_500;
pub const APY_GENESIS_W3_BPS: u16 = 800;

/// Granice okien Genesis w sekundach od genesis_start_ts; przedziały [a, b) — D-2.
/// WP v1.0 §5: dni 0–30 → 20%, dni 31–90 → 15%, od dnia 91 → 8%
/// („do końca dnia 30" ⇒ granica = początek dnia 31).
///
/// Feature `test-periods` (WYŁĄCZNIE testnet — nigdy mainnet): okna skrócone
/// do 0–2 / 3–8 / od 9 dnia, min. okres pozycji 1 dzień. Wybór w compile-time;
/// stałe produkcyjne nigdy nie są konfigurowalne w runtime.
#[cfg(not(feature = "test-periods"))]
pub const WINDOW_1_END: i64 = 31 * SECONDS_PER_DAY;
#[cfg(feature = "test-periods")]
pub const WINDOW_1_END: i64 = 3 * SECONDS_PER_DAY;
#[cfg(not(feature = "test-periods"))]
pub const WINDOW_2_END: i64 = 91 * SECONDS_PER_DAY;
#[cfg(feature = "test-periods")]
pub const WINDOW_2_END: i64 = 9 * SECONDS_PER_DAY;

/// Okres pozycji deklarowany przez uczestnika — OBA programy (WP v1.0 §7).
#[cfg(not(feature = "test-periods"))]
pub const MIN_PERIOD_DAYS: i64 = 7;
#[cfg(feature = "test-periods")]
pub const MIN_PERIOD_DAYS: i64 = 1;
pub const MAX_PERIOD_DAYS: i64 = 3_650;

/// WP okna Genesis: okienkowa wypłata XNT co pełny blok N-dniowy liczony od
/// genesis protokołu. Produkcyjnie 30 dni; w `test-periods` skrócone do 3 dni
/// (spójnie z pozostałymi oknami), by suite mógł zweryfikować kumulację bez
/// przewijania 30 dób symulowanego czasu.
#[cfg(not(feature = "test-periods"))]
pub const GENESIS_WINDOW_DAYS: u64 = 30;
#[cfg(feature = "test-periods")]
pub const GENESIS_WINDOW_DAYS: u64 = 3;

pub const XNT_SHARE_GENESIS_BPS: u128 = 6_500; // Flexible = reszta (35%)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathError {
    Overflow,
    DivisionByZero,
    NegativeTime,
    IndexUnderflow,
}

pub type MathResult<T> = Result<T, MathError>;

/// APY Genesis wg okna wejścia (TC-040…044). Sekunda graniczna → NOWE okno (D-2).
pub fn genesis_apy_bps(elapsed_since_genesis_start: i64) -> MathResult<u16> {
    if elapsed_since_genesis_start < 0 {
        return Err(MathError::NegativeTime);
    }
    Ok(if elapsed_since_genesis_start < WINDOW_1_END {
        APY_GENESIS_W1_BPS
    } else if elapsed_since_genesis_start < WINDOW_2_END {
        APY_GENESIS_W2_BPS
    } else {
        APY_GENESIS_W3_BPS
    })
}

/// Nagroda za okres o stałym APY (Genesis: lock; Flexible: zadeklarowane dni — D-13).
/// reward = amount × apy_bps × period_seconds / (BPS × SECONDS_PER_YEAR), floor.
pub fn period_reward(amount: u64, apy_bps: u16, period_seconds: i64) -> MathResult<u64> {
    if period_seconds < 0 {
        return Err(MathError::NegativeTime);
    }
    let numerator = (amount as u128)
        .checked_mul(apy_bps as u128)
        .ok_or(MathError::Overflow)?
        .checked_mul(period_seconds as u128)
        .ok_or(MathError::Overflow)?;
    let denominator = BPS_DENOMINATOR
        .checked_mul(SECONDS_PER_YEAR as u128)
        .ok_or(MathError::Overflow)?;
    u64::try_from(numerator / denominator).map_err(|_| MathError::Overflow)
}

/// Podział NETTO fundingu XNT: Genesis 65%, Flexible = reszta (TC-070/071; zero dust — I5).
pub fn split_xnt(net_amount: u64) -> (u64, u64) {
    let genesis = ((net_amount as u128) * XNT_SHARE_GENESIS_BPS / BPS_DENOMINATOR) as u64;
    (genesis, net_amount - genesis)
}

/// Aktualizacja indeksu XNT (TC-120 monotoniczny; TC-129 shares==0 → błąd,
/// obsługiwany na poziomie instrukcji przez `xnt_undistributed` — D-5).
pub fn update_xnt_index(
    current_index: u128,
    distributed_net: u64,
    total_shares: u64,
) -> MathResult<u128> {
    if total_shares == 0 {
        return Err(MathError::DivisionByZero);
    }
    let delta = (distributed_net as u128)
        .checked_mul(PRECISION)
        .ok_or(MathError::Overflow)?
        / (total_shares as u128);
    current_index.checked_add(delta).ok_or(MathError::Overflow)
}

/// Naliczone XNT pozycji (TC-121/124 — debt snapshot).
/// Zakres: shares ≤ Sᵢ przy każdym fundingu ⇒ shares×Σ(dᵢ×P/Sᵢ) ≤ P×Σdᵢ < u128::MAX.
pub fn pending_xnt(shares: u64, pool_index: u128, position_debt_index: u128) -> MathResult<u64> {
    let diff = pool_index
        .checked_sub(position_debt_index)
        .ok_or(MathError::IndexUnderflow)?;
    let pending = (shares as u128)
        .checked_mul(diff)
        .ok_or(MathError::Overflow)?
        / PRECISION;
    u64::try_from(pending).map_err(|_| MathError::Overflow)
}

/// Koniec zadeklarowanego okresu pozycji (end_ts).
pub fn period_end_ts(start_ts: i64, period_seconds: i64) -> MathResult<i64> {
    if period_seconds < 0 {
        return Err(MathError::NegativeTime);
    }
    start_ts
        .checked_add(period_seconds)
        .ok_or(MathError::Overflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_ANL: u64 = 1_000_000_000; // decimals = 9
    const MILLION_ANL: u64 = 1_000_000 * ONE_ANL;

    #[test]
    fn tc_040_window_one_gives_20_percent() {
        assert_eq!(genesis_apy_bps(0).unwrap(), 2_000);
        assert_eq!(genesis_apy_bps(WINDOW_1_END - 1).unwrap(), 2_000); // koniec dnia 30
    }

    #[test]
    fn tc_041_exact_start_of_day_31_belongs_to_new_window() {
        assert_eq!(genesis_apy_bps(WINDOW_1_END).unwrap(), 1_500);
    }

    #[test]
    fn tc_042_days_31_to_90_inclusive_give_15_percent() {
        assert_eq!(genesis_apy_bps(WINDOW_1_END + 1).unwrap(), 1_500);
        assert_eq!(genesis_apy_bps(WINDOW_2_END - 1).unwrap(), 1_500);
    }

    #[test]
    fn tc_043_exact_start_of_day_91_belongs_to_new_window() {
        assert_eq!(genesis_apy_bps(WINDOW_2_END).unwrap(), 800);
    }

    #[test]
    fn tc_044_from_day_91_gives_8_percent() {
        assert_eq!(genesis_apy_bps(WINDOW_2_END + 123_456).unwrap(), 800);
        assert_eq!(genesis_apy_bps(i64::MAX).unwrap(), 800);
    }

    /// STRAŻNIK PRODUKCJI (audyt pkt 5): build bez feature MUSI mieć
    /// parametry produkcyjne. CI odpala ten test na wariancie domyślnym —
    /// artefakt release, który go nie przechodzi, nie jest artefaktem
    /// produkcyjnym. Procedura: osobny Program ID dla buildów test-periods.
    #[test]
    #[cfg(not(feature = "test-periods"))]
    fn production_constants_guard() {
        assert_eq!(WINDOW_1_END, 31 * SECONDS_PER_DAY);
        assert_eq!(WINDOW_2_END, 91 * SECONDS_PER_DAY);
        assert_eq!(MIN_PERIOD_DAYS, 7);
        assert_eq!(genesis_apy_bps(30 * SECONDS_PER_DAY).unwrap(), 2_000);
        assert_eq!(genesis_apy_bps(31 * SECONDS_PER_DAY).unwrap(), 1_500);
        assert_eq!(genesis_apy_bps(91 * SECONDS_PER_DAY).unwrap(), 800);
    }

    #[test]
    #[cfg(feature = "test-periods")]
    fn test_periods_feature_uses_short_windows() {
        assert_eq!(WINDOW_1_END, 3 * SECONDS_PER_DAY);
        assert_eq!(WINDOW_2_END, 9 * SECONDS_PER_DAY);
        assert_eq!(MIN_PERIOD_DAYS, 1);
        assert_eq!(genesis_apy_bps(2 * SECONDS_PER_DAY).unwrap(), 2_000);
        assert_eq!(genesis_apy_bps(3 * SECONDS_PER_DAY).unwrap(), 1_500);
        assert_eq!(genesis_apy_bps(9 * SECONDS_PER_DAY).unwrap(), 800);
    }

    #[test]
    fn tc_242_negative_time_is_rejected() {
        assert_eq!(genesis_apy_bps(-1), Err(MathError::NegativeTime));
    }

    #[test]
    fn genesis_1m_anl_at_20_percent_for_365_days_yields_200k() {
        let r = period_reward(MILLION_ANL, 2_000, 365 * SECONDS_PER_DAY).unwrap();
        assert_eq!(r, 200_000 * ONE_ANL);
    }

    #[test]
    fn flexible_1m_anl_at_8_percent_for_100_days() {
        // 1 000 000 × 8% × 100/365 = 21 917,808219178… ANL (floor w bazowych jednostkach)
        let r = period_reward(MILLION_ANL, 800, 100 * SECONDS_PER_DAY).unwrap();
        assert_eq!(r, 21_917_808_219_178);
    }

    #[test]
    fn tc_048_same_rate_reward_scales_linearly_with_time() {
        let r90 = period_reward(MILLION_ANL, 2_000, 90 * SECONDS_PER_DAY).unwrap();
        let r365 = period_reward(MILLION_ANL, 2_000, 365 * SECONDS_PER_DAY).unwrap();
        assert!(r90 < r365);
        // 90-dniowa nagroda × (365/90) ≈ roczna (z dokładnością floor)
        assert!((r90 as u128 * 365 / 90).abs_diff(r365 as u128) <= 365);
    }

    #[test]
    fn tc_031_minimum_amounts_floor_to_dust_not_panic() {
        assert_eq!(period_reward(1, 800, SECONDS_PER_YEAR).unwrap(), 0);
        assert_eq!(
            period_reward(ONE_ANL, 800, SECONDS_PER_YEAR).unwrap(),
            ONE_ANL * 8 / 100
        );
    }

    #[test]
    fn tc_127_max_amount_checked_not_panicking() {
        // realny maks: u64::MAX przy 20% za 1 rok = MAX/5 — mieści się w u64
        let r = period_reward(u64::MAX, 2_000, SECONDS_PER_YEAR).unwrap();
        assert_eq!(r, u64::MAX / 5);
        // wynik ponad zakres u64 (20% × 10 lat = 2× principal) → kontrolowany błąd, nie panic
        assert_eq!(
            period_reward(u64::MAX, 2_000, 10 * SECONDS_PER_YEAR),
            Err(MathError::Overflow)
        );
    }

    #[test]
    fn tc_126_reward_never_rounds_up() {
        for secs in [1i64, 59, 3_601, SECONDS_PER_DAY + 1] {
            let r = period_reward(999_999_999, 777, secs).unwrap() as u128;
            let num = 999_999_999u128 * 777 * secs as u128;
            let den = BPS_DENOMINATOR * SECONDS_PER_YEAR as u128;
            assert!(r * den <= num && (r + 1) * den > num);
        }
    }

    #[test]
    fn tc_120_xnt_index_is_monotonic() {
        let mut idx = 0u128;
        for (dist, shares) in [(500u64, 100u64), (1, 1_000_000), (u64::MAX / 2, 7)] {
            let next = update_xnt_index(idx, dist, shares).unwrap();
            assert!(next >= idx);
            idx = next;
        }
    }

    #[test]
    fn tc_129_zero_shares_division_guard() {
        assert_eq!(
            update_xnt_index(1_000, 500, 0),
            Err(MathError::DivisionByZero)
        );
    }

    #[test]
    fn tc_122_two_equal_users_receive_equal_rewards() {
        let idx = update_xnt_index(0, 1_000, 1_000).unwrap();
        let a = pending_xnt(500, idx, 0).unwrap();
        let b = pending_xnt(500, idx, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a + b, 1_000);
    }

    #[test]
    fn tc_123_rewards_proportional_to_shares() {
        let idx = update_xnt_index(0, 3_000, 3_000).unwrap();
        assert_eq!(
            pending_xnt(2_000, idx, 0).unwrap(),
            pending_xnt(1_000, idx, 0).unwrap() * 2
        );
    }

    #[test]
    fn tc_124_late_staker_gets_nothing_from_earlier_funding() {
        let idx1 = update_xnt_index(0, 1_000, 100).unwrap();
        let b_debt = idx1; // B wchodzi po pierwszym fundingu
        let idx2 = update_xnt_index(idx1, 900, 300).unwrap();
        assert_eq!(pending_xnt(200, idx2, b_debt).unwrap(), 600);
    }

    #[test]
    fn tc_126_index_dust_never_creates_tokens() {
        let idx = update_xnt_index(0, 10, 3).unwrap();
        let total: u64 = (0..3).map(|_| pending_xnt(1, idx, 0).unwrap()).sum();
        assert_eq!(total, 9); // 1 jednostka dust zostaje w vaulcie, nigdy > 10
    }

    #[test]
    fn tc_243_debt_above_index_is_underflow_error() {
        assert_eq!(pending_xnt(10, 5, 6), Err(MathError::IndexUnderflow));
    }

    #[test]
    fn xnt_range_safety_under_extreme_accumulation() {
        let mut idx = 0u128;
        for _ in 0..4 {
            idx = update_xnt_index(idx, u64::MAX / 4, 1).unwrap();
        }
        assert!(pending_xnt(1, idx, 0).unwrap() <= u64::MAX - 3);
    }

    #[test]
    fn tc_070_071_split_conserves_total_and_is_65_percent_floor() {
        for total in [0u64, 1, 99, 10_000, u64::MAX] {
            let (g, f) = split_xnt(total);
            assert_eq!(g as u128 + f as u128, total as u128);
            assert_eq!(g as u128, (total as u128) * 6_500 / 10_000);
        }
    }

    #[test]
    fn tc_045_047_declared_period_end_timestamps() {
        assert_eq!(
            period_end_ts(1_000, 7 * SECONDS_PER_DAY).unwrap(),
            1_000 + 7 * 86_400
        );
        assert_eq!(
            period_end_ts(1_000, 60 * SECONDS_PER_DAY).unwrap(),
            1_000 + 60 * 86_400
        );
        assert_eq!(period_end_ts(i64::MAX, 1), Err(MathError::Overflow));
    }

    #[test]
    fn wp_example_genesis_60_days_window1() {
        // WP §5: 100 000 ANL × 20% × 60/365 ≈ 3 288 ANL (floor)
        let r = period_reward(100_000 * ONE_ANL, 2_000, 60 * SECONDS_PER_DAY).unwrap();
        assert_eq!(r, 3_287_671_232_876);
    }
}
