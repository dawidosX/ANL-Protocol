# ADVERSARIAL SECURITY AUDIT — RUNDA 6 — ANL Staking Protocol

**Audytor:** Claude Fable 5.1 (Anthropic) · **Data:** 2026-09-05
**Paczka:** `ANL-Protocol-audyt-r6-91418cc.zip` · **Commit:** `91418cc436fde8b2e7e1fa829582f6947f840698`
**Środowisko:** cargo 1.97.1, anchor-cli 0.30.1, solana-cargo-build-sbf 1.18.17 (platform-tools v1.41), macOS

---

## 0. Provenance paczki — ZWERYFIKOWANA

| Element | Manifest | Zmierzone | Wynik |
|---|---|---|---|
| sha256 zipa | `d87d9883…5075e9` | `d87d9883…5075e9` | ✅ |
| sha256 `Cargo.lock` | `8b0f5d39…524343` | `8b0f5d39…524343` | ✅ |
| `programs/`, `crates/` w zipie vs `git show 91418cc` | — | `diff -rq` pusty | ✅ |
| sha256 `.so` (manifest / `target/deploy`) | `855ca6a4…4285fb` | `855ca6a4…4285fb` | ✅ |
| **Build reprodukowalny** z `git archive 91418cc` (`cargo build-sbf --features network-testnet,test-periods`) | `855ca6a4…4285fb` | `855ca6a4…4285fb` | ✅ bit w bit |
| `release-manifest-testnet.txt` podaje `head: 4609616` | — | `git diff 4609616 91418cc -- programs crates Cargo.*` pusty | ✅ kod identyczny |

Uwaga (I-04): pole `rustc:` w manifeście to rustc **hosta** (1.97.1). Binarkę SBF kompiluje
rustc z platform-tools (1.75.0). Reprodukcja się powiodła, ale manifest powinien zapisywać
`cargo build-sbf --version` / wersję platform-tools, bo to one determinują sha `.so`.

Baseline na HEAD przed zmianami audytora: integracja **47/47** w obu reżimach, clippy czysty.

---

## 1. Zakres i metoda

Przeczytany w całości kod programu (`lib.rs`, `constants.rs`, `errors.rs`, `state/mod.rs`,
`instructions/*`), crate `anl-math`, harness `tests/integration.rs`, dokumenty rund 4–5.
Każda z 18 instrukcji przeanalizowana pod kątem: kto woła, co przenosi, z jakiego skarbca,
podstawienie kont, wielokrotne użycie pozycji, kolejność stan/transfer, CPI, zegar,
overflow/rounding (tabela w §3). Hipotezy potwierdzone **wyłącznie testami na harnessie
`Env`** (prawdziwe transakcje, CPI do Token-2022/SPL, sterowany Clock), w obu reżimach
(`--features test-periods` i produkcyjnym).

Sekwencje z §6 promptu: pokryte istniejącymi testami (grupy A–H, GW, F-01) lub nowymi
(R6-01, R6-02, I-03). Nie znaleziono sekwencji przenoszącej środki ze skarbca bez księgi.

---

## 2. Ustalenia

### [HIGH] R6-01 — `cap < xnt_debt_index` → `MathOverflow` blokuje `settle_expired` / `claim` / `claim_genesis_window` (principal uwięziony)

