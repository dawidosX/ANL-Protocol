# Security & Bug Bounty — ANL Staking Protocol (X1 testnet)

[PL](SECURITY.pl.md) | **EN**

**Code status (current bounty target):** `v1.3.2-testnet-freeze` — `src_tree 32e1e6f1234314aebe785961cfb4d371428c656a`, program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`, binary sha256 `cbc34c1b6ecea53b0bcc746657e8c9ea1ff984bb55ba9c1a826b6bc7b6c278e4` (truncated to `so_size` 714,960 B), slot 186827491, tag `v1.3.2-testnet-freeze`. Previous freezes: `v1.0-testnet-freeze` (`4c225639…`, slot 185899744), `v1.1-testnet-freeze` (`7ab2a745…`, slot 185933070), `v1.3.1-testnet-freeze` (`e82b34b3…`, slot 186156990) — fixed findings in `docs/BOUNTY-LEDGER.md`.
**Audits:** 11 rounds (2026-07/09), independent auditors — reports in `docs/audits/`. v1.3.2 confirmations (R11): DRAINABLE: NO ×3, deploy YES ×3; v1.3.1 (R10 / R10.1): 9.5 / 9.3 / 9.0 out of 10.

We reward bugs found in the **frozen code** that will become the mainnet base. Whoever finds something now helps fix it before it is too late.
**Public bounty ledger (no personal data):** [`docs/BOUNTY-LEDGER.md`](docs/BOUNTY-LEDGER.md).


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

**In scope:** the `anl_staking` program on X1 testnet (`programs/anl_staking/`, `crates/anl-math/`) at `src_tree 32e1e6f1…` (tag `v1.3.2-testnet-freeze`). Source code, tests and harness: this repository (`programs/anl_staking/tests/integration.rs`, `Env`).

**Freeze vs the LIVE build.** The frozen `src_tree` is the bounty target, but fixes ship as new versions (v1.1 → v1.2 → v1.3.1) and are deployed to testnet after re-audit. **Check what is actually deployed on-chain before you report** — a finding reproduced on the old tree but already fixed in the live version does not qualify (see `docs/BOUNTY-LEDGER.md`). The live version is identified by three things: the deploy slot, the binary sha256 and the `src_tree` in `release-manifest-testnet.txt` on `main` (plus the `v*-testnet-freeze` tags):

```bash
solana program show 4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM -u https://rpc.testnet.x1.xyz   # Last Deployed In Slot
solana program dump 4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM anl.so -u https://rpc.testnet.x1.xyz
head -c <binary_size> anl.so | shasum -a 256          # == the sha256 field in release-manifest-testnet.txt
git rev-parse <tag>:programs/anl_staking/src           # == the src_tree field in the manifest
```

**Note on the binary sha256:** `solana program dump` returns the WHOLE program-data account (zero-padded to the size allocated at the first deploy, e.g. 813,608 B), so the sha256 of the full dump does NOT match the manifest. Compute the hash over the dump **truncated to the size of the deployed binary** (`head -c <size>`); the size is the `so_size` field in `release-manifest-testnet.txt` (= the length of `target/deploy/anl_staking.so` from a reproducible build of the tag, `scripts/build-testnet.sh`, platform-tools v1.41; v1.3.2: 714,960 B, sha `cbc34c1b…`, slot 186827491). Everything in the dump past that size must be zeros.

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

> **ANL Staking Protocol — bug bounty up to 1,000,000 ANL.** The staking code on X1 testnet has passed 11 audit rounds and is frozen (`src_tree 32e1e6f1…`, tag `v1.3.2-testnet-freeze`). Before it reaches mainnet, we pay for finding bugs in it: Critical 1,000,000 ANL · High 250,000 · Medium 50,000 · Low 10,000. Scope, exclusions and rules: `SECURITY.md` (PL: `SECURITY.pl.md`) in the repo `github.com/dawidosX/ANL-Protocol`. Reports **only by direct message (DM) to the admin of the Telegram group https://t.me/ANLprotocol** — a post in the group or in public before the fix = disclosure, no reward. A PoC as a test in our harness is welcome.

---
*Version 1.2 — 2026-09-05 (contact: Telegram DM, group link; Polish version in `SECURITY.pl.md`). Changes to scope/rewards are announced in this file with a date.*
