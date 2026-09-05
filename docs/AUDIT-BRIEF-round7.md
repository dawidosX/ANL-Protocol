# AUDIT BRIEF — RUNDA 7 (wąska re-weryfikacja zmian po rundzie 6)

**Repo:** `dawidosX/ANL-Protocol` · **Kod on-chain (R7.2):** `src_tree 4c2256398137bb417a1b769316137852d14ec4d5` (`HEAD:programs/anl_staking/src`)
**Provenance:** `code_tree 7dbf3de415685767654ad8e068f6034e27e51e53` · `math_tree 6fb61151f3e10b0a5d68249a941721500b34a5b3`
**Binarka testnet:** sha256 `87b431d43280e4eccfca71725bbfafda2f1fbd2fb2d95a94cd2715fd4ae530a3`, slot **185899744**
(program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`; zgodność dumpu on-chain z binarką: 0 bajtów różnicy)
**Data briefu:** 2026-09-05 (aktualizacja R7.2) · **Poprzednia runda:** 6 (raport adwersarialny Claude Code + raporty C i Kimi)
**Załączniki:** `docs/CHANGES-AFTER-ROUND7.md`, `docs/CHANGES-AFTER-ROUND6.md`,
`docs/audits/2026-09-05_audyt-r6-adwersarialny_claude-fable5.md`, `docs/TEST-LOG.txt` (+`.sha256`),
`release-manifest-testnet.txt`, `deny.toml`, `.cargo/audit.toml`

## 0. Po co ta runda

Runda 6 dała cztery zmiany w logice kontraktu: dwie w księgowości XNT (R6-01 High liveness, R6-02 Low)
i dwie w kontroli dostępu / tokenomice (pin `initialize` — HIGH raportu C; twardy limit 200M — MEDIUM
raportu C). R7.1 dodała obronę w głąb dla R6-01 na poziomie modelu puli (`max(cap, debt)` także w
`settle_position_at` / `accrued_to_cap`, po OK Recenzenta) i tekst komunikatu `InvalidPeriod`. R7.2 domknęła
ścieżkę release: osobny Program ID dla buildu bez feature sieciowego, fail-closed provenance w `pack-audit.sh`,
bramka podaży CAPY = 20M na mainnecie (`InvalidCapySupply`, nowy kod 6046 na końcu enumu). Prosimy o
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
| `lib.rs` (R7.2) | `declare_id` bez feature sieciowego = `ChG81WAp…` (testowe); strażniki `program_id_pinned_{testnet,mainnet}`, `program_id_is_test_id` | release path |
| `constants.rs`, `initialize.rs`, `errors.rs` (R7.2) | `CAPY_TOTAL_SUPPLY = 20M×10⁹`; `init_capy_vault` pod `network-mainnet` wymaga `supply == CAPY_TOTAL_SUPPLY` → `InvalidCapySupply` (6046, koniec enumu) | tokenomika WP on-chain |
| `crates/anl-math` | bez zmian (`math_tree` jak w R6) | — |

Nie zmieniono: układu kont żadnej instrukcji ani istniejących kodów błędów (jeden nowy wariant na końcu enumu), klienta.
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

- Testy: **52/52** integracyjne w OBU reżimach (test-periods, prod), lib 13/13 (15/15 pod `network-mainnet`), `anl-math` 24+10, `core` 34+2 (oba
  reżimy, proptest z odtworzeniem kontrprzykładu I-06), clippy `--all-targets -D warnings` ×2, fmt, `cargo audit`
  0 podatności, strażniki pinu pod `network-testnet,test-periods` i `network-mainnet` — `docs/TEST-LOG.txt`
  powiązany z HEAD `544b12a`, `code_tree 7dbf3de4…`, `src_tree 4c225639…`.
- Nowe testy: `regresja_r6_01_cap_ponizej_debt_nie_blokuje_claim` (przed fixem `Custom(6022)`),
  `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle` (przed fixem 15 000 ≠ 10 000),
  `r6_i03_ten_sam_ckpt_jako_prev_day_i_xnt_checkpoint`, `atak_r6_limit_200m_anl_niezaleznie_od_salda_skarbca`,
  `regresja_r7_n_dni_dokladnie_n_koszykow`, `test_r6_property_konserwacja_xnt_losowe_sekwencje` (40 × 120),
  `test_r7_cap_ponizej_debt_daje_zero_bez_overflow` (model), `init_authority_pinned_{testnet,mainnet}`,
  `program_id_pinned_{testnet,mainnet}`, `program_id_is_test_id`, `capy_supply_constant`.
- Deploy testnet R7.2: slot 185899744, sha `87b431d4…` — dump on-chain == binarka (0 bajtów różnicy, weryfikacja
  dwustronna); `audyt-naliczen.js` po deployu: 134 pozycje, 1 znana flaga (`3sva…#11` APY, artefakt).
- Reprodukowalność: `cargo build-sbf` (platform-tools v1.41) na czystym `git archive` daje identyczny sha
  (potwierdzone w R6 dla `855ca6a4…`; rebuild R7 → `332553d4…`, rebuild R7.1 → `4b550417…`, R7.2 na HEAD → `87b431d4…`).

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

**(h) Release path (R7.2).** Build bez feature sieciowego ma inny `declare_id`, więc binarka bez pinu `initialize` i
bez bramki CAPY nie uruchomi się pod `4Cpx…`. `pack-audit.sh` odmawia pakowania, gdy `src_tree`/`code_tree`/
`math_tree`/`sha256` manifestu nie zgadzają się z HEAD i binarką. Czy to zamyka zastrzeżenia provenance z R6
(Kimi, C P-01) i czy bramka `supply == 20M` w `init_capy_vault` (jednorazowa, po wypaleniu authority) jest
wystarczającym inwariantem tokenomiki CAPY?

## 5. Znane, świadome i do decyzji

- `scripts/` w `.gitignore` — naprawione w R7.1 (ignorowany tylko `scripts/node_modules/`).
- WP: cooldown 3 d i ≤ 365 d Flexible, limit 200M, pin `initialize`, SLA bota (settle codziennie) — do naniesienia.
- Upgrade stosu Solana/Anchor przed immutable mainnet (opróżnia listy wyjątków supply-chain).

## 6. Prośba o format odpowiedzi

Werdykt per pytanie (a)–(h) (Czyste / Uwaga / Finding z wagą) i jednozdaniowa konkluzja:
**gotowe do audit-freeze na `src_tree 4c225639…` / `code_tree 7dbf3de4…` — TAK / NIE (co blokuje)**.