- **Severity:** High (liveness; brak kradzieży). Na immutable mainnecie z ustającym fundingiem — trwała utrata dostępu do principalu.
- **Affected code:** `instructions/lifecycle.rs:92-125` (`cap_index_at`, zwracał `ck.index` bez odniesienia do `debt`); skutek w `state/mod.rs:199-201` (`settle_position_at`: `cap_index.checked_sub(debt_index)`) i `state/mod.rs:265-267` (`accrued_to_cap`).
- **Root cause:** checkpoint doby K jest **finalizowany** przy jej domknięciu (`close_day` / roll w `stake`-`claim`-`settle`). Później `redistribute_to_live` (orphan po `settle_expired`, przepadek po `unstake_early` — fix R5 M-01) **podnosi indeks puli bez doby**, a checkpoint K zostaje. Pozycja otwarta po tej redystrybucji ma `debt = indeks > ckpt(K).index`. Jeśli jej cap (ostatni funding ≤ `end_epoch`) wskazuje na K — czyli operator **nie zafundował ani jednej doby** od jej wejścia do `end_epoch` — `checked_sub` zwraca `Overflow` → `AnlError::MathOverflow` (6022).
- **Attack prerequisites:** brak napastnika w klasycznym sensie. Warunki: (1) dowolna redystrybucja po domknięciu doby (permissionless `settle_expired` dojrzałej pozycji wystarczy — nie trzeba `unstake_early`), (2) stake po niej, (3) brak `fund_xnt` do końca `end_epoch` ofiary. Napastnik może wywołać (1) i (2) sam; (3) zależy od operatora (awaria bota, utrata klucza, wind-down, koniec przychodu walidatora).
- **Attack path (dowód: `regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim`):**
  1. Doba 0 (+0.5): A stakuje Genesis 5·MIN dni, Z stakuje MIN dni (`end_epoch` Z = MIN−1).
  2. Doba MIN: `fund_xnt(20 000)` → `ckpt(MIN)`, koszyk.
  3. Doba MIN+1: `close_day(Genesis, MIN)` → indeks 100/share, `ckpt(MIN).index = 100`.
  4. `settle_expired(Z)`: cap Z = debt (brak fundingu ≤ MIN−1) → orphan 10 000 → `redistribute_to_live` → indeks **200**/share. `ckpt(MIN)` nadal 100.
  5. Ofiara V stakuje Genesis MIN dni: `debt = 200`.
  6. Brak fundingu do dojrzenia V. `settle_expired(V)` / `claim(V)` z `xnt_checkpoint = ckpt(MIN)`: `cap = 100 < debt = 200` → **`Custom(6022)`** (zaobserwowane).
  7. Dopiero `fund_xnt` w dowolnej późniejszej dobie „naprawia" stan (`write_final_index` przepisuje `ckpt(MIN)` bieżącym indeksem — zob. R6-02). Bez fundingu: stan nieodwracalny bez upgrade'u.
- **Financial impact:** 0 kradzieży. **100 % principalu + nagroda ANL** każdej dotkniętej pozycji zablokowane do następnego `fund_xnt`; trwale, jeśli funding ustał i program jest immutable. Dotyczy obu pul (Genesis przez orphan, Flexible dodatkowo przez przepadek).
- **Exploitability:** wysoka jako stan awaryjny (każdy `settle_expired` po domknięciu doby tworzy okno), niska jako atak celowany (wymaga ciszy operatora ≥ min. okres: 7 dni prod / 1 dzień testnet). Na testnecie z `test-periods` scenariusz realny przy jednodniowej przerwie bota.
- **PoC:** `regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim` — przed fixem FAIL (`Custom(6022)` na `settle_expired`), po fixie PASS w obu reżimach.
- **Recommended fix (zastosowany, `lifecycle.rs`):**
  ```rust
  // cap_index_at(): po weryfikacji PDA/łańcucha
  Ok(ck.index.max(pos.xnt_debt_index))
  ```
  Semantyka: pozycja bez zafundowanej doby w [wejście, end_epoch] nie ma należności (pending 0); cały jej udział w indeksie to orphan dla żywych (`settle_position_at` liczy orphan z `xnt_reward_index − cap` → konserwacja zachowana). Model w `state/mod.rs::test_r6_property_konserwacja_xnt_losowe_sekwencje` już zakładał `.max(debt)` — program nie.
  Opcjonalnie (obrona w głąb): to samo `max` w `settle_position_at` / `accrued_to_cap`.
- **Regression test:** jw. + istniejące H1/H2/H5 bez zmian wyników.

### [LOW] R6-02 — `fund_xnt` → `write_final_index` nadpisuje już sfinalizowany checkpoint (wypłata zależna od kolejności; inwariant 9)

