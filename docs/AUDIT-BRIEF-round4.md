# Security Audit Brief — ANL Staking Protocol (round 4)

**Repository:** https://github.com/dawidosX/ANL-Protocol (branch `main`, commit `6ec2139`)
**Chain:** X1 Network (Solana-compatible SVM)
**Stack:** Rust + Anchor 0.30.1
**Scope:** on-chain program `programs/anl_staking/` + `crates/anl-math/`
**Status:** live on X1 testnet; not yet on mainnet.

You are an independent security auditor. Review the smart contract for
vulnerabilities. Be adversarial: assume a malicious user, a malicious operator,
and hostile transaction ordering. Report findings by severity
(Critical / High / Medium / Low / Info) with a concrete exploit path and a fix.

---

## 1. What the protocol does

Non-custodial ANL staking with a **triple reward stream**:
- **ANL** — fixed APY from a finite, segregated pool (200,000,000 ANL)
- **XNT** — daily-accrued share of an external validator's revenue (funded via `fund_xnt`)
- **CAPY** — a bonus from a segregated pool (20,000,000 CAPY), reserved at claim,
  paid by a separate `claim_capy`

Two programs: **Genesis** (elevated APY by entry window, up to 20%, locked until
period end) and **Flexible** (fixed 8%, early exit allowed with reward forfeit).
APY assigned at open is immutable for the position's life.

**Four isolated PDA vaults:** Principal, Reward (ANL), XNT, Capy. Invariant:
principal never paid from a reward vault, rewards never from principal vault.

## 2. Files to review (priority order)

| File | Lines | Focus |
|---|---|---|
| `instructions/lifecycle.rs` | 822 | claim, unstake_early, settle_expired, claim_genesis_window, claim_capy, close_day, settlement cap |
| `instructions/fund.rs` | 538 | fund_xnt, checkpoint roll, 65/35 split, PDA validation |
| `instructions/stake.rs` | 262 | position open, reward reservation, period validation |
| `state/mod.rs` | 452 | account layouts, index math, settle_position_at, orphan handling |
| `crates/anl-math/src/lib.rs` | 347 | APY, XNT index (acc-per-share, PRECISION 1e12), splits |
| `instructions/initialize.rs`, `create_pool.rs`, `set_pause.rs` | — | setup, pause guard |

## 3. Changes since last audit (round 3) — REVIEW THESE FIRST

These touch the reward-settlement core and were made after round 3. They need
fresh scrutiny:

**(a) Settlement cap → last closed day.** `settlement_cap_index` now caps XNT to
the **last closed day** (`current_day_basket>0 ? current_day-1 : current_day`,
clamped to `end_epoch`), not hard to `end_epoch`. Rationale: a position claiming
in the current (still-open) day forgoes that partial day and claims immediately.
The forgone partial-day share stays in the basket for living positions.
→ Verify: can this under- or over-pay? Can a user game the day boundary to claim
more? Does the orphan/`xnt_undistributed` accounting stay conservation-correct?

**(b) Removed `DayNotClosed` guards** in `settle_expired` and the `claim`
inline-settle path. Rationale: the cap now self-limits to a closed day, so the
guard became redundant.
→ Verify: is the guard truly redundant, or does its removal open a path where
`cap_index_at` reads a checkpoint with a pre-distribution index (cap too low/high)?

**(c) CAPY split-claim architecture.** `claim` computes and reserves
`pending_capy` but does NOT transfer CAPY; transfer is a separate `claim_capy`.
→ Verify: reservation accounting (`capy_reserved`, `available = vault - reserved`),
no double-count, no way to claim_capy more than reserved.

## 4. Specific attack questions (answer each)

1. Double claim of one position (sequential and via parallel/concurrent txs).
2. Claim before `end_ts`.
3. Operating on another user's position / `user_position` / index.
4. Passing a wrong `pool_type` and passing account validation.
5. Is Genesis `unstake_early` truly blocked on-chain (not just UI)?
6. Replaying `claim_genesis_window` for the same window.
7. `claim_capy` paying out more than `pending_capy` / draining the Capy vault.
8. Permissionless `close_day`: wrong epoch, replay, or as a griefing vector.
9. Manipulating `close_day → claim` ordering to change payout.
10. Edge values: 1 ANL, very large stake, extreme period, u64/u128 overflow.
11. Does closed-position status block all further operations?
12. Reward-coverage: staking beyond Reward Vault balance (`RewardCoverageExceeded`).
13. XNT index precision/rounding: dust accumulation, rounding in attacker's favor.
14. `fund_xnt` griefing: funding wrong epoch, funding to force bad checkpoint chain.
15. Reentrancy / CPI ordering around token transfers and account close.

## 5. Compile-time & test-build guards to verify

- `compile_error!` rejects `network-mainnet` + `test-periods`, and any two-network
  build (see `lib.rs`). CI job `release-guards` proves both negatives.
- `test-periods` feature shortens periods for testing (min 1 day, Genesis windows
  0-2/3-8/from-9, XNT payout window 3 days). Production: 7 days min, 30/90/91 windows.
- Confirm no test-periods constant can leak into a mainnet build.

## 6. Known open items (not findings — context)

- Integration tests (`tests/integration.rs`) don't yet compile against the CAPY
  account set (missing `capy_vault`, `user_profile` in test `Claim`). Production
  code compiles; this is a test-harness debt, being fixed separately.
- Dependency advisories (Dependabot): 13 open (3 high / 5 moderate / 5 low) — under review.

## 7. How to build

```bash
# X1 toolchain: solana-cli 2.3.11 + platform-tools v1.53
cargo-build-sbf --tools-version v1.53 --features network-testnet,test-periods
cargo test -p anl-math          # 24 unit + 10 property tests
cd core && cargo test           # reference model
```

## 8. Deliverable

For each finding: **severity · location (file:line) · exploit path · impact · fix.**
Prioritise the round-3 changes (section 3). If you find nothing exploitable in a
category, say so explicitly — a clean bill on each attack question is useful too.

Prior audit trail: `docs/SECURITY-AUDITS.md` and `docs/audits/`.
Whitepaper (economic model): `docs/whitepaper/`.
