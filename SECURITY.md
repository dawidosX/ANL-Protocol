# Security & Bug Bounty — ANL Staking Protocol (X1 testnet)

[PL](SECURITY.pl.md) | **EN**

**Code status:** audit-freeze `v1.0-testnet-freeze` — `src_tree 4c2256398137bb417a1b769316137852d14ec4d5`, program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`, binary `87b431d4…30a3`, slot 185899744.
**Audits:** 7 rounds (2026-08/09), four independent auditors — reports in `docs/audits/`. Three freeze confirmations (9 / 9 / 9.3 out of 10).

We reward bugs found in the **frozen code** that will become the mainnet base. Whoever finds something now helps fix it before it is too late.

---

## 1. Rewards

| Severity | Reward | What qualifies |
|---|---|---|
| **Critical** | **1,000,000 ANL** | draining tokens from any vault (principal / reward / XNT / CAPY) without authorization; payout above entitlement; double claim; taking over or overwriting another user's position; taking over `authority` |
| **High** | 250,000 ANL | permanent lock of a user's funds or of the whole staking without the authority key (liveness); bypass of the `initialize` pin, the 200M cap, the cooldown or the Genesis lock |
| **Medium** | 50,000 ANL | under- or over-payment that depends on transaction ordering; a new griefing vector with real cost to other users; ledger desynchronisation (reservations, indexes, checkpoints) |
| **Low** | 10,000 ANL | other bugs with a real, reproducible on-chain effect (including inconsistent error codes, dead paths enabling abuse) |

Severity is set by the team together with one of the protocol's independent auditors. The first valid report of a given bug receives the reward; duplicates do not. Payout after the fix and re-audit (not upon report).

## 2. Scope

**In scope:** the `anl_staking` program on X1 testnet (`programs/anl_staking/`, `crates/anl-math/`) at `src_tree 4c225639…`. Source code, tests and harness: this repository (`programs/anl_staking/tests/integration.rs`, `Env`).

**Out of scope:** the `website/` frontend, the public X1 RPC (limits, availability), hosting infrastructure, keys and operational procedures (the single hot key on testnet is known — F-02), the ANL/XNT/CAPY tokens themselves, social engineering.

## 3. Known and consciously accepted (do NOT qualify)

Documented in `docs/CHANGES-AFTER-ROUND{4,5,6,7}.md` and the audit reports:

- Genesis windows (`claim_genesis_window`) without a day roll — self-correcting in the next window / final claim
- rounding dust (floor) left in the vaults; no `sweep` instruction
- the expired position's share (orphan) is distributed to stakers alive **at the moment of** `settle_expired` — the dependence on settlement timing is inherent (bot SLA)
- empty pool ⇒ 100% of the XNT funding goes to the other pool (M-03)
- positions opened before 2026-09-04 with the old `end_epoch` formula (grandfathering, testnet only)
- pause applies only to `stake` (design: exit always works)
- entering mid-day counts that day, exiting mid-day does not (a position for N days = exactly N baskets)
- Genesis up to 3650 days in window 1 reserves up to 200% of principal (capital is genuinely locked)
- no `sweep` of ANL in excess of 200M in the reward vault (operational)
- `DayNotClosed` — dead error variant (error-code stability)
- RustSec advisories allowed in `.cargo/audit.toml` with justification (dev-deps / host, outside the SBF artifact)

If you believe one of the above is nevertheless **exploitable** (e.g. yields theft or a permanent lock) — report it with the sequence; then it qualifies.

## 4. Required proof

A report must contain a **reproducible PoC**: a test in the `Env` harness (preferred — `cargo test -p anl_staking --features test-periods --test integration`) **or** a sequence of transactions on X1 testnet with signatures. Describe: what the attacker does, the financial effect (how much, from which vault, whose funds), the assumptions. "I think that…" without a reproduction does not qualify.

## 5. Responsible disclosure rules

- Report **privately only**: a direct message (DM) to the admin of the Telegram group **https://t.me/ANLprotocol** (join the group, message the admin privately).
- **Do not post the bug in the group or publicly before the fix — a report posted in the group is treated as disclosure and does not qualify for a reward.**
- We respond within 72 h; severity assessment within 7 days; fix and re-audit within 30 days; public disclosure after the fix, no later than 90 days from the report — with credit (upon consent).
- Do not test on other users' positions or funds on testnet in any way that causes lasting harm; do not DoS the public RPC.
- Reward in ANL, paid to the reporter's address after the fix. Payout of the equivalent in XNT is possible — to be agreed at report time.

## 6. Announcement (for the website / X: https://x.com/ANLProtocol / X1 Discord)

> **ANL Staking Protocol — bug bounty up to 1,000,000 ANL.** The staking code on X1 testnet has passed 7 audit rounds and is frozen (`src_tree 4c225639…`). Before it reaches mainnet, we pay for finding bugs in it: Critical 1,000,000 ANL · High 250,000 · Medium 50,000 · Low 10,000. Scope, exclusions and rules: `SECURITY.md` (PL: `SECURITY.pl.md`) in the repo `github.com/dawidosX/ANL-Protocol`. Reports **only by direct message (DM) to the admin of the Telegram group https://t.me/ANLprotocol** — a post in the group or in public before the fix = disclosure, no reward. A PoC as a test in our harness is welcome.

---
*Version 1.2 — 2026-09-05 (contact: Telegram DM, group link; Polish version in `SECURITY.pl.md`). Changes to scope/rewards are announced in this file with a date.*