- **Severity:** Low (brak drainu; konserwacja zachowana; narusza politykę orphanów z R5 M-01 i determinizm).
- **Affected code:** `instructions/fund.rs:384-406` (bezwarunkowe `write_final_index(prev, last_funded_epoch, xnt_reward_index)` po `add_to_basket`); `state/mod.rs:131-145` (`add_to_basket` nie sygnalizowało, czy domknęło dobę).
- **Root cause:** `write_final_index` miało finalizować dobę domykaną przez ten funding. Gdy doba była już domknięta wcześniej (koszyk = 0), funkcja i tak przepisuje `ckpt(last_funded_epoch).index` **bieżącym** indeksem, który zawiera redystrybucje (orphan/przepadek) wykonane po domknięciu.
- **Attack path (dowód: `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle`):** A (długa), M (`end_epoch = MIN`), Z (`end_epoch = MIN−1`) po 100 ANL. Funding 30 000 w dobie MIN, `close_day` w MIN+1 (indeks 100, `ckpt(MIN)=100`), `settle Z` → orphan 10 000 do A+M (indeks 150). Następnie:
  - kolejność **settle M → fund_xnt(MIN+1)**: M dostaje `(100−0)·100 = 10 000` (poprawnie), orphan M 5 000 → A;
  - kolejność **fund_xnt(MIN+1) → settle M**: `write_final_index` ustawia `ckpt(MIN).index = 150` → M dostaje **15 000** (5 000 z orphanu Z, który powstał **po** `end_epoch` M).
- **Financial impact:** redystrybucja między uczestnikami (M zyskuje kosztem A), ograniczona do udziału pro-rata w orphanach/przepadkach z okna [domknięcie K, następny funding]. Σ wypłat ≤ funding bez zmian.
- **Exploitability:** dojrzały uczestnik może **czekać z settle do fundingu** (bot funduje przewidywalnie) i zebrać orphan/przepadek, do którego nie ma prawa. Realne na Flexible (przepadki z `unstake_early` są częste). Ten sam mechanizm „naprawia" R6-01 — po fixie R6-01 przepisywanie nie jest już potrzebne do liveness.
- **PoC:** jw. — przed fixem FAIL (`15000 != 10000`), po fixie PASS w obu reżimach.
- **Recommended fix (zastosowany):** `add_to_basket` zwraca `Option<u64>` (domknięta doba, tylko gdy koszyk > 0 — jak `roll_day_if_needed`); `fund_xnt` woła `write_final_index` **wyłącznie** dla `Some(closed_epoch)`. Konto `prev_ckpt` bota bez zmian (koszyk > 0 ⇒ `current_day == last_funded_epoch`). Klient/bot: bez zmian.
- **Regression test:** jw.; H1a/H1b (determinizm close vs settle) nadal PASS.

### [INFO] I-01 — `initialize` jest first-come: pierwszy wołający zostaje `authority`
`Initialize` nie ogranicza `authority`. Kto pierwszy wywoła instrukcję po deployu, ustawia `authority`, `anl_mint`, `xnt_mint` (w buildzie prod `xnt_mint` przykuty do `EXPECTED_XNT_MINT`, ale `anl_mint` dowolny Token-2022 bez authority). Po inicjalizacji nieodwracalne (PDA singleton) — wymusza redeploy pod nowym Program ID. **Zalecenie proceduralne:** deploy + `initialize` + `init_*_vault` w jednej transakcji lub weryfikacja `GlobalConfig` przed `fund_rewards`; alternatywnie `require_keys_eq!(authority, HARDCODED_MULTISIG)` w buildzie mainnet.

### [INFO] I-02 — bufor `xnt_undistributed` uwalniany tylko przy domknięciu **zafundowanej** doby
Orphan/przepadek przy `total_shares == 0` trafia do bufora (M-03). Bufor wchodzi do indeksu wyłącznie w `close_day()` z koszykiem > 0. Jeśli funding ustanie na stałe, a ktoś jeszcze stakuje, bufor pozostaje w skarbcu bez właściciela. Skala: dust/orphany końcowe. Bez zmian kodu (świadomy residual wind-down); alternatywa: uwalniać bufor także w rollu bez koszyka.

### [INFO / POZYTYWNY] I-03 — to samo konto jako `prev_day_ckpt` i `xnt_checkpoint`
Test `r6_i03_ten_sam_ckpt_jako_prev_day_i_xnt_checkpoint`: roll zapisuje `ckpt(MIN−1)`, cap czyta to samo konto — A dostaje dokładnie 1/2 koszyka, B 1/2, bufor 0. Duplikat nie podwaja ani nie zeruje.

### [INFO] I-04 — manifest: `rustc` hosta zamiast toolchainu SBF
Zob. §0. Reprodukcja udana; zapisywać `cargo build-sbf --version` (platform-tools) obok.

