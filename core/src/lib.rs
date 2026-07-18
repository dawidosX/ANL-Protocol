//! ANL Staking Protocol — deterministyczny rdzeń matematyczny.
//!
//! Zero zależności. Każda funkcja jest czystą funkcją wejścia — bez zegara,
//! bez stanu globalnego, bez losowości. Implementuje Specyfikację Techniczną
//! v1.0 (decyzje D-1…D-14). Testy oznaczone identyfikatorami TC-xxx
//! odpowiadają katalogowi Volume 10B.

#![deny(clippy::arithmetic_side_effects)]
#![forbid(unsafe_code)]

// ============================================================ stałe (spec §2)

/// Skala indeksu nagród XNT (D-5).
pub const PRECISION: u128 = 1_000_000_000_000; // 1e12
/// Basis points.
pub const BPS_DENOMINATOR: u128 = 10_000;

/// Doba w sekundach. W buildzie testowym (feature `short-periods`,
/// 10C §13 / D-11) doba trwa 60 s — stałe produkcyjne nigdy nie są
/// konfigurowalne w runtime.
#[cfg(not(feature = "short-periods"))]
pub const SECONDS_PER_DAY: i64 = 86_400;
#[cfg(feature = "short-periods")]
pub const SECONDS_PER_DAY: i64 = 60;

/// Rok = 365 dni (D-3).
pub const SECONDS_PER_YEAR: i64 = 365 * SECONDS_PER_DAY;

pub const APY_FLEXIBLE_BPS: u16 = 800;
pub const APY_GENESIS_W1_BPS: u16 = 2_000; // dni 0–30 (WP v1.0 §5)
pub const APY_GENESIS_W2_BPS: u16 = 1_500; // dni 31–90
pub const APY_GENESIS_W3_BPS: u16 = 800; // od dnia 91

pub const XNT_SHARE_GENESIS_BPS: u128 = 6_500;

/// Granica okna 1 (sekundy od startu programu): [0, W1) → 20%.
/// WP v1.0 §5: „do końca dnia 30" ⇒ W1 = początek dnia 31.
/// Feature `test-periods` (testnet): okna 3/9 dni, min. okres 1 dzień.
#[cfg(not(feature = "test-periods"))]
pub const WINDOW_1_END: i64 = 31 * SECONDS_PER_DAY;
#[cfg(feature = "test-periods")]
pub const WINDOW_1_END: i64 = 3 * SECONDS_PER_DAY;
/// Granica okna 2: [W1, W2) → 15%, [W2, ∞) → 8% (koniec dnia 90).
#[cfg(not(feature = "test-periods"))]
pub const WINDOW_2_END: i64 = 91 * SECONDS_PER_DAY;
#[cfg(feature = "test-periods")]
pub const WINDOW_2_END: i64 = 9 * SECONDS_PER_DAY;

/// Minimalny okres pozycji — OBA programy (WP v1.0 §7).
#[cfg(not(feature = "test-periods"))]
pub const MIN_PERIOD_DAYS: u32 = 7;
#[cfg(feature = "test-periods")]
pub const MIN_PERIOD_DAYS: u32 = 1;
/// Maksymalny okres pozycji (sanity bound).
pub const MAX_PERIOD_DAYS: u32 = 3_650;

// ============================================================ błędy

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreError {
    /// Czas przed startem programu / ujemny odstęp.
    TimeBeforeStart,
    /// Przepełnienie arytmetyczne.
    MathOverflow,
    /// Deklarowany okres pozycji poza [MIN_PERIOD_DAYS, MAX_PERIOD_DAYS].
    InvalidPeriod,
    /// Aktualizacja indeksu przy zerowych shares (obsługiwane przez
    /// `xnt_undistributed`, bezpośrednie wywołanie jest błędem — TC-129).
    ZeroShares,
    /// Indeks pozycji nowszy niż indeks puli (niemożliwe przy poprawnym
    /// użyciu; chroni przed underflow — TC-243).
    DebtAheadOfIndex,
}

