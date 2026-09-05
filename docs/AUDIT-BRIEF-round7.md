# AUDIT BRIEF — RUNDA 7 (wąska re-weryfikacja zmian po rundzie 6)

**Repo:** `dawidosX/ANL-Protocol` · **Kod on-chain (R7.1):** `src_tree d95f8ba909db0ce6c96c8832ee862e7e779bd4a1` (`HEAD:programs/anl_staking/src`)
**Provenance:** `code_tree 0da5cdaa26e46d6db3a63587bffef3ebfe569fe3` · `math_tree 6fb61151f3e10b0a5d68249a941721500b34a5b3`
**Binarka testnet:** sha256 `4b55041782da0e40169e0050b680a81fc17e7df954689c8a9a350f7e4f916d1f`, slot **185892286**
(program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`; zgodność dumpu on-chain z binarką: 0 bajtów różnicy)
**Data briefu:** 2026-09-05 (aktualizacja R7.1) · **Poprzednia runda:** 6 (raport adwersarialny Claude Code + raporty C i Kimi)
**Załączniki:** `docs/CHANGES-AFTER-ROUND7.md`, `docs/CHANGES-AFTER-ROUND6.md`,
`docs/audits/2026-09-05_audyt-r6-adwersarialny_claude-fable5.md`, `docs/TEST-LOG.txt` (+`.sha256`),
`release-manifest-testnet.txt`, `deny.toml`, `.cargo/audit.toml`

## 0. Po co ta runda

Runda 6 dała cztery zmiany w logice kontraktu: dwie w księgowości XNT (R6-01 High liveness, R6-02 Low)
i dwie w kontroli dostępu / tokenomice (pin `initialize` — HIGH raportu C; twardy limit 200M — MEDIUM
raportu C). R7.1 dodała obronę w głąb dla R6-01 na poziomie modelu puli (`max(cap, debt)` także w
`settle_position_at` / `accrued_to_cap`, po OK Recenzenta) i tekst komunikatu `InvalidPeriod`. Prosimy o
**zawężony** przegląd: czy te poprawki są szczelne i nie otworzyły nowych wektorów, oraz o potwierdzenie
freeze na **tree-hash** (nie na commit).

## 1. Co się zmieniło w kodzie (diff `91418cc..HEAD`, tylko `programs/` i `crates/`)

| Plik | Zmiana | Adresuje |
|---|---|---|
| `lifecycle.rs` | `cap_index_at`: `Ok(ck.index.max(pos.xnt_debt_index))` | R6-01 |
| `state/mod.rs` | `add_to_basket` → `Result<Option<u64>>` (domknięta doba tylko gdy koszyk > 0) | R6-02 |
| `fund.rs` | `write_final_index` wyłącznie dla `Some(closed_epoch)` z `add_to_basket` | R6-02 |
| `constants.rs`, `initialize.rs` | `EXPECTED_INIT_AUTHORITY`; `require_keys_eq!` w buildach `network-*` | I-01 / HIGH (C) |
| `stake.rs` | `total_anl_paid + new_reserved ≤ ANL_REWARD_POOL` | MEDIUM (C) |
| `state/mod.rs` (R7.1) | `settle_position_at`, `accrued_to_cap`: `cap = max(cap, debt)` przed odejmowaniem (obrona w głąb; orphan względem podniesionego capu) | R6-01 |
| `errors.rs` (R7.1) | tekst `InvalidPeriod` (dopisek o limicie Flexible 365) — kody bez zmian | I-07 |
| `constants.rs` (R7.1) | moduły `#[cfg(test)]` — strażniki wartości `EXPECTED_INIT_AUTHORITY` pod `network-testnet` / `network-mainnet` (nie wchodzą do binarki) | I-01 |
| `crates/anl-math` | bez zmian (`math_tree` jak w R6) | — |

Nie zmieniono: układu kont żadnej instrukcji, kodów błędów (brak nowych wariantów), klienta.
Świadome residualy bez zmian: okna Genesis bez rolla (samokorekta), dust floor, pauza tylko na `stake`,
`DayNotClosed` martwy, legacy `end_epoch` na testnecie, orphan zależny od timingu settle (SLA bota),
M-03 (pusta pula → 100 % dla drugiej).

## 2. Inwarianty, które mają być teraz prawdziwe (do obalenia)

> **(I)** Dla każdej pozycji po `end_ts` istnieje przechodząca transakcja `claim` **niezależnie od tego, czy operator
> kiedykolwiek jeszcze wywoła `fund_xnt`** (liveness bez zależności od fundingu).
> **(II)** Wypłata XNT dojrzałej pozycji = `(max(index_final(K), debt) − debt) × shares / P`, gdzie K = ostatnia
> zafundowana epoka ≤ `end_epoch` — **niezależnie od kolejności** `close_day` / `settle_expired` / `claim` / `fund_xnt`
> / cudzych `stake`/`unstake_early`, i bez udziału w redystrybucjach wykonanych po domknięciu doby K.
> **(III)** `total_anl_paid + anl_reward_reserved ≤ 200 000 000 ANL` po każdej instrukcji, niezależnie od salda
> `reward_vault`; jednocześnie `reward_vault ≥ anl_reward_reserved`.
> **(IV)** W buildzie sieciowym `initialize` wykonuje wyłącznie `EXPECTED_INIT_AUTHORITY`.

## 3. Dowody

- Testy: **52/52** integracyjne w OBU reżimach (test-periods, prod), lib 12/12, `anl-math` 24+10, `core` 34+2 (oba
  reżimy, proptest z odtworzeniem kontrprzykładu I-06), clippy `--all-targets -D warnings` ×2, fmt, `cargo audit`
  0 podatności, strażniki pinu pod `network-testnet,test-periods` i `network-mainnet` — `docs/TEST-LOG.txt`
  powiązany z HEAD `319ad10`, `code_tree 0da5cdaa…`.
- Nowe testy: `regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim` (przed fixem `Custom(6022)`),
  `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle` (przed fixem 15 000 ≠ 10 000),
  `r6_i03_ten_sam_ckpt_jako_prev_day_i_xnt_checkpoint`, `atak_r6_limit_200m_anl_niezaleznie_od_salda_skarbca`,
  `regresja_r7_n_dni_dokladnie_n_koszykow`, `test_r6_property_konserwacja_xnt_losowe_sekwencje` (40 × 120),
  `test_r7_cap_ponizej_debt_daje_zero_bez_overflow` (model), `init_authority_pinned_{testnet,mainnet}`.
- Deploy testnet R7.1: slot 185892286, sha `4b550417…` — dump on-chain == binarka (0 bajtów różnicy, weryfikacja
  dwustronna); `audyt-naliczen.js` po deployu: 134 pozycje, 1 znana flaga (`3sva…#11` APY, artefakt).
- Reprodukowalność: `cargo build-sbf` (platform-tools v1.41) na czystym `git archive` daje identyczny sha
  (potwierdzone w R6 dla `855ca6a4…`; rebuild R7 → `332553d4…`, rebuild R7.1 na HEAD → `4b550417…`).

## 4. Pytania celowane (prosimy o werdykt do każdego: Czyste / Uwaga / Finding z wagą)

**(a) `cap = max(ckpt, debt)` a okna Genesis.** `claim_genesis_window` liczy `accrued_to_cap(shares, debt, cap)`
i kumuluje `xnt_window_claimed`; finalny `claim` płaci `xnt_accrued − xnt_window_claimed`. Czy istnieje sekwencja
(okno przy `cap = debt`, potem funding, potem finalny claim z innym checkpointem; albo legacy `end_epoch`), w której
`max` prowadzi do **nadpłaty** w oknie lub **underflow** przy finalnym odejmowaniu? Nasz argument: indeksy wzdłuż
łańcucha checkpointów są monotoniczne, `prog_epoch ≤ end_epoch`, `max` tylko podnosi cap do `debt` (pending 0),
więc `window_claimed ≤ accrued` zawsze.

**(b) `write_final_index` tylko przy domknięciu.** Trzy ścieżki finalizują checkpoint doby K: `close_day`, roll w
`stake`/`claim`/`settle_expired` (`roll_day_and_write_checkpoint`), oraz `fund_xnt` następnej doby (`add_to_basket`
→ `Some(K)`). Czy istnieje stan, w którym doba K ma koszyk > 0, żadna z tych ścieżek nie zapisze finalnego indeksu,
a `cap_index_at` odczyta **bazowy** `ckpt(K).index` (sprzed rozdzielenia koszyka)? W szczególności: pula, która w
dobie K dostała `part = 0` (druga pula pusta), a potem ma koszyk > 0 w K+1.

**(c) Pin `initialize` + ceremonia mainnet.** `EXPECTED_INIT_AUTHORITY` jest stałą kompilacji gate'owaną
`network-*`; na testnecie = klucz deployera = upgrade authority = operator (jeden hot key). Czy procedura
„stała → multisig, build mainnet, sha w manifeście, deploy + `initialize` + `init_*_vault` + `init_capy_vault`
(mint 20M → wypalenie authority)" jest kompletna i czy brak testu pinu w suite (cfg sieciowy) jest akceptowalny,
a jeśli nie — jaki dowód (np. test pod `network-testnet` z kluczem testowym) uznacie za wystarczający?

**(d) Limit 200M vs rezerwacja i CAPY.** `stake` wymaga `total_anl_paid + reserved_new ≤ ANL_REWARD_POOL`;
`claim` liczy CAPY z `remaining_anl = ANL_REWARD_POOL − total_anl_paid`. Czy `unstake_early` (zwalnia rezerwację,
nie zwiększa `paid`) i nadwyżka salda `reward_vault` ponad 200M mogą doprowadzić do stanu, w którym
`remaining_anl` i suma pozostałych zobowiązań się rozjeżdżają (np. CAPY entitlement liczony wobec puli, której
część już nigdy nie zostanie wyemitowana)? Czy `saturating_sub` w `remaining_anl` jest właściwy, gdy autorytet
wpłaci > 200M do skarbca?

**(e) Freeze na `code_tree`.** Prosimy o potwierdzenie, że porównanie `code_tree`/`math_tree`/`src_tree` z TEST-LOG,
manifestu i paczki (`pack-audit.sh` — manifest OBOK zipa) wystarcza jako dowód „ten sam kod", i że różnice
commitów wynikające wyłącznie z docs/manifestów nie są zastrzeżeniem provenance.

**(f) borsh 0.9.3 w grafie SBF (RUSTSEC-2023-0033).** `cargo +solana tree --target sbf-solana-solana` pokazuje
`borsh 0.9.3` przez `solana-program` (legacy API `borsh0_9`). Nasza ocena: nieosiągalne — advisory dotyczy
deserializacji `Vec<T>` dla ZST bez `Copy`; konta i argumenty programu nie mają ZST ani `Vec<ZST>`
(u8…u128/i64/bool/Pubkey/enum/[u8; N]); Anchor 0.30.1 deserializuje przez borsh 0.10.x. Prosimy o potwierdzenie
lub kontrprzykład (ścieżka, w której nasz kod wywołuje borsh 0.9).

**(g) Obrona w głąb w modelu (R7.1).** `settle_position_at` i `accrued_to_cap` liczą `cap = max(cap, debt)` przed
odejmowaniem; orphan liczony względem podniesionego capu (`index − max(cap, debt)`), więc cały udział pozycji od
wejścia trafia do żywych, a konserwacja jest zachowana. Alternatywą był błąd sygnalizacyjny w modelu — odrzucony
jako ta sama klasa ryzyka liveness co R6-01. Czy zgadzacie się, że podwójny `max` (w `cap_index_at` i w modelu)
nie zmienia żadnej wypłaty dla `cap ≥ debt` i nie otwiera nadpłaty dla `cap < debt`?

## 5. Znane, świadome i do decyzji

- `scripts/` w `.gitignore` — naprawione w R7.1 (ignorowany tylko `scripts/node_modules/`).
- WP: cooldown 3 d i ≤ 365 d Flexible, limit 200M, pin `initialize`, SLA bota (settle codziennie) — do naniesienia.
- Upgrade stosu Solana/Anchor przed immutable mainnet (opróżnia listy wyjątków supply-chain).

## 6. Prośba o format odpowiedzi

Werdykt per pytanie (a)–(g) (Czyste / Uwaga / Finding z wagą) i jednozdaniowa konkluzja:
**gotowe do audit-freeze na `src_tree d95f8ba9…` / `code_tree 0da5cdaa…` — TAK / NIE (co blokuje)**.