### [INFO] I-05 — flaky test `ts_split_init_intermediate_state_guards`
Stake w kroku (e) i (f) to identyczne transakcje (ten sam blockhash); bez nowego slotu BanksClient zwraca zapamiętany wynik (e) = `SetupIncomplete` (6044). Obserwowany 1/4 uruchomień. **Naprawione** w teście: `env.advance(1)` przed (f). Nie dotyczy programu.

### [INFO] I-06 — model referencyjny `core/`: granica dustu w `xnt_pool_operation_machine` za ciasna
Proptest znalazł kontrprzykład (zapisany przez ten audyt w **nieśledzonym** `core/tests/properties.proptest-regressions`): 4 stake o łącznych shares > `PRECISION`, 1 fund → strata 4 > granica 3. Granica `events·(max_shares/P + 2)` nie liczy **floor per pozycja** przy sumowaniu `pending` (n żywych pozycji ⇒ do n jednostek). Inwariant 3 (brak inflacji) **trzyma**. Fix: `bound += liczba_żywych_pozycji` (tak jak w `state/mod.rs::test_r6_property…`: `+ live.len()`). Decyzja o commicie pliku regresji — po stronie zespołu. `core/` nie modyfikowano.

### [INFO] I-07 — komunikaty/martwe warianty
`InvalidPeriod`: „7..=3650 days" — Flexible ma teraz ≤ 365 (F-01). `DayNotClosed` martwy (znane). Kosmetyka, kody stabilne.

### Zweryfikowane i zamknięte bez ustaleń (skrót)
F-01 (R6, w paczce): limit 365 d Flexible + cooldown 3 d — testy F-01a/b/c PASS; koszt griefingu rezerwacją: 12,5× wolnej rezerwy kapitału zablokowanego ≥ 3 dni na cykl. Genesis do 3650 d rezerwuje ≤ 2× principal — kapitał realnie zablokowany (brak wyjścia), wyczerpanie pokrycia to zamierzony fail-closed. Residualy R5 (okno bez rolla → samokorekta, dowód `xnt_window_claimed ≤ xnt_accrued` przez monotoniczność łańcucha checkpointów — potwierdzony także po fixie R6-01; pauza tylko na `stake`; dust floor) — bez nowego wektora.

---

## 3. Analiza 18 instrukcji

| Instrukcja | Kto | Transfer (skarbiec →) | Klucz bezpieczeństwa | Wynik |
|---|---|---|---|---|
| `initialize` | **każdy (pierwszy)** | — | PDA singleton; walidacja mintów (Token-2022, brak mint/freeze authority, allowlista rozszerzeń, decimals 9, `EXPECTED_XNT_MINT` prod) | I-01 |
| `init_*_vault` ×4 | authority (`has_one`) | — | PDA, `token::authority = vault_authority`; CAPY: mint authority = None | OK |
| `create_pool` | authority | — | PDA per typ; enum 2 wartości | OK |
| `pause` / `resume` | authority | — | tylko `stake` bramkowany (wyjścia działają) | OK |
| `set_operator` | authority | — | operator ⊂ {fund_rewards, fund_xnt} | OK |
| `fund_rewards` | authority/operator | → reward_vault | net accounting | OK |
| `fund_capy` | **każdy** | → capy_vault | podnosi tylko `available`; front-run nieopłacalny | OK |
| `fund_xnt` | authority/operator | → xnt_vault | `epoch == clock`; łańcuch checkpointów (`next`), owner/PDA; **write_final_index bezwarunkowy** | **R6-02** |
| `close_day` | **każdy** | — | `current_day == epoch < cur`; owner-check + PDA `day_ckpt`; idempotentne | OK |
| `stake` | każdy | owner → principal_vault | net ≥ MIN; APY immutable; rezerwacja ≤ saldo reward_vault; roll przed dodaniem shares; `SetupIncomplete`; F-01 | OK |
| `settle_expired` | **każdy** | — | `now ≥ end_ts`; roll; cap = ckpt(ostatni funding ≤ end_epoch); deterministyczne | **R6-01** |
| `claim` | owner | reward_vault → owner (ANL), xnt_vault → owner, principal_vault → owner | `close = owner`; index nieużywalny ponownie; `reward_vault ≥ anl_reward`; stan po transferach ale w tej samej tx (atomowe) | **R6-01** |
| `claim_capy` | owner | capy_vault → owner | `min(pending, vault)`; `capy_reserved` | OK |
| `claim_genesis_window` | owner | xnt_vault → owner | `now < end_ts`; `prog_epoch ≤ end_epoch`; kumulacja `xnt_window_claimed` | **R6-01** (ten sam `checked_sub`) |
| `unstake_early` | owner (Flexible) | principal_vault → owner | `now < end_ts`; cooldown; przepadek do żywych | OK |