pub type CoreResult<T> = Result<T, CoreError>;

// ============================================================ okna APY (spec §7, D-2)

/// APY (w bps) przypisywane pozycji Genesis wchodzącej `elapsed` sekund po
/// `genesis_start_ts`. Przedziały lewostronnie domknięte, prawostronnie
/// otwarte: sekunda graniczna należy do NOWEGO okna (TC-041/043).
pub fn genesis_apy_for_entry(elapsed_secs: i64) -> CoreResult<u16> {
    if elapsed_secs < 0 {
        return Err(CoreError::TimeBeforeStart);
    }
    Ok(if elapsed_secs < WINDOW_1_END {
        APY_GENESIS_W1_BPS
    } else if elapsed_secs < WINDOW_2_END {
        APY_GENESIS_W2_BPS
    } else {
        APY_GENESIS_W3_BPS
    })
}

// ============================================================ okres pozycji (WP v1.0 §7)

/// Koniec zadeklarowanego okresu — jednakowo dla Genesis i Flexible.
/// Okres wybiera uczestnik: [MIN_PERIOD_DAYS, MAX_PERIOD_DAYS].
pub fn declared_end_ts(start_ts: i64, declared_days: u32) -> CoreResult<i64> {
    if !(MIN_PERIOD_DAYS..=MAX_PERIOD_DAYS).contains(&declared_days) {
        return Err(CoreError::InvalidPeriod);
    }
    let span = (declared_days as i64)
        .checked_mul(SECONDS_PER_DAY)
        .ok_or(CoreError::MathOverflow)?;
    start_ts.checked_add(span).ok_or(CoreError::MathOverflow)
}

/// Okres zakończony ⟺ now >= end_ts (granica INKLUZYWNA — TC-092).
/// Po tej chwili naliczanie OBU strumieni stoi; pozycja czeka na claim.
pub fn period_ended(now: i64, end_ts: i64) -> bool {
    now >= end_ts
}

/// Nagrody wymagalne ⟺ okres zakończony (WP §7: całość albo nic).
pub fn rewards_matured(now: i64, end_ts: i64) -> bool {
    period_ended(now, end_ts)
}

// ============================================================ nagroda ANL (spec §5, D-1)

/// Deterministyczna nagroda ANL za okres:
/// `amount × apy_bps × period_secs / (10 000 × SECONDS_PER_YEAR)`,
/// zaokrąglenie w dół (polityka dust, TC-126). Jedno dzielenie na końcu.
pub fn period_reward(amount: u64, apy_bps: u16, period_secs: i64) -> CoreResult<u64> {
    if period_secs < 0 {
        return Err(CoreError::TimeBeforeStart);
    }
    let num = (amount as u128)
        .checked_mul(apy_bps as u128)
        .and_then(|v| v.checked_mul(period_secs as u128))
        .ok_or(CoreError::MathOverflow)?;
    let den = BPS_DENOMINATOR
        .checked_mul(SECONDS_PER_YEAR as u128)
        .ok_or(CoreError::MathOverflow)?;
    let out = num.checked_div(den).ok_or(CoreError::MathOverflow)?;
    u64::try_from(out).map_err(|_| CoreError::MathOverflow)
}

// ============================================================ podział XNT (spec §6.2)

/// Podział DZIENNEGO fundingu XNT (WP v1.0 §8): Genesis = ⌊delta×65%⌋, Flexible = reszta.
/// Suma części ZAWSZE równa delcie — zero dust na tym etapie.
pub fn split_xnt(delta: u64) -> (u64, u64) {
    let genesis = ((delta as u128) * XNT_SHARE_GENESIS_BPS / BPS_DENOMINATOR) as u64;
    (genesis, delta - genesis)
}

// ============================================================ silnik indeksu XNT (spec §6)

