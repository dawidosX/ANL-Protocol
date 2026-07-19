# Security Audit History — ANL Staking Protocol

**Status:** living document · last updated **19 Jul 2026**
**Scope:** the on-chain program `anl_staking` (Rust / Anchor), its math crate `anl-math`, the reference model in `core/`, CI, and the release/evidence tooling in `scripts/`.
**Polish version:** [SECURITY-AUDITS.pl.md](SECURITY-AUDITS.pl.md)

> This document supersedes the old, append-only `AUDIT-RESPONSE.md` notes. It presents the full audit trail chronologically: every round, every finding, its fix, and the evidence. The consolidated findings table is in [§8](#8-consolidated-findings-table); open items are in [§9](#9-open-items).

---

## 1. Methodology

The protocol undergoes iterative, AI-assisted security audits by independent reviewers (different models than the one implementing the code), each round working on a fresh snapshot of the repository. Each round produces a written report; the team responds with code fixes, records them here, and submits the updated snapshot for re-review. Supporting evidence lives in the repository itself: the CI pipeline (4 jobs: lint / test / release-guards / supply-chain), `scripts/audit-evidence.sh` (fmt, clippy `-D warnings`, all test suites, negative builds, cargo audit/deny), and `docs/TEST-LOG.txt`.

Severity conventions follow the reviewers' reports: **Critical** (funds at risk under realistic conditions), **High/H**, **Medium/M**, **Low/L**, and **process findings** (evidence/release pipeline rather than on-chain logic).

---

## 2. Round #1 — preliminary audit (GPT), 18 Jul 2026

First external review of the complete Phase 1+2 implementation (all lifecycle instructions, three vaults, ANL reward reservation, daily XNT engine). The report contained **9 findings**. The team's assessment: a solid, honest audit — every point actionable, and finding #1 a genuine catch.

| # | Finding | Severity | Initial disposition |
|---|---------|----------|---------------------|
| 1 | XNT accrual past `end_ts` depends on bot discipline: if daily funding arrives after a position's end, inline settle computes from an inflated index and pays out XNT belonging to others | **Critical** | Accepted; required an accounting redesign (see §4–5) |
| 2 | `fund_xnt` required the `authority` signature — forcing a hot multisig/Ledger key into a daily automated path | Medium | Fixed the same evening (operator role); the audited snapshot predated the fix |
| 3 | `declare_id!` is a placeholder Program ID | Info | Deliberate pre-deploy state; moved to the deployment checklist |
| 4 | ANL mint's Token-2022 extensions unvalidated (PermanentDelegate / TransferHook / TransferFee could subvert vault accounting) | High | Accepted; fixed (extension gate) |
| 5 | `test-periods` build safeguards insufficient — a warning log is not a safeguard | High | Accepted; fixed (hard guard test, later compile-time guards) |
| 6 | No verifiable/reproducible build | Medium | Moved to the deployment checklist |
| 7 | No account `version` checks in instructions | Medium | Accepted; fixed |
| 8 | Incomplete token-vault account constraints | Medium | Accepted; fixed |
| 9 | Pause policy not transparently communicated to users | Low | Accepted; whitepaper governance section |

## 3. Fixes after Round #1 (18 Jul 2026)

* **Operator role (finding 2):** `set_operator(new_operator)` callable by `authority` (multisig/Ledger); `fund_rewards`/`fund_xnt` accept authority **or** operator. The operator is a funding-only hot key — its compromise cannot touch user funds. (`instructions/fund.rs`, `state`)
* **Token-2022 mint extension gate (finding 4):** `initialize` unpacks the ANL mint with `StateWithExtensions` and enforces an allowlist — only passive metadata extensions (`MetadataPointer`, `TokenMetadata`) are accepted; `PermanentDelegate`, `TransferHook`, `TransferFee`, any unknown extension, and a set freeze authority are rejected (`ForbiddenMintExtension`, `MintHasFreezeAuthority`). (`instructions/initialize.rs`)
* **Account versioning (finding 7):** every instruction context enforces `version == ACCOUNT_VERSION` (`InvalidAccountVersion`).
* **Full vault constraints (finding 8):** every vault account in every context is constrained by mint + PDA authority + token program.
* **Production constants guard (finding 5, first stage):** test `production_constants_guard` compiled only in the default (production) variant asserts windows 31/91 days and min. period 7 days; CI runs it on every push — a release artifact that fails it is not a production artifact.
* Findings 3 and 6 entered the hard deployment checklist; finding 9 entered the whitepaper (governance/pause section).
* Test status after fixes: anl-math 24/24 (both variants), core 34/34, integration green. Finding **#1 remained open by design**, with a fix proposal (expiry buckets per pool×day) sent to the auditor together with the updated snapshot.

---

## 4. Round #2 — review of the fixed snapshot (Grok), 18–19 Jul 2026

Independent second review of the post-fix repository. **Score: 8.5/10.** The round-1 fixes were confirmed; finding **#1 (XNT accrual past `end_ts`)** was confirmed as the one remaining critical issue, and property-based/fuzz testing of the XNT accounting was recommended (a recommendation later repeated by every reviewer — see §9). The response to this round was not a patch but a redesign: the XNT epoch model below.

## 5. The XNT epoch model — closing Critical #1

The daily-basket accounting was rebuilt around the X1 network's native settlement unit, the **epoch**:

* **Checkpoints per pool×epoch.** Dedicated PDA accounts record the cumulative XNT index (`acc-per-share`) at the close of each epoch for each pool.
* **`fund_xnt(amount, epoch)`.** Funding is now explicitly attributed to an epoch and rolls forward the required checkpoints (`roll_checkpoint`); the instruction takes the checkpoint accounts it touches.
* **`end_epoch = epoch_of(end_ts − 1)`.** A position accrues XNT for **full epochs** up to and including the epoch in which its period ends; the ANL stream still stops exactly at `end_ts`. Both READMEs document this asymmetry explicitly.
* **`settlement_cap_index`.** Settlement (whether via `settle_expired`, inline settle in `claim`, or `unstake_early`) computes XNT from the index **capped at the position's end-epoch checkpoint**, never from the live index. Late funding therefore cannot credit a matured position with XNT from epochs after its end — the guarantee is enforced by contract arithmetic, not by bot uptime.
* **`epoch_of` returns `Option<u64>`** — timestamps before genesis map to an explicit `BeforeGenesis` error instead of a silent fallback.

With this model, the bot's failure mode degrades gracefully: a bot outage delays distribution but can no longer misattribute it.

---

## 6. Round #3 — detailed audit, 19 Jul 2026

In-depth review of the epoch-model implementation. **Score: 6.8/10** (stricter methodology and scope than round #2; the score reflects process maturity as much as code). Four findings:

| ID | Finding | Severity |
|----|---------|----------|
| **M-01** | `FundXnt` context did not enforce `ACCOUNT_VERSION` on the two pool accounts | Medium |
| **M-02** | Documentation out of sync with the epoch model: `end_ts` vs `end_epoch` semantics stated inconsistently; stale test counts | Medium (docs) |
| **L-01** | Checkpoint accounts read without an explicit program-owner check (`settlement_cap_index`, `roll_checkpoint` paths) | Low |
| **H-01** | No compile-time exclusion of `test-periods` on mainnet builds — the guard was procedural only | High |

## 7. Fixes after Round #3 + two independent verifications (19 Jul 2026)

All four findings were fixed and re-verified by **two independent reviewers**, each working on the final package: a source-focused verification (GPT, stored as `docs/audits/audit-3-verification-gpt.md`) and a process-focused verification (Grok, *"ANL Protocol — weryfikacja zmian po audycie #3"*, stored as `docs/audits/audit-3-verification-grok.pdf`). Both confirm the code fixes; they diverge only on documentation/process residue, and both views are recorded below.

* **M-01 — fixed.** `constraint = genesis_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion` and the Flexible equivalent. Evidence: `programs/anl_staking/src/instructions/fund.rs:124-140`.¹
* **L-01 — fixed.** `require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch)` at both checkpoint read sites. Evidence: `programs/anl_staking/src/instructions/lifecycle.rs:69-72` (`settlement_cap_index`) and `programs/anl_staking/src/instructions/fund.rs:196-208` (`roll_checkpoint`).¹
* **H-01 — fixed at the cfg level.** `compile_error!` guards: `network-mainnet` + `test-periods` cannot coexist, and exactly one network feature must be selected. Evidence: `programs/anl_staking/src/lib.rs:11-15`, `programs/anl_staking/Cargo.toml:11-18`.¹ `docs/TEST-LOG.txt` carries the raw negative proof (`cargo check … --features network-mainnet,test-periods` → the exact `compile_error!` message). CI's release-guards job builds the forbidden combination, asserts a non-zero exit **and** the exact message (`.github/workflows/ci.yml:46-59`¹), and additionally compiles both positive variants so a broken cfg cannot block all builds.
* **M-02 — fixed.** Both READMEs now state the rule unambiguously: the ANL stream stops exactly at `end_ts`, while XNT settles by full epochs up to `end_epoch = epoch_of(end_ts − 1)` (blockquote in both languages); the old "both streams stop at `end_ts`" phrasing is gone, and the summary-table test counts are synchronized (24/24 anl-math, 4/4 integration). Evidence: `README.md:19-20,87`, `README.pl.md:19-20,89`.¹ One residue found by the Grok verification: the build-section comment still reads `# math (23)` (`README.md:74-80`¹) — tracked under V-05.
* **No regressions found** in the checkpoint model, the instruction surface (handler renames were internal-only; `#[program]` function names unchanged, so instruction discriminators are unaffected — final IDL comparison still recommended before deploy), or the `epoch_of → Option<u64>` change.

The Grok verification's headline: *the remaining problem is no longer staking logic but the evidence chain from a clean commit to the deployed binary.* New **process findings** from that verification (all fixed the same day — see the fix list below and §8):

* **M-EVIDENCE-01** — the CI supply-chain job runs `cargo audit || true` and `cargo deny … || true`, so a vulnerability or banned dependency does not turn CI red (`.github/workflows/ci.yml:71-83`¹).
* **M-EVIDENCE-02** — `scripts/audit-evidence.sh` is not fail-closed: `set -uo pipefail` (no `-e`), no clean-tree gate, it overwrites the tracked `docs/TEST-LOG.txt` before checking `git status`, and prints `GOTOWE` (with exit 0) even after failed steps.
* `scripts/build-mainnet.sh` checks cleanliness with `git diff --quiet` (misses staged and untracked changes); `scripts/build-testnet.sh` does not check cleanliness at all. Correct gate: `test -z "$(git status --porcelain)"`.
* The second release-guard (mainnet+testnet together) asserts only a non-zero exit, not the specific message `select exactly one network feature`.
* README (both languages) still documents plain `anchor build` paths that bypass the release scripts, still carries the stale test count "23" (actual: 24), and does not yet describe the network features / release-script policy.
* The `docs/TEST-LOG.txt` attached to the audited package began with a real `cargo fmt --check` diff — and the diff was genuine: `instructions/initialize.rs` had been left unformatted, which also turned the CI lint job red from commit `27cd983` until the file was formatted on 19 Jul 2026. The old evidence script masked exactly this class of failure (M-EVIDENCE-02 in action).

**Evidence-pipeline fixes (19 Jul 2026, same-day response):** `scripts/audit-evidence.sh` rewritten fail-closed (`set -euo pipefail`, clean-tree gate via `git status --porcelain` before anything runs, log written to a temp file outside the repo, negative builds assert exit code **and** exact `compile_error!` message, footer binds the run to `git rev-parse HEAD`, `docs/TEST-LOG.sha256` records the log hash, non-zero exit on any failure); clean-tree gates (`git status --porcelain`) added to both `scripts/build-*.sh`; `|| true` removed from the supply-chain CI job and an approved `deny.toml` committed to the repo; the second release-guard now asserts the message `select exactly one network feature`; README (EN+PL) rewritten to the release-script policy with the stale "23" corrected to 24.

**First honest supply-chain run (19 Jul 2026):** with `|| true` removed, `cargo audit` scanned 606 locked dependencies and reported **8 vulnerabilities + 16 informational warnings** — all in the Solana 1.x client/test stack, none introduced by this codebase: `ed25519-dalek 1.0.1` (RUSTSEC-2022-0093), `curve25519-dalek 3.2.1` (RUSTSEC-2024-0344), `ring 0.16.20` (RUSTSEC-2025-0009), `rustls-webpki 0.101.7` (RUSTSEC-2026-0098/0099/0104), `quinn-proto 0.10.6` (RUSTSEC-2026-0037, high 8.7; RUSTSEC-2026-0185). Triage: these crates form the RPC/QUIC/TLS client networking and SDK secret-key layers; the on-chain SBF artifact performs no private-key operations and no TLS/QUIC networking, so the exposure is developer tooling and tests, not the deployed program. Interim measure: **documented, quarterly-reviewed ignores** (next review 2026-10-19): the 8 vulnerabilities in `.cargo/audit.toml`, and in `deny.toml` the same 8 plus the 16 informational IDs, since cargo-deny (advisories config v2) treats unmaintained/unsound advisories as errors rather than warnings. Ignores are per-ID only — any new advisory of any type still blocks CI. Permanent fix — a Solana/Anchor stack upgrade — is tracked in §9 and gates the mainnet DoD (goal: empty ignore lists).

**Verdicts (19 Jul 2026).** *GPT verification:* testnet / closed pilot **ready** (with a separate Program ID, strictly limited asset value and monitoring); no open round-#3 code findings block immutability — immutable mainnet becomes reachable once its 9-point Definition of Done (§9) is satisfied. *Grok verification:* closed testnet **conditionally ready** once the evidence pipeline is fixed; immutable mainnet **not ready** until the commit→binary chain is fail-closed. **Team position (adopted):** the stricter reading wins — V-01…V-05 were fixed the same day (see above), and mainnet remains gated on the full Definition of Done.

¹ Line numbers as cited in the 19 Jul 2026 verification reports, valid for the audited snapshots (`27cd983`…`ddf4b36`); they may drift with subsequent commits — commit + file path + symbol are authoritative (round #4, DOC-05).

### Round #4 — re-verification of the `audit4` package (19 Jul 2026, GPT + Grok, commit `ddf4b36`)

Both reviewers verified the sources independently (archived: `docs/audits/audit-4-verification-gpt.md`, `docs/audits/audit-4-verification-grok.pdf`). Agreement: **V-01…V-05 closed in source**; the rewritten `audit-evidence.sh` has no bypass (tee+`pipefail`, the `core` subshell and `expect_fail` were explicitly examined); the supply-chain triage is acceptable for a closed testnet — Grok additionally confirmed **from `Cargo.lock`** that all 8 advisories enter via dev-dependencies (`solana-sdk`, `solana-program-test`) and are absent from the SBF artifact, with the caveat that this acceptance must not be auto-extended to separate infrastructure (noted: the daily bot is TypeScript/`@solana/web3.js` and does not link the Rust SDK stack; it never constructs keypairs from independently supplied halves). Both recommend **per-network `declare_id!` before the testnet deploy** — adopted (§9).

Grok's key catch: the audited package still carried the **stale `docs/TEST-LOG.txt` from `27cd983`** — a log in which every cargo command ended in `command not found` and the *old* evidence script still counted a missing compiler as a passing negative test (M-EVIDENCE-02 illustrated perfectly) — and no `TEST-LOG.sha256`. Hence the fresh evidence run is a hard pre-testnet condition. Script-hardening suggestions were applied the same day: HEAD + clean-tree re-checked before publishing the log, tool versions and the `Cargo.lock` hash recorded, temp log cleaned up on success, `rm -f` of the previous binary + `test -s` after build in both release scripts.

Documentation findings **DOC-01…DOC-05** accepted and corrected in this revision (R1-07 wording narrowed; R1-05 evidence split into the hard guard vs the procedural exclusion; the pipeline status re-worded as "completed in source, run pending"; footnote anchored to commits). On **DOC-04** (alleged over-positive attribution of GPT's earlier verdict): the archived report `docs/audits/audit-3-verification-gpt.md` states verbatim "Testnet / zamknięty pilot: **gotowy**", which this document quotes accurately; Grok's objection is recorded here for transparency.

**Verdicts (round #4).** *GPT:* closed testnet **ready unconditionally** (separate testnet Program ID, monitoring, epoch-model comms). *Grok:* **conditionally ready** — pending the fresh evidence run bound to the final HEAD and a separate testnet Program ID; immutable mainnet not ready. **Team position: the stricter reading wins again** — both conditions are explicit items of the §9 deployment checklist.

---

## 8. Consolidated findings table

Severity: C = Critical, H = High, M = Medium, L = Low, I = Info, P = process. Status: ✅ fixed & verified, 🟡 open (tracked in §9), 📋 deployment checklist.

| ID | Round | Sev | Finding | Status | Evidence / fix location |
|----|-------|-----|---------|--------|--------------------------|
| R1-01 | 1 | C | XNT accrual past `end_ts` bot-dependent | ✅ | Epoch model (§5): checkpoints per pool×epoch, `end_epoch = epoch_of(end_ts−1)`, `settlement_cap_index`; `instructions/fund.rs`, `instructions/lifecycle.rs`, `state/mod.rs` |
| R1-02 | 1 | M | Daily funding required the authority key | ✅ | Operator role: `set_operator`; `instructions/fund.rs`, `lib.rs` |
| R1-03 | 1 | I | Placeholder Program ID | 📋 | `anchor keys sync` at deploy; separate IDs for testnet/mainnet builds |
| R1-04 | 1 | H | ANL mint Token-2022 extensions unvalidated | ✅ | Allowlist gate in `instructions/initialize.rs` (`ForbiddenMintExtension`, `MintHasFreezeAuthority`) |
| R1-05 | 1 | H | `test-periods` safeguards log-only | ✅ | Hard compile guard for the mainnet variant: H-01 `compile_error!` (`lib.rs:11-15`¹) + `production_constants_guard` test; builds without a network feature are excluded from deployment **procedurally** (release only via `scripts/build-*.sh`; per-network `declare_id!` planned — see §9) |
| R1-06 | 1 | M | No verifiable build | 📋 | Deployment checklist (§9) |
| R1-07 | 1 | M | No account version checks | ✅ | `version == ACCOUNT_VERSION` for all versioned state accounts in the audited instruction contexts (`UserProfile` is intentionally unversioned); the gap later found in `FundXnt` closed as R3-M-01 |
| R1-08 | 1 | M | Incomplete vault constraints | ✅ | Mint + PDA authority + token program constraints in every context |
| R1-09 | 1 | L | Pause policy transparency | ✅ | Whitepaper governance section; exit paths (`claim`, `unstake_early`, `settle_expired`) always work |
| R3-M-01 | 3 | M | `FundXnt` missing pool version constraints | ✅ | `instructions/fund.rs:124-140`¹ |
| R3-M-02 | 3 | M | Docs out of sync: `end_ts`/`end_epoch` semantics, stale test counts | ✅ | `README.md:19-20,87`, `README.pl.md:19-20,89`¹; residual `# math (23)` comment → V-05 |
| R3-L-01 | 3 | L | Checkpoint reads without owner check | ✅ | `instructions/lifecycle.rs:69-72`, `instructions/fund.rs:196-208`¹ |
| R3-H-01 | 3 | H | No compile-time mainnet×test-periods exclusion | ✅ | `lib.rs:11-15`, `Cargo.toml:11-18`¹; CI release-guards `.github/workflows/ci.yml:46-59`¹ |
| V-01 | 3-ver | P/M | Supply-chain CI non-blocking (`\|\| true`) | ✅ | `\|\| true` removed, approved `deny.toml` committed; `.github/workflows/ci.yml` (supply-chain job), `deny.toml` |
| V-02 | 3-ver | P/M | `audit-evidence.sh` not fail-closed | ✅ | Rewritten fail-closed: `set -euo`, clean-tree gate, temp log, HEAD + `TEST-LOG.sha256`; `scripts/audit-evidence.sh` |
| V-03 | 3-ver | P | Build scripts' clean-tree checks insufficient | ✅ | `git status --porcelain` gate in both; `scripts/build-mainnet.sh`, `scripts/build-testnet.sh` |
| V-04 | 3-ver | P | 2nd release-guard doesn't assert the error message | ✅ | Asserts `select exactly one network feature`; `.github/workflows/ci.yml` (release-guards) |
| V-05 | 3-ver | P | README: plain `anchor build` paths, stale "23", missing release policy | ✅ | Build sections rewritten to release-script policy, "23"→24, network features documented; `README.md`, `README.pl.md` |
| R4-DOC-01…05 | 4 | P | Documentation-accuracy findings (over-broad R1-07 wording, R1-05 evidence split, pipeline "completed" vs "run pending", verdict attribution, line-number drift) | ✅ | Corrected in this revision — see Round #4 section |
| R4-ID | 4 | P | Per-network `declare_id!` (mainnet / testnet / dev-only placeholder) so a no-feature build cannot land on a deployed address | 📋 | Adopted; deployment-checklist item, **before testnet** |

---

## 9. Open items

**Evidence & release pipeline — ✅ tooling completed in source, 19 Jul 2026** (V-01…V-05; confirmed by round #4). The **fresh evidence run** — `audit-evidence.sh` executed on the final HEAD, with `docs/TEST-LOG.txt` + `docs/TEST-LOG.sha256` committed — remains a deployment-checklist item and a hard pre-testnet condition (round #4, DOC-03): fail-closed `audit-evidence.sh`, clean-tree gates in both build scripts, blocking supply-chain with a committed `deny.toml`, message assertion in the second release-guard, README release policy with corrected test counts. Details in §7 and the table above.

**Dependency stack (from the first blocking supply-chain run):**
1. Upgrade the Solana/Anchor dependency stack so the 8 ignored RUSTSEC advisories (see §7) drop out of the tree; empty both ignore lists. Required before the immutable-mainnet DoD; ignores expire at the quarterly review (next: 2026-10-19).

**Testing — ✅ completed 19 Jul 2026:**
2. Property-based tests of the XNT accounting invariants (recommended by all three audit rounds): `crates/anl-math/tests/properties.rs` (10 properties: 65/35 conservation and monotonicity, index monotonicity, **no-inflation** of `pending_xnt` with a bounded rounding loss, `period_reward` linearity within floor, APY windows on the variant's own constants) and `core/tests/properties.rs` (2 randomized machines: an operation-sequence machine asserting after every op that index never decreases, share sums stay consistent and **paid + undistributed + owed ≤ funded**; and an epoch-checkpoint model proving the audit-#1 property — funding epochs after a position's `end_epoch` never changes its settlement). Runs in both variants inside the existing CI test job and `audit-evidence.sh`; deeper coverage-guided fuzzing of the on-chain crate remains a pre-mainnet option.

**Operations:**
3. Update the daily bot (`/opt/anl-bot/`, private W5 environment) to the new `fund_xnt(amount, epoch)` signature, checkpoint accounts, and checkpointing at settle — the current bot predates the epoch model and is incompatible with the contract.

**Deployment checklist (before testnet):**
4. Final Program IDs via `anchor keys sync` with **per-network `declare_id!`** (`#[cfg(feature = "network-mainnet")]` / `network-testnet` / dev-only placeholder for feature-less builds), per-cluster `Anchor.toml`, rebuild, re-verification of all PDAs (R1-03 + R4-ID; both round-#4 reviewers: do this **before** the testnet deploy).
5. Tagged, clean commit; full CI run on that exact commit; `docs/TEST-LOG.txt` regenerated by the fixed evidence script on a real Git checkout with HEAD recorded.
6. Compare the pre/post-rename IDL before deployment (instruction discriminators expected unchanged; verify).
7. Testnet observation period with valueless or strictly limited assets; upgrade authority retained under the multisig throughout — **no `--final`, no key deletion** at this stage.

**Definition of Done — immutable mainnet** (per the GPT verification of 19 Jul 2026, adopted by the team; every point must be green, in order):
1. Final Program ID (`anchor keys sync` + `declare_id!` + `Anchor.toml` + rebuild + verification of all PDAs).
2. `anchor build --verifiable` + `anchor verify` against X1 (loader-semantics confirmation) (R1-06).
3. Full run of the **fixed** `scripts/audit-evidence.sh` on a clean Git tree, output in `docs/TEST-LOG.txt` bound to `git rev-parse HEAD`.
4. All tests green on toolchain 1.89: anl-math 34 (24 unit + 10 property), core 36 (34 unit + 2 property), integration 4 — in both variants.
5. Negative guard builds (`network-mainnet`+`test-periods` and `network-mainnet`+`network-testnet`) still fail to compile.
6. `cargo clippy --workspace --all-targets -- -D warnings` clean in both variants.
7. `cargo audit` and `cargo deny check` with no critical/high findings — as **blocking** CI jobs (V-01) and with **empty ignore lists** (the stack upgrade from §9 completed).
8. Release manifest from `scripts/build-mainnet.sh` (HEAD + features + sha256 of the binary + rustc version) attached to the release notes.
9. Upgrade authority stays active until points 1–8 are all green; `--final` is the **last** step of this list and is never executed earlier (fleet rule: zero `--final` in the current phase — the finalization decision is taken explicitly, at the end, by the authority holder).

---

## 10. Document history

| Date | Change |
|------|--------|
| 19 Jul 2026 | First consolidated edition (EN+PL); supersedes the append-only `AUDIT-RESPONSE.md`; incorporates rounds #1–#3 and **both** independent round-#3 verification reports (GPT + Grok), incl. finding M-02 and the immutable-mainnet Definition of Done |
| 19 Jul 2026 (later) | V-01…V-05 fixed (evidence pipeline, CI, README); root cause of the red CI lint identified and fixed (unformatted `initialize.rs`); statuses and §9 updated |
| 19 Jul 2026 (later) | First blocking supply-chain run: 8 RUSTSEC advisories in the Solana 1.x tooling stack triaged; documented ignores in `.cargo/audit.toml` + `deny.toml`; stack-upgrade task added and wired into the DoD |
| 19 Jul 2026 (later) | Round #4 re-verification (GPT + Grok) incorporated: V-01…V-05 confirmed closed in source; DOC-01…05 corrected; script hardening applied; per-network `declare_id!` adopted into the checklist; fresh evidence run pinned as a hard pre-testnet condition |
| 19 Jul 2026 (later) | Property-based test suites added (anl-math: 10, core: 2 randomized machines incl. the epoch-cap immunity property) — the recommendation repeated by every audit round is closed; test counts updated across docs |
