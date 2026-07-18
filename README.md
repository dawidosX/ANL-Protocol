# ANL Staking Protocol — Smart Contract (X1 Network)

On-chain program in Rust + Anchor 0.29. Implementation of **[White Paper v1.0](docs/ANL_White_Paper_EN.pdf)**
([wersja polska](docs/ANL_White_Paper_PL.pdf) · [polski README](README.pl.md))
— model: daily XNT, user-declared periods, all-or-nothing rewards.
Target network: **X1 Network (x1.xyz)** — a Solana fork.

## Model (WP v1.0)

- **Both programs** (Genesis / Flexible): the participant declares the period, **7..=3650 days**.
- **Genesis windows** (from public launch): days **0–30 → 20%**, **31–90 → 15%**,
  **from 91 → 8%**. Flexible: always 8%. Immutable APY — the rate from the moment of opening.
- The **ANL reward** is known upfront and **reserved** at stake time
  (`GlobalConfig.anl_reward_reserved`) — a stake without coverage in the Reward Vault
  is rejected (`RewardCoverageExceeded`, WP §11).
- **Daily XNT**: `fund_xnt` splits validator revenue **65% Genesis / 35% Flexible**
  and advances basket indices (`acc-per-share`, PRECISION 1e12). Empty basket →
  the share waits in `xnt_undistributed` and enters at the next funding.
- **Period end**: accrual of BOTH streams stops at `end_ts`. `settle_expired`
  (permissionless) freezes the position's XNT and removes its shares from the basket.
- **`claim`** (after `end_ts`): ANL reward + accrued XNT + principal in **one
  transaction**; the position account is closed (rent returns to the owner).
- **`unstake_early`** (before `end_ts`): principal returns in full; **all rewards are
  forfeited** — the ANL reservation is released (tokens never left the Reward Vault),
  accrued XNT returns to the basket's `xnt_undistributed`.

## Status: Phase 1 + Phase 2 ✅ (integration-tested)

| Module | Scope | Tests |
|---|---|---|
| `crates/anl-math` | APY windows (31/91), period rewards, XNT index, 65/35 split, dust | **23/23** |
| `core/` | reference model: declared periods, `settle`, `forfeit`, WP examples | **34/34** |
| `initialize` | GlobalConfig + VaultAuthority + 3 vaults; ANL=Token-2022, XNT=SPL | TC-001…006 |
| `create_pool` | exactly 2 pools, 65/35 XNT shares | TC-010…016 |
| `pause` / `resume` | emergency brake | TC-100…105 |
| `stake` | actual received, Immutable APY, 7..=3650-day period, reward reservation | ✅ integ. |
| `fund_rewards` / `fund_xnt` | NET deposits; daily 65/35 split into indices | ✅ integ. |
| `settle_expired` | permissionless; freezes XNT, removes shares after `end_ts` | ✅ integ. |
| `claim` | ANL+XNT+principal in 1 tx, reservation release, account close | ✅ integ. |
| `unstake_early` | full principal back; ANL (reservation) and XNT (undistributed) forfeited | ✅ integ. |

## Operations (daily bot) — IMPORTANT

Order every day: **1) `settle_expired` for positions with `end_ts` ≤ now,
2) only then `fund_xnt`.** Settling before funding guarantees a matured position
takes no part in that day's distribution (WP §8, to the day).
`settle_expired` is permissionless — a bot outage blocks nothing,
and `claim` performs an inline settle.

Pausing (`pause`) blocks `stake`; the exit paths (`claim`, `unstake_early`,
`settle_expired`) always work — a user is never trapped.

## Test build — the `test-periods` feature

For the testing phase (X1 testnet), time parameters are shortened at **compile time**:

| Parameter | Production | `test-periods` |
|---|---|---|
| Min. position period | 7 days | **1 day** |
| Genesis Window 1 (20%) | days 0–30 | **days 0–2** |
| Genesis Window 2 (15%) | days 31–90 | **days 3–8** |
| Genesis Window 3 (8%) | from day 91 | **from day 9** |

```bash
cargo test -p anl-math --features test-periods
anchor build -- --features test-periods     # TESTNET artifact
```

**Never deploy a `test-periods` build to mainnet.** Safeguards:
the feature is not in `default`, and `initialize` logs a warning `msg!`
in every test build — visible in the logs of the first transaction.

## Building

```bash
cargo test -p anl-math          # math (23)
cd core && cargo test           # reference model (34)
anchor build                    # SBF artifact
anchor keys sync                # proper Program ID
```

Toolchain: **Rust ≥ 1.80** (verified on 1.89). `Cargo.lock` generated
on 1.89 — the old rustc 1.75 pins have been removed.

## Integration tests (solana-program-test)

```bash
cargo test -p anl_staking --features test-periods --test integration   # 3/3
```

In-process, with real CPIs into Token-2022 and SPL Token, clock driven via the sysvar.
Scenarios: full 2-user cycle with daily XNT (2:1 proportions, settle freezes accrual,
claim = ANL+XNT+principal in 1 tx, account closed) · early exit (100% principal back,
forfeiture into the basket pool, redistribution at the next funding, PeriodNotEnded /
PeriodAlreadyEnded guards) · Immutable-APY windows + reward coverage + validations +
pause (stake blocked, claim works). The suite reads constants from anl-math — it runs
identically in both build variants.

## Phase 3 (next)

Full-cycle integration testing on the X1 testnet (Volume 10B), 24h+ fuzzing,
AI audit, a 100,000 ANL pilot (launch in pause), dashboard.