/// Stan indeksu jednej puli. Odpowiada polom `PoolConfig`
/// (`xnt_reward_index`, `xnt_undistributed`, `total_shares`).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct XntPool {
    pub index: u128,
    pub undistributed: u64,
    pub total_shares: u64,
}

impl XntPool {
    /// Funding części tej puli (`part`, wartość NETTO — Token §9).
    /// Przy `total_shares == 0` środki czekają w `undistributed` i wchodzą
    /// do indeksu przy najbliższym fundingu z shares > 0 (D-5, TC-028/077/078).
    /// Indeks nigdy nie maleje (TC-120); brak dzielenia przez zero (TC-129).
    pub fn fund(&mut self, part: u64) -> CoreResult<()> {
        let part_total = self
            .undistributed
            .checked_add(part)
            .ok_or(CoreError::MathOverflow)?;
        if self.total_shares == 0 {
            self.undistributed = part_total;
            return Ok(());
        }
        let inc = (part_total as u128)
            .checked_mul(PRECISION)
            .ok_or(CoreError::MathOverflow)?
            / (self.total_shares as u128); // shares > 0 — sprawdzone wyżej
        self.index = self.index.checked_add(inc).ok_or(CoreError::MathOverflow)?;
        self.undistributed = 0;
        Ok(())
    }

    /// Snapshot debt dla nowej pozycji (TC-121/124): pozycja nie otrzymuje
    /// nagród sprzed swojego utworzenia.
    pub fn debt_snapshot(&self) -> u128 {
        self.index
    }

    /// Nagroda XNT należna pozycji o `shares` i `debt_index`
    /// (zaokrąglenie w dół — TC-126).
    pub fn pending(&self, shares: u64, debt_index: u128) -> CoreResult<u64> {
        let diff = self
            .index
            .checked_sub(debt_index)
            .ok_or(CoreError::DebtAheadOfIndex)?;
        let out = (shares as u128)
            .checked_mul(diff)
            .ok_or(CoreError::MathOverflow)?
            / PRECISION;
        u64::try_from(out).map_err(|_| CoreError::MathOverflow)
    }

    pub fn add_shares(&mut self, s: u64) -> CoreResult<()> {
        self.total_shares = self
            .total_shares
            .checked_add(s)
            .ok_or(CoreError::MathOverflow)?;
        Ok(())
    }

    pub fn remove_shares(&mut self, s: u64) -> CoreResult<()> {
        self.total_shares = self
            .total_shares
            .checked_sub(s)
            .ok_or(CoreError::MathOverflow)?;
        Ok(())
    }

    /// Wcześniejsze zerwanie pozycji (WP v1.0 §7): naliczone XNT pozycji
    /// wracają do puli dystrybucji koszyka — trafiają do `undistributed`
    /// i wchodzą do indeksu przy najbliższym fundingu. Shares schodzą.
    pub fn forfeit(&mut self, shares: u64, debt_index: u128) -> CoreResult<u64> {
        let pending = self.pending(shares, debt_index)?;
        self.remove_shares(shares)?;
        self.undistributed = self
            .undistributed
            .checked_add(pending)
            .ok_or(CoreError::MathOverflow)?;
        Ok(pending)
    }

    /// Zakończenie okresu pozycji (settle): shares schodzą z koszyka —
    /// pozycja po `end_ts` nie uczestniczy w dziennej dystrybucji (WP §8).
    /// Zwraca zamrożoną, należną kwotę XNT pozycji.
    pub fn settle(&mut self, shares: u64, debt_index: u128) -> CoreResult<u64> {
        let pending = self.pending(shares, debt_index)?;
        self.remove_shares(shares)?;
        Ok(pending)
    }
}

// ============================================================ walidacje pomocnicze

/// Minimalny stake = 1 ANL w najmniejszych jednostkach (D-7).
pub fn min_stake_amount(decimals: u8) -> u64 {
    10u64.saturating_pow(decimals as u32)
}

