# AUDIT BRIEF — RUNDA 7 (wąska re-weryfikacja zmian po rundzie 6)

**Repo:** `dawidosX/ANL-Protocol` · **Kod on-chain:** `src_tree b463b069…` (commit `814e16b`, identyczne w HEAD)
**Provenance:** `code_tree 7e04b873a4de2e195d26b3e53ae7a5c972246db0` · `math_tree 6fb61151f3e10b0a5d68249a941721500b34a5b3`
**Binarka testnet:** sha256 `332553d4236f3c75949980aeba5d9fcec1a08fafed69d9dba2adf4857cbd8b37`, slot **185887287**
(program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`; zgodność dumpu on-chain z binarką: 0 bajtów różnicy)
**Data briefu:** 2026-09-05 · **Poprzednia runda:** 6 (raport adwersarialny Claude Code + raporty C i Kimi)
**Załączniki:** `docs/CHANGES-AFTER-ROUND7.md`, `docs/CHANGES-AFTER-ROUND6.md`,
`docs/audits/2026-09-05_audyt-r6-adwersarialny_claude-fable5.md`, `docs/TEST-LOG.txt` (+`.sha256`),
`release-manifest-testnet.txt`, `deny.toml`, `.cargo/audit.toml`

## 0. Po co ta runda

Runda 6 dała cztery zmiany w logice kontraktu: dwie w księgowości XNT (R6-01 High liveness, R6-02 Low)
i dwie w kontroli dostępu / tokenomice (pin `initialize` — HIGH raportu C; twardy limit 200M — MEDIUM
raportu C). Prosimy o **zawężony** przegląd: czy te cztery poprawki są szczelne i nie otworzyły nowych
wektorów, oraz o potwierdzenie freeze na **tree-hash** (nie na commit).

## 1. Co się zmieniło w kodzie (diff `91418cc..814e16b`, tylko `programs/` i `crates/`)

| Plik | Zmiana | Adresuje |
|---|---|---|
| `lifecycle.rs` | `cap_index_at`: `Ok(ck.index.max(pos.xnt_debt_index))` | R6-01 |
| `state/mod.rs` | `add_to_basket` → `Result<Option<u64>>` (domknięta doba tylko gdy koszyk > 0) | R6-02 |
| `fund.rs` | `write_final_index` wyłącznie dla `Some(closed_epoch)` z `add_to_basket` | R6-02 |
| `constants.rs`, `initialize.rs` | `EXPECTED_INIT_AUTHORITY`; `require_keys_eq!` w buildach `network-*` | I-01 / HIGH (C) |
| `stake.rs` | `total_anl_paid + new_reserved ≤ ANL_REWARD_POOL` | MEDIUM (C) |
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

- Testy: **52/52** integracyjne w OBU reżimach (test-periods, prod), lib 11/11, `anl-math` 24+10, `core` 34+2 (oba
  reżimy, proptest z odtworzeniem kontrprzykładu I-06), clippy `--all-targets -D warnings` ×2, fmt, `cargo audit`
  0 podatności — `docs/TEST-LOG.txt` powiązany z HEAD `9152b54`, `code_tree 7e04b873…`.
- Nowe testy: `regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim` (przed fixem `Custom(6022)`),
  `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle` (przed fixem 15 000 ≠ 10 000),
  `r6_i03_ten_sam_ckpt_jako_prev_day_i_xnt_checkpoint`, `atak_r6_limit_200m_anl_niezaleznie_od_salda_skarbca`,
  `regresja_r7_n_dni_dokladnie_n_koszykow`, `test_r6_property_konserwacja_xnt_losowe_sekwencje` (40 × 120).
- Deploy testnet: slot 185887287, sha `332553d4…` — dump on-chain == binarka; `audyt-naliczen.js` po deployu:
  134 pozycje, 1 znana flaga (`3sva…#11` APY, artefakt).
- Reprodukowalność: `cargo build-sbf` (platform-tools v1.41) na czystym `git archive` daje identyczny sha
  (potwierdzone w R6 dla `855ca6a4…`; rebuild R7 na HEAD → `332553d4…`).

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

## 5. Znane, świadome i do decyzji

- `borsh 0.9.3` (RUSTSEC-2023-0033, unsound przy ZST) jest w grafie SBF przez `solana-program`; nasz kod
  deserializuje przez Anchor (borsh 0.10.4) i nie ma ZST w kontach — prosimy o potwierdzenie.
- `scripts/` w `.gitignore` (nowe skrypty cicho pomijane) — do usunięcia wpisu.
- WP: cooldown 3 d i ≤ 365 d Flexible, limit 200M, pin `initialize`, SLA bota (settle codziennie) — do naniesienia.
- Upgrade stosu Solana/Anchor przed immutable mainnet (opróżnia listy wyjątków supply-chain).

## 6. Prośba o format odpowiedzi

Werdykt per pytanie (a)–(e) (Czyste / Uwaga / Finding z wagą) i jednozdaniowa konkluzja:
**gotowe do audit-freeze na `code_tree 7e04b873…` — TAK / NIE (co blokuje)**.