`UncheckedAccount` (wszystkie uzasadnione): `vault_authority` (seeds+bump, tylko signer CPI), `xnt_checkpoint` / `prev_day_ckpt` / `genesis_prev_ckpt` / `flexible_prev_ckpt` / `day_ckpt` (owner == program_id, PDA z pól konta, wersja/epoka/pool_type). Placeholder Program ID w `Option` → owner ≠ program → `CheckpointMismatch`. Brak `remaining_accounts`, brak arbitrary CPI (tylko Token/Token-2022), brak hooków (rozszerzenia zabronione), brak reentrancy.

---

## 4. Matematyka

- **ANL:** `reward = floor(net·apy_bps·period_s / (10⁴·365·86400))`, u128, max `net·2000·3650·86400 ≈ 1.2e31` ≪ u128. Rezerwacja przy `stake`, zwolnienie przy `claim`/`unstake_early`; `reward_vault ≥ anl_reward_reserved` inwariantnie (skarbiec maleje tylko o zarezerwowane). ✅
- **XNT (Wariant A):** `Δindex = floor(part·1e12 / S)`, `pending = floor(shares·(cap−debt)/1e12)`; wszystkie dzielenia w dół ⇒ Σ roszczeń ≤ Σ fundingu (property-test `test_r6_property_konserwacja…` 40 seedów × 120 kroków; `core/` I-06 potwierdza brak inflacji). Strata < S/1e12 + n_pozycji jednostek na zdarzenie (S ≤ 1e17 ⇒ < 1e-4 XNT). Underflow `cap − debt` — **R6-01** (naprawiony). ✅ po fixie
- **CAPY:** `ent = min(anl_reward·available/remaining_anl, available)`; `available = vault − reserved`; `reserved ≤ vault` inwariantnie; `remaining_anl` saturujące. ✅
- **Epoki:** `end_epoch = epoch_of(end_ts) − 1` (pełne doby); okno `prog_epoch ≤ end_epoch` ⇒ `xnt_window_claimed ≤ xnt_accrued` dzięki monotoniczności indeksów wzdłuż łańcucha checkpointów (zapisy do `ckpt(K)` kończą się przed utworzeniem `ckpt(K′>K)`; po fixie R6-02 również nie ma nadpisań wstecz). ✅

---

## 5. Werdykt końcowy

**1. CAN REWARD POOL BE DRAINED? — NO** (dla kodu `91418cc` + poprawki R6-01/R6-02).
Nie istnieje ścieżka przeniesienia ANL/XNT/CAPY ze skarbca bez wpisu w księdze: reward_vault chroniony inwariantem `saldo ≥ anl_reward_reserved` (stake fail-closed, claim tylko zarezerwowane), xnt_vault — konserwacją indeksu (floor wszędzie, cap z checkpointu ≤ end_epoch, orphan do żywych), capy_vault — `reserved ≤ vault`. Podstawienia kont wykluczają PDA + `token::mint/authority/token_program` + owner-checki. Wielokrotne użycie pozycji wyklucza `close = owner` i monotoniczny `position_index`. To „NO" **nie jest dowodem formalnym** — opiera się na przeglądzie, testach integracyjnych (50) i property-testach modelu.

**2. WORST-CASE ATTACK.** R6-01: dowolny uczestnik po każdym domknięciu doby wywołuje `settle_expired` dojrzałej pozycji (permissionless), po czym stakuje; jeśli operator nie zafunduje ani jednej doby do końca okresu, pozycja jest nie do zamknięcia. Bez kradzieży, ale na immutable mainnecie przy wind-down = trwała utrata principalu przez ofiary. **Naprawione.** Drugi w kolejności: R6-02 — dojrzały uczestnik opóźnia `settle` do fundingu i zbiera orphan/przepadek po swoim `end_epoch` kosztem żywych. **Naprawione.**

