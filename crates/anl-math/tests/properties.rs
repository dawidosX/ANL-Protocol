//! Testy PROPERTY-BASED matematyki nagród (rekomendacja audytów #1–#3).
//!
//! Losowe wejścia (proptest) sprawdzają INWARIANTY zamiast pojedynczych
//! wartości: zachowanie sumy przy podziale 65/35, monotoniczność indeksu,
//! brak inflacji przy wypłatach (nikt nie dostaje więcej, niż wpłacono),
//! ograniczone straty zaokrągleń, liniowość nagrody okresowej.
//! Uzupełniają — nie zastępują — testy jednostkowe TC-xxx.

use anl_math::*;
use proptest::prelude::*;

proptest! {
    // ---------------------------------------------------------- split_xnt

    /// Suma części ZAWSZE równa całości (zero gubienia i zero dodruku).
    #[test]
    fn split_conserves_total(net in any::<u64>()) {
        let (g, f) = split_xnt(net);
        prop_assert_eq!(g as u128 + f as u128, net as u128);
    }

    /// Część Genesis to dokładnie floor(net * 65%), Flexible — reszta.
    #[test]
    fn split_matches_bps(net in any::<u64>()) {
        let (g, _f) = split_xnt(net);
        prop_assert_eq!(g as u128, (net as u128) * XNT_SHARE_GENESIS_BPS / BPS_DENOMINATOR);
    }

    /// Monotoniczność: więcej wpływu ⇒ żadna część nie maleje.
    #[test]
    fn split_monotone(a in any::<u64>(), b in any::<u64>()) {
        prop_assume!(a <= b);
        let (ga, fa) = split_xnt(a);
        let (gb, fb) = split_xnt(b);
        prop_assert!(ga <= gb && fa <= fb);
    }

    // ---------------------------------------------------- update_xnt_index

    /// Indeks NIGDY nie maleje (TC-120 uogólnione na losowe wejścia).
    #[test]
    fn index_monotone(
        idx in any::<u128>(),
        dist in any::<u64>(),
        shares in 1u64..,
    ) {
        if let Ok(next) = update_xnt_index(idx, dist, shares) {
            prop_assert!(next >= idx);
        } // Overflow indeksu to poprawna odmowa, nie regres monotoniczności.
    }

    /// shares == 0 ⇒ zawsze błąd (dystrybucję w pustym koszyku trzyma
    /// `undistributed` na poziomie instrukcji — D-5).
    #[test]
    fn index_zero_shares_rejected(idx in any::<u128>(), dist in any::<u64>()) {
        prop_assert!(update_xnt_index(idx, dist, 0).is_err());
    }

    // ------------------------------------------- pending_xnt: brak inflacji

    /// WŁASNOŚĆ BEZPIECZEŃSTWA: przy jednym fundingu `dist` żadna pozycja
    /// (shares ≤ total_shares z chwili fundingu) nie naliczy więcej niż
    /// `dist`, a strata zaokrągleń całego koszyka jest ograniczona:
    /// dist − pending(total) ≤ total_shares/PRECISION + 2.
    #[test]
    fn pending_no_inflation_and_bounded_loss(
        idx0 in 0u128..(u128::MAX / 4),
        dist in any::<u64>(),
        total in 1u64..,
        frac in 0u64..=u64::MAX,
    ) {
        let idx1 = match update_xnt_index(idx0, dist, total) {
            Ok(v) => v,
            Err(_) => return Ok(()), // overflow indeksu — poprawna odmowa
        };
        let shares = if frac >= total { total } else { frac }; // shares ≤ total
        let p = pending_xnt(shares, idx1, idx0).unwrap();
        prop_assert!(p <= dist, "inflacja: pending {} > dist {}", p, dist);

        let all = pending_xnt(total, idx1, idx0).unwrap();
        prop_assert!(all <= dist);
        let loss = dist - all;
        let bound = (total as u128 / PRECISION) + 2;
        prop_assert!(
            (loss as u128) <= bound,
            "strata zaokrągleń {} > granica {}", loss, bound
        );
    }

    /// debt > index ⇒ jawny błąd (IndexUnderflow), nigdy cicha wartość.
    #[test]
    fn pending_debt_ahead_rejected(
        shares in any::<u64>(),
        idx in any::<u128>(),
        ahead in 1u128..1_000_000_000u128,
    ) {
        prop_assume!(idx <= u128::MAX - ahead);
        prop_assert!(pending_xnt(shares, idx, idx + ahead).is_err());
    }

    // ---------------------------------------------------- period_reward

    /// Liniowość w kwocie z tolerancją floor: r(a)+r(b) ≤ r(a+b) ≤ r(a)+r(b)+1.
    #[test]
    fn reward_superadditive_within_one(
        a in 0u64..=u64::MAX / 2,
        b in 0u64..=u64::MAX / 2,
        apy in 0u16..=3_000,
        secs in 0i64..=100 * 366 * SECONDS_PER_DAY,
    ) {
        let (ra, rb, rab) = match (
            period_reward(a, apy, secs),
            period_reward(b, apy, secs),
            period_reward(a + b, apy, secs),
        ) {
            (Ok(x), Ok(y), Ok(z)) => (x, y, z),
            _ => return Ok(()), // overflow — poprawna odmowa
        };
        let lo = ra as u128 + rb as u128;
        prop_assert!(lo <= rab as u128 && (rab as u128) <= lo + 1);
    }

    /// Monotoniczność nagrody we wszystkich trzech argumentach.
    #[test]
    fn reward_monotone(
        a in 0u64..=u64::MAX / 2,
        apy in 0u16..=3_000,
        secs in 0i64..=100 * 366 * SECONDS_PER_DAY,
    ) {
        if let (Ok(r0), Ok(r_amt), Ok(r_apy), Ok(r_sec)) = (
            period_reward(a, apy, secs),
            period_reward(a.saturating_add(1), apy, secs),
            period_reward(a, apy.saturating_add(1), secs),
            period_reward(a, apy, secs.saturating_add(1)),
        ) {
            prop_assert!(r_amt >= r0 && r_apy >= r0 && r_sec >= r0);
        }
    }

    // ---------------------------------------------------- genesis_apy_bps

    /// APY okna wejścia: niemalejąco w czasie NIGDY (malejące okna 20→15→8%),
    /// wartości wyłącznie ze zbioru {W1, W2, W3}; granice wg stałych wariantu.
    #[test]
    fn genesis_apy_piecewise(t in 0i64..=WINDOW_2_END + 10 * SECONDS_PER_DAY) {
        let apy = genesis_apy_bps(t).unwrap();
        let expected = if t < WINDOW_1_END {
            APY_GENESIS_W1_BPS
        } else if t < WINDOW_2_END {
            APY_GENESIS_W2_BPS
        } else {
            APY_GENESIS_W3_BPS
        };
        prop_assert_eq!(apy, expected);
    }
}
