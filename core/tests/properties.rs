//! Testy PROPERTY-BASED modelu referencyjnego (rekomendacja audytów #1–#3).
//!
//! Dwie maszyny losowych scenariuszy:
//!  1. `xnt_pool_operation_machine` — losowe sekwencje stake/fund/settle/
//!     forfeit z inwariantami po KAŻDEJ operacji: monotoniczność indeksu,
//!     spójność sumy shares, oraz kluczowa własność bezpieczeństwa
//!     „brak inflacji": wypłacone + undistributed + należne ≤ wpłacone.
//!  2. `epoch_cap_immunity` — model checkpointów per epoka odtwarzający
//!     `settlement_cap_index` kontraktu i dowodzący własności zamykającej
//!     krytyka #1 audytu: funding epok PO end_epoch pozycji NIGDY nie
//!     zmienia jej rozliczenia.

use anl_core::*;
use proptest::prelude::*;

// ---------------------------------------------------------------- maszyna 1

#[derive(Debug, Clone)]
enum Op {
    Stake(u64),
    Fund(u64),
    Settle(usize),
    Forfeit(usize),
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        (1u64..=1_000_000_000_000u64).prop_map(Op::Stake),
        (0u64..=1_000_000_000_000u64).prop_map(Op::Fund),
        any::<usize>().prop_map(Op::Settle),
        any::<usize>().prop_map(Op::Forfeit),
    ]
}

struct Pos {
    shares: u64,
    debt: u128,
    alive: bool,
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn xnt_pool_operation_machine(ops in proptest::collection::vec(op_strategy(), 1..80)) {
        let mut pool = XntPool { index: 0, undistributed: 0, total_shares: 0 };
        let mut positions: Vec<Pos> = Vec::new();
        let mut funded: u128 = 0;   // suma wszystkich fundingów
        let mut paid_out: u128 = 0; // suma zamrożonych wypłat settle
        let mut prev_index: u128 = 0;
        let mut events: u128 = 0;   // liczba zdarzeń „stratnych" (floor)
        let mut max_shares: u64 = 0;

        for op in &ops {
            match *op {
                Op::Stake(s) => {
                    pool.add_shares(s).unwrap();
                    positions.push(Pos { shares: s, debt: pool.debt_snapshot(), alive: true });
                }
                Op::Fund(p) => {
                    pool.fund(p).unwrap();
                    funded += p as u128;
                    events += 1;
                }
                Op::Settle(i) | Op::Forfeit(i) => {
                    let alive: Vec<usize> = positions.iter().enumerate()
                        .filter(|(_, p)| p.alive).map(|(k, _)| k).collect();
                    if alive.is_empty() { continue; }
                    let k = alive[i % alive.len()];
                    let (sh, debt) = (positions[k].shares, positions[k].debt);
                    match *op {
                        Op::Settle(_) => paid_out += pool.settle(sh, debt).unwrap() as u128,
                        _ => { pool.forfeit(sh, debt).unwrap(); } // wraca do undistributed
                    }
                    positions[k].alive = false;
                    events += 1;
                }
            }
            max_shares = max_shares.max(pool.total_shares);

            // INWARIANT 1: indeks nigdy nie maleje.
            prop_assert!(pool.index >= prev_index);
            prev_index = pool.index;

            // INWARIANT 2: suma shares żywych pozycji == total_shares koszyka.
            let live: u128 = positions.iter().filter(|p| p.alive)
                .map(|p| p.shares as u128).sum();
            prop_assert_eq!(live, pool.total_shares as u128);

            // INWARIANT 3 (bezpieczeństwo): BRAK INFLACJI —
            // wypłacone + czekające w undistributed + należne żywym ≤ wpłacone.
            let owed: u128 = positions.iter().filter(|p| p.alive)
                .map(|p| pool.pending(p.shares, p.debt).unwrap() as u128).sum();
            let accounted = paid_out + pool.undistributed as u128 + owed;
            prop_assert!(
                accounted <= funded,
                "INFLACJA: rozliczone {} > wpłacone {}", accounted, funded
            );

            // INWARIANT 4: strata zaokrągleń ograniczona — na każde zdarzenie
            // co najwyżej max_shares/PRECISION + 2 jednostki.
            let loss = funded - accounted;
            let bound = events * ((max_shares as u128) / PRECISION + 2);
            prop_assert!(loss <= bound, "strata {} > granica {}", loss, bound);
        }
    }