**3. MAXIMUM LOSS.** Kradzież: **0**. Liveness (przed fixem): 100 % principalu + ANL pozycji otwartych w oknie [redystrybucja, następny funding] przy trwałym braku fundingu. Redystrybucja (przed fixem R6-02): ≤ pro-rata udział w orphanach/przepadkach jednego okna między domknięciem doby a fundingiem.

**4. TOP 10 ATTACK PATHS (jako napastnik):**
1. Podstawienie skarbca/mintu/odbiorcy w `claim` → PDA/mint/authority → odbite (A1, A2). ✗
2. Podwójny `claim`/`unstake` w jednej i w dwóch tx → `close = owner`, index monotoniczny → odbite (D1, G1, G2). ✗
3. Cudzy/wcześniejszy/inna-pula checkpoint w `settle` → łańcuch `next`, pool_type, PDA → odbite (F2). ✗
4. Stake tuż przed `close_day`, by złapać dobę → roll w `stake` domyka dobę **przed** dodaniem shares → 0. ✗
5. Griefing rezerwacją Flexible 3650 d + darmowe wyjście → F-01 (365 d + cooldown): koszt 12,5× wolnej rezerwy na 3 dni. ✗
6. Wyścig `close_day` vs `settle` → H1a/H1b deterministyczne. ✗
7. `fund_capy` przed własnym `claim` → płacę CAPY, odzyskuję ułamek. ✗
8. **Opóźnić `settle` do `fund_xnt`, by wziąć orphan po `end_epoch`** → **R6-02, działało** (15 000 vs 10 000). ✔ → naprawione.
9. **Doprowadzić ofiarę do stanu `cap < debt`** (settle po domknięciu + jej stake) → **R6-01, blokada** przy ciszy operatora. ✔ → naprawione.
10. Front-run `initialize` po deployu → zostaję authority (I-01) → procedura deployu. (⚠ tylko okno deployu)

**5. INVARIANTS 1–10:**
| # | Inwariant | Status |
|---|---|---|
| 1 | Principal ≤ wpłata netto | PASS (A3, `amount = net`) |
| 2 | Rewards ≤ formuła | PASS (ANL immutable; XNT cap; CAPY pro-rata) |
| 3 | Σ wypłat ≤ środki | PASS (rezerwacja; property; `vault ≥` checki) |
| 4 | Brak double-claim | PASS (D1, G1–G3, GW3) |
| 5 | Nikt nie claimuje cudzych | PASS (A2, seeds owner) |
| 6 | Brak podmiany skarbca/mintu/odbiorcy | PASS (A1, B2, F2, I-03) |
| 7 | Brak transferu bez księgi | PASS |
| 8 | Zmiana konfiguracji bez retro-rewards | PASS (APY immutable; `set_operator`/`pause` neutralne; `fund_capy` podnosi tylko przyszłe entitlementy) |
| 9 | Niezależność od kolejności (wypłaty i liveness) | **FAIL na 91418cc** (R6-01, R6-02) → **PASS po fixach** (H1, R6-01, R6-02) |
| 10 | Saldo skarbców ≥ księga (dust ≥ 0) | PASS (property-test; dust < S/1e12 + n na zdarzenie); I-06 = granica w modelu, nie inflacja |

**6. CRITICAL ASSUMPTIONS (poza kodem):**
- **Upgrade authority** programu może wszystko (podmiana kodu ⇒ drain). Post-freeze: multisig/immutable (plan §5 CHANGES-AFTER-ROUND5).
- **Authority** nie ma instrukcji wypłaty — ale `initialize` first-come (I-01) i ceremonia CAPY (mint 20M → wypalenie) muszą być wykonane atomowo/weryfikowalnie.
- **Operator** może tylko wpłacać; kompromitacja = brak/nadmiar fundingu. Liveness `claim` po R6-01 **nie zależy** już od fundingu; przed fixem zależała.
- **Zegar** Solana/X1 (`Clock::get`) — granice dób i okien; drift zegara przenosi się na `epoch_of`.
- **Build**: `test-periods` nigdy na mainnet (strażnik CI `production_constants_guard`, `compile_error!`); sha `.so` w manifeście + reprodukcja (udana).
- **Runtime X1 (SVM 1.18)**: testy wykonane na solana-program-test 1.18.26 in-process, nie na SBF/X1 — semantyka CU/stack niepotwierdzona tym audytem (Box wszędzie; wcześniejsze dowody on-chain w `docs/audits/2026-07-20_onchain-sbf-evidence.md`).
- **Klient** musi poprawnie wyliczać `xnt_checkpoint` (ostatni funding ≤ target) i `prev_day_ckpt` (gdy tx domyka dobę); błędny → `CheckpointRequired/Mismatch`, retry — brak wektora, ale zależność UX.
- Token ANL/CAPY: fixed supply (mint authority None) — wymuszone on-chain; XNT = wrapped native.

