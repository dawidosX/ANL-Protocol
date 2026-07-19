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
- **Period end**: the ANL reward stops at the exact `end_ts`; XNT uses daily epoch granularity and includes the full `end_epoch` (see WP §8.1). `settle_expired`
  (permissionless) freezes the position's XNT and removes its shares from the basket.
- **`claim`** (after `end_ts`): ANL reward + accrued XNT + principal in **one
  transaction**; the position account is closed (rent returns to the owner).
- **`unstake_early`** (before `end_ts`): principal returns in full; **all rewards are
  forfeited** — the ANL reservation is released (tokens never left the Reward Vault),
  accrued XNT returns to the basket's `xnt_undistributed`.

## Status: Phase 1 + Phase 2 ✅ (integration-tested)

| Module | Scope | Tests |
|---|---|---|
| `crates/anl-math` | APY windows (31/91), period rewards, XNT index, 65/35 split, dust | **24/24** |
| `core/` | reference model: declared periods, `settle`, `forfeit`, WP examples | **34/34** |
| `initialize` | GlobalConfig + VaultAuthority + 3 vaults; ANL=Token-2022, XNT=SPL | TC-001…006 |
| `create_pool` | exactly 2 pools, 65/35 XNT shares | TC-010…016 |
| `pause` / `resume` | emergency brake | TC-100…105 |
| `stake` | actual received, Immutable APY, 7..=3650-day period, reward reservation | ✅ integ. |
| `fund_rewards` / `fund_xnt(amount, epoch)` | NET deposits; 65/35 split; epoch checkpoint | ✅ integ. |
| `settle_expired` | permissionless; XNT from checkpoint ≤ `end_epoch` (audit #1 ✅) | ✅ integ. |
| `claim` | ANL+XNT+principal in 1 tx, reservation release, account close | ✅ integ. |
| `unstake_early` | full principal back; ANL (reservation) and XNT (undistributed) forfeited | ✅ integ. |

## Operations (daily bot) — IMPORTANT

After audit #1 the `end_epoch` boundary is enforced by the CONTRACT (epoch
checkpoints) — funding of a later epoch can never increase a position's
payout, regardless of operation ordering. The daily bot still runs
1) `settle_expired`, 2) `fund_xnt` (minimises index dilution from unsettled
matured positions). `fund_xnt` takes `epoch == epoch_of(now)` plus checkpoint
accounts (current + previous on the first funding of a new epoch).

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


> **XNT semantics — epoch, not second.** The XNT stream settles in daily epoch units: `end_epoch = epoch_of(end_ts − 1)`. A position earns XNT for every epoch in which any second of its period is active (including the full end epoch); funding of an epoch `> end_epoch` never increases the payout. The ANL reward accrues to the exact `end_ts`. This is the intended model (WP §8.1) — not a doc↔code mismatch.

## Integration tests (solana-program-test)

```bash
cargo test -p anl_staking --features test-periods --test integration   # 4/4
```

In-process, with real CPIs into Token-2022 and SPL Token, clock driven via the sysvar.
Scenarios: full 2-user cycle with daily XNT (2:1 proportions, settle freezes accrual,
claim = ANL+XNT+principal in 1 tx, account closed) · early exit (100% principal back,
forfeiture into the basket pool, redistribution at the next funding, PeriodNotEnded /
PeriodAlreadyEnded guards) · Immutable-APY windows + reward coverage + validations +
pause (stake blocked, claim works). The suite reads constants from anl-math — it runs
identically in both build variants.


## Security — audit #1 (18 Jul 2026)

The contract went through a preliminary security audit. Findings status and fixes:
**[docs/AUDIT-RESPONSE.md](docs/AUDIT-RESPONSE.md)**. In short: an **operator** role
(funding-only hot key, `set_operator` from the multisig), a Token-2022 mint-extension
gate in `initialize`, `version` checks in every instruction, full vault constraints,
and a hard production-constants guard test. **Open: #1** (XNT expiry buckets) — must
be closed before any immutable deployment.

## Phase 3 (next)

Full-cycle integration testing on the X1 testnet (Volume 10B), 24h+ fuzzing,
AI audit, a 100,000 ANL pilot (launch in pause), dashboard.
