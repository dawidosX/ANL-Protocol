# Zmiany po rundzie 6 → stan R7 (2026-09-05)

**Kod on-chain (testnet):** `src_tree b463b069…` (identyczne w `814e16b` i HEAD), binarka sha256
`332553d4236f3c75949980aeba5d9fcec1a08fafed69d9dba2adf4857cbd8b37`, slot **185887287**,
program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM` (zweryfikowane dumpem on-chain, 0 bajtów różnicy).
**Provenance drzewa:** `code_tree 7e04b873a4de2e195d26b3e53ae7a5c972246db0` (`HEAD:programs`),
`math_tree 6fb61151f3e10b0a5d68249a941721500b34a5b3` (`HEAD:crates/anl-math`) — zob. §6.
**Evidence:** `docs/TEST-LOG.txt` na HEAD `9152b54` — integracja **52/52** w obu reżimach, lib 11/11,
`anl-math` 24+10, `core` 34+2 (oba reżimy), clippy `-D warnings` ×2, fmt, `cargo audit` 0 podatności.

Źródła rundy 6: `docs/audits/2026-09-05_audyt-r6-adwersarialny_claude-fable5.md` (Claude Code, R6-01/R6-02/I-01…I-07),
raport C (HIGH pin `initialize`, MEDIUM limit 200M, uwagi bez zmiany kodu), Kimi R6 (Info: orphan vs timing settle).

---

## 1. Mapowanie ustaleń → zmiany kodu (wszystkie w `814e16b`, wdrożone)

| ID | Waga | Problem | Zmiana | Plik | Test |
|---|---|---|---|---|---|
| **R6-01** | High (liveness) | cap z checkpointu sfinalizowanego PRZED wejściem pozycji < `xnt_debt_index` (po `redistribute_to_live`) ⇒ `checked_sub` → `MathOverflow` blokował `settle_expired`/`claim`/`claim_genesis_window` aż do następnego `fund_xnt` (na stałe przy braku fundingu) | `cap_index_at` zwraca `ck.index.max(pos.xnt_debt_index)` — pozycja bez zafundowanej doby w [wejście, end_epoch] ma pending 0, cały jej udział to orphan dla żywych | `lifecycle.rs` | `regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim` (przed: `Custom(6022)`) |
| **R6-02** | Low (inwariant 9) | `fund_xnt` → `write_final_index` nadpisywał JUŻ sfinalizowany checkpoint bieżącym indeksem (z redystrybucjami po domknięciu) ⇒ dojrzała pozycja opóźniająca settle do fundingu brała orphan po swoim `end_epoch` (15 000 vs 10 000) | `add_to_basket` → `Result<Option<u64>>` (domknięta doba tylko gdy koszyk > 0); zapis finalnego indeksu wyłącznie dla `Some(closed_epoch)` | `state/mod.rs`, `fund.rs` | `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle` |
| **I-01 / HIGH (C)** | High (deploy) | `initialize` first-come: pierwszy wołający po deployu zostawał `authority` i wybierał minty | `require_keys_eq!(authority, EXPECTED_INIT_AUTHORITY)` w buildach sieciowych (`network-testnet`/`network-mainnet`); testy (bez feature sieci) używają kluczy lokalnych | `constants.rs`, `initialize.rs` | brak testu w suite (cfg sieciowy) — dowód: `cargo check --features network-testnet` w evidence + stała = `Dx2v…zvEm`; **do decyzji** (§7) |
| **MEDIUM (C)** | Medium (tokenomika) | emisja nagród ANL ograniczona tylko saldem `reward_vault` — nadmiar w skarbcu rozszerzał pulę ponad 200M | `stake`: `total_anl_paid + new_reserved ≤ ANL_REWARD_POOL` (saldo = pokrycie, stała = maksimum emisji) | `stake.rs` | `atak_r6_limit_200m_anl_niezaleznie_od_salda_skarbca` |
| **I-03** | Info (+) | to samo konto jako `prev_day_ckpt` i `xnt_checkpoint` | bez zmian — dowód, że duplikat nie podwaja ani nie zeruje | — | `r6_i03_ten_sam_ckpt_jako_prev_day_i_xnt_checkpoint` |
| **I-05** | Info (testy) | flaky `ts_split_init_intermediate_state_guards`: identyczne tx (e)/(f) bez nowego slotu ⇒ BanksClient zwracał zapamiętany wynik | `env.advance(1)` przed (f) | `tests/integration.rs` | — |
| **I-06** | Info (model) | granica dustu w `core/tests/properties.rs` nie liczyła floor per żywa pozycja (kontrprzykład proptest: 4 stake + 1 fund, strata 4 > 3) | `bound += liczba_żywych_pozycji`; seed w `core/tests/properties.proptest-regressions` (w repo) | `core/` | proptest zielony z odtworzeniem kontrprzykładu (oba reżimy) |
| **I-04** | Info (provenance) | manifest podawał rustc hosta, nie toolchain SBF | `build_sbf:` (platform-tools) w manifeście i TEST-LOG; `code_tree`/`math_tree` | `scripts/` | — |
| I-02, I-07 | Info | bufor `xnt_undistributed` przy wind-down; komunikat `InvalidPeriod` „7..=3650" | bez zmian (residual / kosmetyka) | — | — |

Konserwacja XNT po R6-01/R6-02: `test_r6_property_konserwacja_xnt_losowe_sekwencje` (40 ziaren × 120 operacji) bez naruszeń;
`core` proptest „brak inflacji" zielony.

## 2. Odpowiedzi na pozostałe punkty raportu C — **bez zmiany kodu**

### 2.1 „Stake po `fund_xnt` tej samej doby dostaje koszyk"
Zamierzone i **symetryczne**. Model A (WP): udział w dobie = shares na jej **koniec**; wejście w środku doby
zalicza tę dobę (rozwadnia obecnych), wyjście w środku doby ją **traci** (`end_epoch = epoch_of(end_ts) − 1`,
R4). Pozycja na N dni dostaje dokładnie N koszyków — ani doby wyjścia „za darmo", ani mniej.
Dowód: `regresja_r7_n_dni_dokladnie_n_koszykow` — stake mid-day (po fundingu doby 0) na MIN dni, funding
codziennie, claim ⇒ `3 250 × MIN` (Genesis 65 % z 10 000, dzielone z kotwicą), a nie `3 250 × (MIN+1)`.
Uwaga: gdy pula jest **pusta** w chwili fundingu, 100 % koszyka idzie do drugiej puli (M-03) — pierwszy
staker pustej puli nie ma z tej doby nic. To ta sama reguła, nie luka.

### 2.2 „Orphan przed opóźnionym settle" (Kimi R6 Info)
Po R5 M-01 orphan/przepadek trafia do shares **żywych w chwili settle**. Kto jest „żywy" zależy od momentu
wywołania permissionless `settle_expired` — inherentna zależność od timingu, nie od kolejności legalnych
instrukcji (R6-02 usunął jedyny wektor, gdzie kolejność zmieniała kwotę). Mitigacja operacyjna: bot
wykonuje `close_day → settle_expired` wszystkich dojrzałych pozycji **w tej samej dobie** (SLA bota).
Zdanie do WP / SECURITY-NOTES: „Udział wygasłej pozycji w dobach po jej `end_epoch` jest rozdzielany
pozycjom aktywnym w chwili jej rozliczenia; protokół rozlicza wygasłe pozycje codziennie."

### 2.3 Dependabot / `cargo audit` (Cargo.lock v3, 613 crate'ów, baza RustSec 1239 advisories, 2026-09-05)
Metoda: `cargo tree -i <crate> -e normal` (graf hosta) oraz **`cargo +solana tree --target sbf-solana-solana -e normal`**
(graf artefaktu on-chain; `solana-zk-token-sdk` i `solana-program` gate'ują większość zależności przez
`cfg(not(target_os = "solana"))`).

| Advisory | Crate | Typ | Ścieżka | W grafie **SBF**? | Ocena |
|---|---|---|---|---|---|
| RUSTSEC-2026-0258 **(nowe)** | h2 0.3.27 | vulnerability | reqwest ← solana-client / solana-program-test | **NIE** | dev-deps (HTTP/2 klienta RPC); dodane do `audit.toml` + `deny.toml` |
| RUSTSEC-2026-0037/0185 | quinn-proto 0.10.6 | vulnerability (high) | solana-client (QUIC) | NIE | dev-deps |
| RUSTSEC-2026-0098/0099/0104 | rustls-webpki 0.101.7 | vulnerability | TLS klienta RPC | NIE | dev-deps |
| RUSTSEC-2025-0009 | ring 0.16.20 | vulnerability | TLS klienta RPC | NIE | dev-deps |
| RUSTSEC-2022-0093 | ed25519-dalek 1.0.1 | vulnerability | solana-sdk (podpisywanie) | NIE | SDK/testy; program nie podpisuje |
| RUSTSEC-2024-0344 | curve25519-dalek 3.2.1 | vulnerability | solana-zk-token-sdk pod `cfg(not(solana))` | NIE | poza artefaktem |
| RUSTSEC-2023-0033 | **borsh 0.9.3** | unsound (ZST) | solana-program 1.18 (moduł kompatybilności `borsh0_9`) | **TAK** | **nieosiągalne (potwierdzone przez Recenzenta):** advisory = deserializacja `Vec<T>` dla ZST bez `Copy` (dzielenie przez `size_of::<T>() == 0`); nasze konta/argumenty nie mają ZST ani `Vec<ZST>` (u8…u128/i64/bool/Pubkey/enum/[u8;N]); Anchor 0.30.1 deserializuje borsh 0.10.x; borsh 0.9 tylko dla legacy API `solana-program`, którego nie wołamy |
| RUSTSEC-2025-0141 | **bincode 1.3.3** | unmaintained | solana-program, anchor-lang | **TAK** | brak podatności; znika z upgrade'em stosu |
| RUSTSEC-2023-0126, 2026-0248 | im 15.1.0 | unsound / unmaintained | solana-frozen-abi ← solana-sdk (`cfg(not(solana))`) | NIE | host |
| RUSTSEC-2026-0251/0255 | sized-chunks 0.6.5 | unmaintained / unsound | im | NIE | host |
| RUSTSEC-2026-0247 | bitmaps 2.1.0 | unmaintained | im, sized-chunks | NIE | host |
| RUSTSEC-2026-0253 | lru 0.7.8 | unsound | solana-program-test | NIE | dev-deps |
| RUSTSEC-2026-0191 | solana_rbpf 0.8.3 | unsound | solana-program-test (VM hosta) | NIE | dev-deps |
| RUSTSEC-2026-0186 | memmap2 0.5.10 | unsound | solana-accounts-db (host) | NIE | host |
| RUSTSEC-2026-0097 | rand 0.7.3 | unsound | zk-token-sdk `cfg(not(solana))` | NIE | host |
| pozostałe unmaintained (ansi_term, atty, derivative, paste, proc-macro-error, number_prefix, rustls-pemfile, libsecp256k1, ouroboros) | — | unmaintained/unsound | narzędzia / host | NIE | bez wpływu na artefakt |

Wniosek: z 31 advisories w `Cargo.lock` **dwa** dotyczą crate'ów kompilowanych do SBF (`borsh 0.9.3`, `bincode 1.3.3`),
oba bez znanej podatności wykorzystywalnej przez nasz kod. Cel „pusta lista" wymaga upgrade'u stosu Solana/Anchor
(otwarte, przed immutable mainnet). Listy `audit.toml`/`deny.toml` zsynchronizowane (9 podatności + informacyjne).

### 2.3b Dependabot (npm) — 2 nowe alerty po dodaniu `scripts/package-lock.json` do repo (R7.1)
Od R7.1 lockfile npm narzędzi operacyjnych jest śledzony, więc Dependabot skanuje także `scripts/`. Oba alerty dotyczą
zależności przechodnich `@solana/web3.js@1.98.x` → `jayson` (klient JSON-RPC); **żaden nie dotyczy programu on-chain**
(`programs/`, `crates/`) ani frontendu (`website/` ładuje web3.js z CDN, bez `jayson`/`stream-json`).

| Advisory | Pakiet | Waga | Ścieżka | Dotyka | Ocena / plan |
|---|---|---|---|---|---|
| GHSA-528h-pc64-c93x | `stream-json` ≤ 3.4.0 | moderate (DoS: filtry O(depth²) na zagnieżdżonym JSON) | `@solana/web3.js` → `jayson` → `stream-json` | tylko `scripts/*.js` (audyt-naliczen, diagnoza-user2, **fund-xnt** = bot operatora) | złośliwy/skompromitowany RPC mógłby zablokować pętlę zdarzeń bota (liveness `fund_xnt`), nie środki; mitigacja: własny/zaufany RPC (`RPC_URL`), bot pod supervisorem z restartem; upgrade po wydaniu `jayson` z poprawką (`npm audit fix --force` proponuje web3.js 0.0.3 — nie stosować) |
| GHSA-w5hq-g745-h8pq | `uuid` < 11.1.1 | moderate (brak kontroli granic bufora w v3/v5/v6 przy `buf`) | `@solana/web3.js` → `jayson` → `uuid` | jw. | `jayson` używa `uuid` do id żądań (v4, bez `buf`) — ścieżka podatna nieosiągalna w naszym użyciu; upgrade razem z `jayson` |

`cargo audit` (RustSec, baza 1239 advisories) bez nowych pozycji względem `deny.toml`/`.cargo/audit.toml`. Do backlogu:
`npm audit --prefix scripts` jako krok informacyjny w `audit-evidence.sh` (nie fail-closed — narzędzia, nie artefakt).

### 2.4 Provenance — hash drzewa zamiast commita
Zarzut „6 różnych commitów w jednej rundzie" (Kimi R6, C P-01) wynika z commitów **docs/manifestów** między buildem,
evidence i deployem. Od R7 skrypty zapisują `code_tree = git rev-parse HEAD:programs` i
`math_tree = HEAD:crates/anl-math` (TEST-LOG, manifest). Audytor porównuje **tree-hash**, nie commit:
`code_tree` zmienia się wyłącznie przy zmianie kodu lub testów programu. Dodatkowo `src_tree`
(`HEAD:programs/anl_staking/src`) identyfikuje sam kod on-chain — w R7 `b463b069…` dla `814e16b` (deploy) i HEAD.

Procedura freeze (obowiązująca): (1) jeden commit-freeze na `main`, drzewo czyste; (2) `audit-evidence.sh` →
TEST-LOG z `HEAD`, `code_tree`, `math_tree`; commit dowodu; (3) `build-testnet.sh` → manifest (sha `.so`,
`code_tree`, `build_sbf`); (4) deploy **przez Dawida**, `solana program show` → `deployed_slot` w manifeście;
(5) snapshot: `git archive`, `git bundle --all`, `pack-audit.sh` (manifest OBOK zipa). Reprodukowalność:
`cargo build-sbf` na `git archive <commit>` daje identyczny sha (R6: `855ca6a4…`, R7: `332553d4…`).

### 2.5 WP — TODO (decyzje produktowe, do naniesienia przez Dawida)
- Flexible: cooldown **3 dni** przed `unstake_early` (test-periods: 1 h) — było „wyjście w każdej chwili".
- Flexible: maksymalny okres **365 dni** (Genesis bez zmian: do 3650).
- Twardy limit emisji nagród ANL: **200 000 000 ANL** (wypłacone + zarezerwowane), niezależnie od salda skarbca.
- `initialize` przypięty do klucza z ceremonii (`EXPECTED_INIT_AUTHORITY`); na mainnecie = multisig.
- Rozliczanie wygasłych pozycji codziennie (SLA bota) — zdanie z §2.2.

## 3. Compute Units (bez zmian od R6: pomiar z testnetu)
Claim 60 237 · Stake 41 264 · ClaimGenesisWindow 28 945 · ClaimCapy 22 714 (limit 200 000; zapas > 3×).
R6-01/R6-02 nie dodają pętli ani kont — CU bez istotnej zmiany (pomiar po deployu R7: do uzupełnienia z logów).

## 4. Testy dodane w R6→R7
`regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim`, `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle`,
`r6_i03_ten_sam_ckpt_jako_prev_day_i_xnt_checkpoint`, `atak_r6_limit_200m_anl_niezaleznie_od_salda_skarbca`,
`regresja_r7_n_dni_dokladnie_n_koszykow`; `core`: granica dustu + seed regresji. Razem **52** integracyjnych.

## 5. Weryfikacja on-chain po deployu R7
`scripts/audyt-naliczen.js` (2026-09-05, po slocie 185887287): **7 portfeli, 134 pozycje, 1 flaga** — znana
`3svaA6…#11: APY 2000 != 800` (artefakt buildu sprzed 1.09, nie bug). Pozycje sprzed R4 oznaczone
`[end_epoch wg STAREJ formuly]` (grandfathering, M-02 R5).

## 6. Historia commitów R6→R7 (`main`)
| Commit | Treść | Zmienia `programs/src`? |
|---|---|---|
| `fe0df6d` | fix R6-01/R6-02 + testy R6 + raport audytu + I-05 | **TAK** |
| `cb4f08c` | pin `initialize` (`EXPECTED_INIT_AUTHORITY`), limit 200M, test, podaż testowa 1B | **TAK** |
| `814e16b` | pin tylko w buildach sieciowych | **TAK** (ostatnia zmiana kodu; `src_tree b463b069…`) |
| `e310e88` | manifest binarki R7 (`332553d4…`) | nie |
| `088705f` | core: granica dustu (I-06) | nie (`core/`) |
| `4f01fa3`, `5376108` | TEST-LOG evidence (51/51) | nie |
| `50f67ef` | deploy R7 slot 185887287 | nie |
| `1ab6954` | test `regresja_r7_n_dni_dokladnie_n_koszykow` | nie (`tests/`) |
| `807dd28` | `code_tree`/`math_tree`/platform-tools w skryptach | nie |
| `9152b54` | polityka supply-chain (h2 + 6 ostrzeżeń, weryfikacja grafu SBF) | nie |
| `bda077c` | TEST-LOG evidence na `9152b54` (52/52) | nie |
| `6e3021d` | manifest z `code_tree`, `build_sbf`, slot deployu bez zmian | nie |

## 7. Otwarte / do decyzji (poza kodem lub dla Recenzenta)
1. **Pin `initialize` bez testu w suite** — stała jest gate'owana feature sieciowym, a testy budują bez niego.
   Propozycja: test kompilowany pod `network-testnet` z kluczem testowym w `EXPECTED_INIT_AUTHORITY` przez `cfg`,
   albo akceptacja dowodu „cargo check + odczyt stałej" (obecnie). `EXPECTED_INIT_AUTHORITY` = klucz deployera
   (`Dx2v…zvEm`) = upgrade authority = operator — **jeden hot key** (F-02): na mainnecie multisig.
2. **`scripts/` w `.gitignore`** (wpis `scripts/`): pliki już śledzone działają, ale każdy **nowy** skrypt jest cicho
   pomijany przez `git add`. Zalecenie: usunąć wpis, ignorować tylko `scripts/node_modules/`.
3. `borsh 0.9.3` w grafie SBF (RUSTSEC-2023-0033) — prośba o potwierdzenie przez Recenzenta, że kompat-moduł
   `solana-program::borsh0_9` nie jest używany przez nasz kod (Anchor: borsh 0.10.4).
4. Upgrade stosu Solana/Anchor (opróżnia listy `audit.toml`/`deny.toml`) — przed immutable mainnet.
5. Pomiar CU po R7 z logów testnetu (delta do §3).