**7. TESTS TO ADD (poza dodanymi R6-01/R6-02/I-03):**
- Property (integracyjny, losowy): sekwencje {stake, fund, close_day, settle, unstake_early, claim, window} na prawdziwych tx z asercją po każdym kroku: Σ sald użytkowników XNT + xnt_vault == Σ fundingu; każda pozycja **zawsze** claimowalna po `end_ts` bez fundingu (regresja R6-01 w ogólności).
- Fuzz `cap_index_at`: dla losowych łańcuchów checkpointów i `target` — zwrócony ckpt to max epoka ≤ target; każdy inny podany → `CheckpointMismatch`.
- `claim_genesis_window` w stanie `cap < debt` (po fixie: `NothingToClaim`, nie `MathOverflow`).
- Flexible: `unstake_early` → natychmiastowy stake ofiary → cisza → claim (wariant Flexible R6-01).
- `write_final_index` nigdy nie obniża/podnosi checkpointu z koszykiem 0 (asercja w teście po każdym `fund_xnt`: `ckpt(last).index` niezmienione, jeśli doba była domknięta).
- Model `core/`: poprawić granicę dustu (I-06) i zacommitować `proptest-regressions`.
- On-chain (testnet): pomiar CU dla `claim` z rollem + dwoma checkpointami; `stake` pierwszego w dobie.

**8. FINAL VERDICT: 8 / 10** (kod `91418cc` + poprawki R6-01/R6-02 z tego raportu; sam `91418cc`: 7 / 10).
Drain: nie znaleziono ścieżki, inwarianty 1–8 i 10 potwierdzone testami i własnościami; inwariant 9 był złamany w dwu miejscach (jeden High liveness) i jest naprawiony z regresjami w obu reżimach. **NOT PROVEN SAFE** pozostają: brak dowodu formalnego konserwacji XNT na poziomie instrukcji (tylko model + 50 testów), zachowanie na realnym SBF/X1 (CU) dla ścieżek z rollem, oraz procedury poza kodem (upgrade authority, `initialize`, ceremonia CAPY).

---

## 6. Zmiany wykonane w drzewie roboczym (do przeglądu zespołu)

| Plik | Zmiana |
|---|---|
| `programs/anl_staking/src/instructions/lifecycle.rs` | R6-01: `cap_index_at` → `Ok(ck.index.max(pos.xnt_debt_index))` |
| `programs/anl_staking/src/state/mod.rs` | R6-02: `add_to_basket` → `Result<Option<u64>>` (domknięta doba tylko gdy koszyk > 0) |
| `programs/anl_staking/src/instructions/fund.rs` | R6-02: `write_final_index` tylko dla `Some(closed_epoch)` |
| `programs/anl_staking/tests/integration.rs` | + `r6_prefix`, `regresja_r6_01_…`, `regresja_r6_02_…` (+ `r6_02_scenario`), `r6_i03_…`; I-05 `advance(1)` |
| `docs/audits/README.md` | wiersz tego raportu |
| `core/tests/properties.proptest-regressions` | **nieśledzony**, wygenerowany przez proptest (I-06) — decyzja zespołu |

**Wyniki po zmianach:** integracja **50/50** (`--features test-periods`, 3 uruchomienia) i **50/50** (prod); `anl_staking --lib` 11/11 (oba); `anl-math` 24+10 (oba); `cargo fmt --check` OK; `cargo clippy --workspace --all-targets -D warnings` OK (oba warianty); `core/` 34/34 lib, `properties` 1/2 (I-06, niezależne od zmian — pada także na czystym HEAD po wgraniu seedu).
Kod programu **nie był zmieniany poza dwoma poprawkami**. Binarka `855ca6a4…` (testnet) została zbudowana z `91418cc`, czyli **bez poprawek — zawiera podatności R6-01/R6-02**; przed deployem konieczny rebuild + nowy manifest.