// ============================================================================
// TESTY — identyfikatory TC odpowiadają Volume 10B
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    const D: i64 = SECONDS_PER_DAY;
    const ANL: u64 = 1_000_000; // 1 ANL przy decimals=6 → tu: jednostki umowne

    // ---------------- okna APY (sekcja 9 / 10B §9) ----------------

    #[test]
    #[cfg(not(feature = "test-periods"))]
    fn tc_040_genesis_entry_day_0_30_gets_20pct() {
        assert_eq!(genesis_apy_for_entry(0).unwrap(), 2000);
        assert_eq!(genesis_apy_for_entry(12 * D).unwrap(), 2000);
        assert_eq!(genesis_apy_for_entry(30 * D).unwrap(), 2000); // dzień 30 nadal 20%
        assert_eq!(genesis_apy_for_entry(31 * D - 1).unwrap(), 2000); // koniec dnia 30
    }

    #[test]
    #[cfg(not(feature = "test-periods"))]
    fn tc_041_exact_start_of_day_31_belongs_to_new_window() {
        // WP §5: przedział półotwarty — początek dnia 31 = 15%
        assert_eq!(genesis_apy_for_entry(31 * D).unwrap(), 1500);
    }

    #[test]
    #[cfg(not(feature = "test-periods"))]
    fn tc_042_entry_day_31_90_gets_15pct() {
        assert_eq!(genesis_apy_for_entry(45 * D).unwrap(), 1500);
        assert_eq!(genesis_apy_for_entry(90 * D).unwrap(), 1500); // dzień 90 nadal 15%
        assert_eq!(genesis_apy_for_entry(91 * D - 1).unwrap(), 1500); // koniec dnia 90
    }

    #[test]
    #[cfg(not(feature = "test-periods"))]
    fn tc_043_exact_start_of_day_91_belongs_to_new_window() {
        assert_eq!(genesis_apy_for_entry(91 * D).unwrap(), 800);
    }

    #[test]
    fn tc_044_entry_from_day_91_gets_8pct() {
        assert_eq!(genesis_apy_for_entry(200 * D).unwrap(), 800);
        assert_eq!(genesis_apy_for_entry(10_000 * D).unwrap(), 800);
    }

    #[test]
    fn tc_242_negative_elapsed_rejected() {
        assert_eq!(genesis_apy_for_entry(-1), Err(CoreError::TimeBeforeStart));
    }

    // ---------------- okres pozycji (WP v1.0 §7) ----------------

    #[test]
    fn tc_045_047_declared_end_ts_for_user_periods() {
        let t0 = 1_753_000_000;
        assert_eq!(declared_end_ts(t0, 7).unwrap(), t0 + 7 * D);
        assert_eq!(declared_end_ts(t0, 60).unwrap(), t0 + 60 * D);
        assert_eq!(declared_end_ts(t0, 365).unwrap(), t0 + 365 * D);
    }

    #[test]
    fn tc_050_period_bounds_enforced_for_both_programs() {
        assert_eq!(declared_end_ts(0, MIN_PERIOD_DAYS - 1), Err(CoreError::InvalidPeriod));
        assert_eq!(declared_end_ts(0, 0), Err(CoreError::InvalidPeriod));
        assert_eq!(declared_end_ts(0, MAX_PERIOD_DAYS + 1), Err(CoreError::InvalidPeriod));
        assert!(declared_end_ts(0, MIN_PERIOD_DAYS).is_ok());
        assert!(declared_end_ts(0, MAX_PERIOD_DAYS).is_ok());
    }

    #[test]
    fn tc_048_period_length_does_not_change_apy() {
        // APY zależy wyłącznie od momentu wejścia — nie od długości okresu.
        let apy = genesis_apy_for_entry(WINDOW_1_END - 1).unwrap();
        for _days in [7u32, 60, 365, 3_650] {
            assert_eq!(apy, 2000);
        }
    }

    #[test]
    fn tc_091_092_093_period_end_boundary_inclusive() {
        let end = 1_000_000;
        assert!(!period_ended(end - 1, end)); // TC-091
        assert!(period_ended(end, end)); // TC-092 — dokładnie w granicy
        assert!(period_ended(end + 1, end)); // TC-093
    }

    #[test]
    fn d12_maturity_boundary_inclusive() {
        assert!(!rewards_matured(999, 1000));
        assert!(rewards_matured(1000, 1000));
    }

    // ---------------- nagroda ANL (10B §16) ----------------

    #[test]
    fn whitepaper_example_1m_anl_20pct_full_year() {
        // Przykład z Volume 1 / White Paper: 1 000 000 × 20% × 365 dni
        let r = period_reward(1_000_000 * ANL, 2000, 365 * D).unwrap();
        assert_eq!(r, 200_000 * ANL);
    }

    #[test]
    fn flexible_100_days_8pct_matches_site_calculator() {
        // 1 000 000 × 8% × 100/365 = 21 917.80… → floor
        let r = period_reward(1_000_000, 800, 100 * D).unwrap();
        assert_eq!(r, 21_917);
    }

    #[test]
    fn tc_122_two_equal_users_receive_equal_rewards() {
        let a = period_reward(500 * ANL, 1500, 180 * D).unwrap();
        let b = period_reward(500 * ANL, 1500, 180 * D).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn tc_123_reward_proportional_to_amount() {
        let x = period_reward(1_000 * ANL, 800, 90 * D).unwrap();
        let y = period_reward(2_000 * ANL, 800, 90 * D).unwrap();
        // dwukrotny wkład → dokładnie dwukrotna nagroda (formuła liniowa,
        // dzielenie na końcu)
        assert_eq!(y, x * 2);
    }

    #[test]
    fn tc_127_maximum_amount_no_overflow() {
        // u64::MAX × 2000 bps × 365 dni mieści się w u128
        let r = period_reward(u64::MAX, 2000, 365 * D).unwrap();
        assert_eq!(r, u64::MAX / 5); // 20% z maksimum (floor)
    }

    #[test]
    fn tc_128_minimum_nonzero_reward_floors_to_zero() {
        // 1 jednostka × 8% × 1 s → 0 (zaokrąglenie w dół, nigdy w górę)
        assert_eq!(period_reward(1, 800, 1).unwrap(), 0);
        // brak nieprzewidywalnego zaokrąglenia w górę
        assert_eq!(period_reward(12, 800, D).unwrap(), 0);
    }

    #[test]
    fn tc_126_reward_never_exceeds_linear_bound() {
        // suma wypłat ≤ wartość „idealna" bez zaokrągleń
        for amt in [1u64, 999, 123_456_789, u64::MAX / 3] {
            let r = period_reward(amt, 2000, 365 * D).unwrap() as u128;
            let ideal = (amt as u128) * 2000 / 10_000;
            assert!(r <= ideal);
        }
    }

    #[test]
    fn negative_period_rejected() {
        assert_eq!(
            period_reward(ANL, 800, -1),
            Err(CoreError::TimeBeforeStart)
        );
    }

    // ---------------- podział XNT 65/35 (10B §11) ----------------

    #[test]
    fn tc_070_071_split_65_35_sums_exactly() {
        for delta in [0u64, 1, 3, 100, 12_345, u64::MAX] {
            let (g, f) = split_xnt(delta);
            assert_eq!(g as u128 + f as u128, delta as u128); // zero dust
            // Genesis dostaje ⌊65%⌋
            assert_eq!(g as u128, (delta as u128) * 6500 / 10_000);
        }
    }

    // ---------------- silnik indeksu XNT (10B §16) ----------------

    #[test]
    fn tc_120_index_is_monotonic() {
        let mut p = XntPool {
            total_shares: 1_000,
            ..Default::default()
        };
        let mut prev = p.index;
        for part in [500u64, 0, 1, 999_999, 42] {
            p.fund(part).unwrap();
            assert!(p.index >= prev);
            prev = p.index;
        }
    }

    #[test]
    fn tc_129_fund_with_zero_shares_goes_to_undistributed() {
        let mut p = XntPool::default(); // shares == 0
        p.fund(10_000).unwrap(); // brak paniki, brak dzielenia przez zero
        assert_eq!(p.undistributed, 10_000);
        assert_eq!(p.index, 0);
    }

    #[test]
    fn tc_028_077_undistributed_folds_into_next_funding() {
        let mut p = XntPool::default();
        p.fund(10_000).unwrap(); // pula pusta — poczekalnia
        p.add_shares(100).unwrap();
        let debt = p.debt_snapshot(); // pozycja wchodzi PO pierwszym fundingu
        p.fund(0).unwrap(); // kolejny funding uwalnia poczekalnię
        assert_eq!(p.undistributed, 0);
        // pozycja z shares=100 otrzymuje całą poczekalnię
        assert_eq!(p.pending(100, debt).unwrap(), 10_000);
    }

    #[test]
    fn tc_121_124_late_staker_gets_no_historical_rewards() {
        let mut p = XntPool {
            total_shares: 100,
            ..Default::default()
        };
        let debt_a = p.debt_snapshot(); // A wchodzi przed fundingiem
        p.fund(1_000).unwrap(); // funding #1 — tylko dla A
        p.add_shares(100).unwrap();
        let debt_b = p.debt_snapshot(); // B wchodzi PO fundingu #1
        p.fund(2_000).unwrap(); // funding #2 — A i B po połowie

        let a = p.pending(100, debt_a).unwrap();
        let b = p.pending(100, debt_b).unwrap();
        assert_eq!(a, 1_000 + 1_000); // 100% #1 + 50% #2
        assert_eq!(b, 1_000); // wyłącznie 50% #2 — zero historii (TC-124)
    }

    #[test]
    fn tc_125_claim_does_not_reduce_future_eligibility() {
        let mut p = XntPool {
            total_shares: 100,
            ..Default::default()
        };
        let mut debt = p.debt_snapshot();
        p.fund(500).unwrap();
        assert_eq!(p.pending(100, debt).unwrap(), 500);
        debt = p.debt_snapshot(); // claim = przesunięcie debt do bieżącego indeksu
        assert_eq!(p.pending(100, debt).unwrap(), 0); // TC-062/074: no double claim
        p.fund(700).unwrap();
        assert_eq!(p.pending(100, debt).unwrap(), 700); // pełna przyszła nagroda
    }

    #[test]
    fn tc_123_xnt_proportional_to_shares() {
        let mut p = XntPool {
            total_shares: 300,
            ..Default::default()
        };
        let debt = p.debt_snapshot();
        p.fund(3_000).unwrap();
        let one = p.pending(100, debt).unwrap();
        let two = p.pending(200, debt).unwrap();
        assert_eq!(two, one * 2);
        assert_eq!(one, 1_000);
    }

    #[test]
    fn tc_126_132_conservation_payouts_never_exceed_funding() {
        // symulacja: nierówne shares wymuszające dust przy dzieleniu
        let shares = [7u64, 13, 31];
        let mut p = XntPool {
            total_shares: shares.iter().sum(),
            ..Default::default()
        };
        let debts: Vec<u128> = shares.iter().map(|_| p.debt_snapshot()).collect();
        let funded: u64 = 1_000_003; // liczba pierwsza — gwarantowany dust
        p.fund(funded).unwrap();
        let paid: u128 = shares
            .iter()
            .zip(&debts)
            .map(|(s, d)| p.pending(*s, *d).unwrap() as u128)
            .sum();
        assert!(paid <= funded as u128); // TC-126: dust nie tworzy tokenów
        assert!(funded as u128 - paid < shares.len() as u128 + 1); // dust ograniczony
    }

    #[test]
    fn wp_s7_settle_freezes_accrual_at_period_end() {
        // pozycja A kończy okres → settle zdejmuje shares; kolejne fundingi
        // idą wyłącznie do pozostałych pozycji (WP §8: „pozycja aktywna").
        let mut p = XntPool { total_shares: 200, ..Default::default() };
        let debt_a = p.debt_snapshot();
        let debt_b = p.debt_snapshot();
        p.fund(1_000).unwrap(); // dzień 1: A i B po 500
        let frozen_a = p.settle(100, debt_a).unwrap();
        assert_eq!(frozen_a, 500);
        assert_eq!(p.total_shares, 100);
        p.fund(1_000).unwrap(); // dzień 2: wyłącznie B
        assert_eq!(p.pending(100, debt_b).unwrap(), 1_500);
        // zamrożona kwota A nie rośnie — naliczanie stanęło z końcem okresu
        assert_eq!(frozen_a, 500);
    }

    #[test]
    fn wp_s7_early_exit_forfeits_xnt_back_to_pool() {
        // zerwanie: naliczone XNT wracają do puli dystrybucji i trafiają
        // do pozostałych pozycji przy najbliższym fundingu (WP §7).
        let mut p = XntPool { total_shares: 200, ..Default::default() };
        let debt_a = p.debt_snapshot();
        let debt_b = p.debt_snapshot();
        p.fund(1_000).unwrap(); // A i B po 500
        let forfeited = p.forfeit(100, debt_a).unwrap();
        assert_eq!(forfeited, 500);
        assert_eq!(p.undistributed, 500);
        assert_eq!(p.total_shares, 100);
        p.fund(500).unwrap(); // dzień 2: 500 nowe + 500 z poczekalni → całość dla B
        assert_eq!(p.pending(100, debt_b).unwrap(), 500 + 1_000);
        assert_eq!(p.undistributed, 0);
    }

    #[test]
    fn wp_s5_example_genesis_60_days_100k() {
        // WP §5: 100 000 × 20% × 60/365 ≈ 3 288 (floor, jednostki umowne)
        let r = period_reward(100_000 * ANL, 2000, 60 * D).unwrap();
        assert_eq!(r, 3_287_671_232);
    }

    #[test]
    fn tc_243_debt_ahead_of_index_is_error_not_underflow() {
        let p = XntPool::default();
        assert_eq!(
            p.pending(10, PRECISION),
            Err(CoreError::DebtAheadOfIndex)
        );
    }

    #[test]
    fn tc_192_claim_ordering_independence() {
        // kolejność claimów nie zmienia sumy należności
        let mut p = XntPool {
            total_shares: 200,
            ..Default::default()
        };
        let da = p.debt_snapshot();
        let db = p.debt_snapshot();
        p.fund(10_001).unwrap();
        let a_then_b = p.pending(100, da).unwrap() + p.pending(100, db).unwrap();
        let b_then_a = p.pending(100, db).unwrap() + p.pending(100, da).unwrap();
        assert_eq!(a_then_b, b_then_a);
    }

    #[test]
    fn tc_240_fuzz_like_amount_sweep_no_panic() {
        // deterministyczny „mini-fuzz": wartości graniczne i pseudolosowe
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        let mut vals = vec![0u64, 1, u64::MAX, u64::MAX - 1];
        for _ in 0..200 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            vals.push(x);
        }
        for v in vals {
            let _ = period_reward(v, 2000, 365 * D); // Ok lub kontrolowany błąd
            let (g, f) = split_xnt(v);
            assert_eq!(g as u128 + f as u128, v as u128);
        }
    }

    // ---------------- walidacje ----------------

    #[test]
    fn d7_min_stake_is_one_whole_anl() {
        assert_eq!(min_stake_amount(6), 1_000_000);
        assert_eq!(min_stake_amount(9), 1_000_000_000);
    }
}