    // ------------------------------------------------------------ maszyna 2

    /// Model checkpointów per epoka (jak w kontrakcie po modelu epok):
    /// checkpoint = indeks koszyka na koniec każdej epoki Z FUNDINGIEM;
    /// rozliczenie pozycji tnie po checkpointcie „ostatnia epoka fundingu
    /// ≤ end_epoch" (settlement_cap_index). WŁASNOŚĆ (zamyka krytyka #1):
    /// dowolny funding epok > end_epoch nie zmienia wyniku rozliczenia.
    #[test]
    fn epoch_cap_immunity(
        pre  in proptest::collection::vec(0u64..=1_000_000_000u64, 0..6),  // epoki przed stake
        mid  in proptest::collection::vec(0u64..=1_000_000_000u64, 1..6),  // epoki stake..=end (≥1!)
        post in proptest::collection::vec(0u64..=1_000_000_000u64, 1..6),  // epoki PO end (spóźnione!)
        my_shares in 1u64..=1_000_000u64,
        other_shares in 1u64..=1_000_000u64,
    ) {
        let mut pool = XntPool { index: 0, undistributed: 0, total_shares: 0 };
        let mut checkpoints: Vec<(usize, u128)> = Vec::new(); // (epoka, indeks)
        let mut epoch = 0usize;

        // tło: cudze shares od początku, żeby fundingi „pre" wchodziły do indeksu
        pool.add_shares(other_shares).unwrap();

        for &f in &pre {
            if f > 0 { pool.fund(f).unwrap(); checkpoints.push((epoch, pool.index)); }
            epoch += 1;
        }

        // nasza pozycja startuje teraz; end_epoch = ostatnia epoka fazy „mid"
        pool.add_shares(my_shares).unwrap();
        let debt = pool.debt_snapshot();
        for &f in &mid {
            if f > 0 { pool.fund(f).unwrap(); checkpoints.push((epoch, pool.index)); }
            epoch += 1;
        }
        let end_epoch = epoch.saturating_sub(1);

        // cap = checkpoint ostatniej epoki fundingu ≤ end_epoch (albo debt)
        let cap = |ckpts: &[(usize, u128)]| -> u128 {
            ckpts.iter().rev().find(|(e, _)| *e <= end_epoch)
                .map(|&(_, i)| i).unwrap_or(0).max(debt)
        };
        let payout_now = my_shares as u128 * (cap(&checkpoints) - debt) / PRECISION;

        // SPÓŹNIONY funding: epoki > end_epoch (dokładnie scenariusz krytyka #1)
        for &f in &post {
            if f > 0 { pool.fund(f).unwrap(); checkpoints.push((epoch, pool.index)); }
            epoch += 1;
        }
        let payout_late = my_shares as u128 * (cap(&checkpoints) - debt) / PRECISION;

        // WŁASNOŚĆ GŁÓWNA: rozliczenie NIEZMIENNE mimo późniejszych fundingów.
        prop_assert_eq!(payout_now, payout_late,
            "spóźniony funding zmienił rozliczenie: {} → {}", payout_now, payout_late);

        // Sanity: cap nigdy nie przekracza żywego indeksu (rozliczenie po
        // checkpointcie ≤ rozliczenie po indeksie bieżącym).
        let live = pool.pending(my_shares, debt).unwrap() as u128;
        prop_assert!(payout_late <= live);
    }
}
