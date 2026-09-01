# ANL Staking Protocol — Smart Contract (X1 Network)

[![CI](https://github.com/dawidosX/ANL-Protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/dawidosX/ANL-Protocol/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Network: X1](https://img.shields.io/badge/network-X1-7A3BFF.svg)](https://x1.xyz)
[![Phase: Testnet](https://img.shields.io/badge/phase-testnet-FFB11A.svg)](https://testnet.anl-protocol.com)

On-chain program in **Rust + Anchor**. Non-custodial ANL staking with a
**triple reward stream** on **X1 Network (x1.xyz)** — a Solana-compatible chain.

Implementation of **White Paper v1.2** — [EN](docs/whitepaper/whitepaper.html) ·
[PDF EN](docs/whitepaper/ANL-Whitepaper-v1.2-EN.pdf) ·
[PDF PL](docs/whitepaper/ANL-Whitepaper-v1.2-PL.pdf) ·
[polski README](README.pl.md)

---

## What it is

A participant locks **ANL** for a self-declared period (min. 7 days) and receives
**three reward streams**:

- **ANL** — fixed APY from a finite, segregated pool of **200,000,000 ANL**
- **XNT** — daily-accrued share of a dedicated X1 validator's real revenue
- **CAPY** — a bonus from a segregated pool of **20,000,000 CAPY**, credited at claim
  proportionally to the ANL reward paid

Two programs: **Genesis** (elevated APY by entry window, up to 20%) and
**Flexible** (fixed 8%). The rate assigned at opening is **immutable** for the life
of the position.

## Model (WP v1.2)

- **Both programs** (Genesis / Flexible): participant declares the period, **7..=3650 days**.
- **Genesis windows** (from public launch): days **0–30 → 20%**, **31–90 → 15%**,
  **from 91 → 8%**. Flexible: always 8%. Immutable APY.
- **ANL reward** reserved at stake (`GlobalConfig.anl_reward_reserved`) — a stake
  without coverage in the Reward Vault is rejected (`RewardCoverageExceeded`).
- **Daily XNT**: `fund_xnt` splits validator revenue **65% Genesis / 35% Flexible`,
  advances basket indices (acc-per-share, PRECISION 1e12). Empty basket → the share
  waits in `xnt_undistributed`.
- **CAPY bonus**: at `claim`, `pending_capy` is computed
  (`anl_reward × available_capy / remaining_anl`) and **reserved** — paid out by a
  **separate** `claim_capy` instruction. The core claim (ANL+XNT) never blocks on CAPY.
- **Settlement cap → last closed day**: XNT is settled to the **last closed day**
  (≤ `end_epoch`). A position claiming in the current (still-open) day forgoes that
  partial day and **claims immediately** — no wait for the day to close.
- **`claim`** (after `end_ts`): ANL reward + accrued XNT + principal in one tx;
  position account closed. CAPY reserved (claimed separately).
- **`unstake_early`** (Flexible, before `end_ts`): principal returns in full; **all
  rewards forfeited**; no CAPY bonus. Genesis positions are locked until period end.

## Four isolated vaults

| Vault | Contents | Rule |
|---|---|---|
| Principal Vault | user principal | payouts only to position owners |
| Reward Vault | 200,000,000 ANL | payouts only as accrued rewards |
| XNT Vault | validator revenue | daily 65/35 distribution |
| Capy Vault | 20,000,000 CAPY | payouts only as reserved bonuses (`claim_capy`) |

The program never pays principal from a reward vault or rewards from the principal
vault — a hard-coded invariant covered by tests.

## Building

Toolchain that produces a working SBF artifact on this repo:

```bash
# solana-cli 2.3.11 + tools v1.53 (older CLI/tools fail on edition2024 / sysroot)
cargo-build-sbf --tools-version v1.53 --features network-testnet,test-periods
```

Math + reference model:

```bash
cargo test -p anl-math          # math (24 unit + 10 property)
cd core && cargo test           # reference model
```

**Never deploy a `test-periods` build to mainnet.** Enforced at compile time:
`compile_error!` rejects `network-mainnet`+`test-periods` and any two-network build.
CI's `release-guards` job proves both negative cases on every push.

## Test build — the `test-periods` feature

| Parameter | Production | `test-periods` |
|---|---|---|
| Min. position period | 7 days | 1 day |
| Genesis Window 1 (20%) | days 0–30 | days 0–2 |
| Genesis Window 2 (15%) | days 31–90 | days 3–8 |
| Genesis Window 3 (8%) | from day 91 | from day 9 |
| Genesis XNT payout window | 30 days | 3 days |

## Testnet — live

The protocol runs on **X1 testnet**: [testnet.anl-protocol.com](https://testnet.anl-protocol.com).
Full cycle verified on-chain: stake → `fund_xnt` → `claim` (ANL+XNT) → `claim_capy` (CAPY).
Live stats (TVL, vault balances, validator) read directly from chain.

## Security

The protocol has undergone **multiple rounds of security review** with findings
fixed and independently re-verified — full trail in
**[docs/SECURITY-AUDITS.md](docs/SECURITY-AUDITS.md)**, reviewer reports archived
under `docs/audits/`.

**Note (post-audit-3 changes, pending re-review):** the settlement cap was changed
to *last closed day* and the `DayNotClosed` guards were removed (the cap now
self-limits to a closed day). This touches the reward-settlement core and is queued
for audit round 4. See [docs/CHANGES-AFTER-AUDIT3.md](docs/CHANGES-AFTER-AUDIT3.md).

Status: **closed-testnet phase; not yet deployed to mainnet.** Found something?
Please open a private security advisory on GitHub rather than a public issue.

## Repository layout

```
programs/anl_staking/   Anchor program (instructions, state)
crates/anl-math/        pure math (APY, XNT index, splits) + property tests
core/                   reference model (declared periods, settle, forfeit)
docs/whitepaper/        White Paper v1.2 (web + PDF, PL + EN)
docs/audits/            archived auditor reports
website/testnet/        testnet dApp front-end
scripts/                build-testnet / build-mainnet / audit-evidence
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
