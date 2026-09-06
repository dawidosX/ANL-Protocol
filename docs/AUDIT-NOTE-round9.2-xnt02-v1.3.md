# AUDIT NOTE — runda 9.2 (v1.3): XNT-02 droga B+ i hazard horyzontu

**Data:** 2026-09-06 · **Branch:** `fix/v1.3-xnt02-horizon` (baza: `fix/v1.2-r9.1` @ `59732b0`)
**Status:** kod NIE zmergowany, NIE zbudowany, NIE wdrozony — raport dla Recenzenta przed merge.

## 1. Zakres v1.3

| # | Zrodlo | Temat | Commit |
|---|---|---|---|
| 1 | @Olxbug (hazard wdrozeniowy) | test-periods `XNT_FUNDING_HORIZON_SECS` 9 d → 100 lat (testnet genesis 2026-08-18 starszy niz 9 d ⇒ upgrade v1.2 zabilby `fund_xnt` na stale); prod 3 lata bez zmian; straznik kompilacji `test >= prod`; testy horyzontu na wstrzyknietym czasie | `a1eaf59` |
| 2 | Pawel (XNT-02, Medium) | redystrybucja (przepadek `unstake_early`, orphan `settle_expired`/`claim`) domyka BIEZACA dobe z checkpointem TEJ doby (tworzonym na zadanie, dowiazanym do lancucha); indeks zmienia sie wylacznie przy domknieciu doby | ten branch |

## 2. XNT-02 — przyczyna i zasada v1.3

Do v1.2 `redistribute_to_live` podnosil `xnt_reward_index` bez checkpointu. Pozycja B zywa w chwili przepadku A, rozliczana z checkpointu OSTATNIEGO FUNDINGU ≤ end_epoch, nie widziala wzrostu; nadwyzka stawala sie orphanem i trafiala do poznego C (PoC: B 5 zamiast 10, C 6 zamiast 1).

**Zasada v1.3 (droga B+):** `xnt_reward_index` zmienia sie TYLKO z checkpointem BIEZACEJ doby (domkniecie doby albo redystrybucja z checkpointem epoki zegara), nigdy wstecz:

1. `roll_day_if_needed(cur)` ustawia `current_day = cur` takze przy pustym koszyku (po rollu zawsze `current_day == epoka zegara`).
2. `redistribute_with_checkpoint(amount)`: `total_shares > 0` ⇒ `xnt_reward_index += amount / total_shares` (koszyk fundingu doby i legacy NIETKNIETE — domyka je koniec doby wg finalnych shares); `total_shares == 0` ⇒ `xnt_protocol_revenue` (bez checkpointu, v1.2).
3. Handler (settle_expired / claim / unstake_early): jesli indeks wzrosl ⇒ `write_current_day_checkpoint`: PDA `[xnt_ckpt, pool_type, cur_epoch]` tworzony NA ZADANIE (create_account / allocate+assign z podpisem PDA, platnik = signer), `index = nowy`, `next = NO_EPOCH`, `ckpt(last_funded_epoch).next = cur`, `last_funded_epoch = cur`. Jesli wezel dzisiejszy istnieje (funding lub wczesniejsza redystrybucja tej doby) — tylko `index` (idempotencja).
4. `last_funded_epoch` = **ostatnia doba z checkpointem** (funding LUB redystrybucja) = ogon lancucha `next_funded_epoch`.

Dlaczego „na zadanie”, a nie Anchor `init_if_needed`: seeds z epoka zegara wymagalyby argumentu instrukcji (zmiana danych ABI), a konto powstawaloby przy KAZDYM claim/settle (czynsz, smieciowe wezly). Tu konto powstaje tylko, gdy redystrybucja faktycznie zaszla, i zawsze jest wezlem lancucha.

Dlaczego nie „koszyk ostatniej zafundowanej doby” (pierwotna droga B): atrybucja wstecz nadpisywala historyczny checkpoint i pozycja dojrzala-nierozliczona lapala przepadek po swoim koncu (XNT-01 wracal: 400 vs 200; R6-02 dawal 15 000). Z checkpointem biezacej doby: pozycja z end_epoch < dzis rozlicza sie z wczesniejszego wezla (nie widzi wzrostu), pozycja zywa przez te dobe widzi go w capie.

## 3. Odczyty `last_funded_epoch` / `first_funded_epoch` (audyt spojnosci)

| Miejsce | Uzycie | Wplyw v1.3 |
|---|---|---|
| `fund.rs::roll_checkpoint` | `== epoch` (wezel dzisiejszy istnieje → walidacja, bez re-init); `epoch > last` (EpochMismatch); `prev = ckpt(last)` + `next = epoch`; `first_funded` przy NO_EPOCH | spojne: wezel z redystrybucji tej doby przechodzi galezia `== epoch` (`init_if_needed` znajduje konto z dyskryminatorem); bot podaje `prev = ckpt(pool.last_funded_epoch)` PER PULA (fund-xnt.js juz tak robi) |
| `fund.rs` koniec `fund_xnt` | `last_funded_epoch = epoch` (obie pule) | bez zmian; komentarz R6-02 uzupelniony (doba domknieta redystrybucja ma koszyk 0 ⇒ `add_to_basket` zwraca None ⇒ jej checkpoint NIE jest nadpisywany) |
| `lifecycle.rs::cap_index_at` | `last == NO_EPOCH ∨ first > target` ⇒ cap = debt; inaczej ckpt z `epoch ≤ target ∧ (next == NO_EPOCH ∨ next > target)` | spojne: kazdy istniejacy checkpoint jest wezlem lancucha (funding lub redystrybucja), wiec dla kazdego `target` kwalifikuje sie DOKLADNIE jeden wezel |
| `state.rs::write_current_day_checkpoint` (nowe) | `== current_day` ⇒ tylko index; inaczej `current_day > last` (EpochMismatch), link `prev`, `last = current_day` | nowy pisarz |
| `create_pool.rs` | init NO_EPOCH | bez zmian |
| `Env` (testy) / `fund-xnt.js` (bot) / www | ogon lancucha czytany z puli (offset 61) | Env: `chain_tail_ckpt` per pula (zamiast lokalnego licznika) |

## 4. Zmiany ABI (konta) — WYMAGANE po stronie klientow

| Instrukcja | Nowe konta (na koncu) | Semantyka istniejacych | Klient |
|---|---|---|---|
| `settle_expired` | `cur_day_ckpt` (mut, PDA epoki zegara), `system_program`; `cranker` staje sie `mut` (platnik czynszu) | `prev_day_ckpt` = ZAWSZE `ckpt(last_funded_epoch)` gdy pula miala funding (roll + dowiazanie); None = placeholder program ID | **bot settle** (poza repo) — dolozyc 2 konta, oznaczyc cranker writable, podawac prev zawsze |
| `claim` | `cur_day_ckpt` (mut), `system_program` | `prev_day_ckpt` jak wyzej | **www `claimPosition`** — zrobione (konta 18-20) |
| `unstake_early` | `prev_ckpt` (Option, mut), `cur_day_ckpt` (mut), `system_program` | — | **www `unstakeEarly`** — zrobione (konta 9-11; pakowanie `close_day` bez zmian) |
| `fund_xnt` | brak | `genesis_prev_ckpt`/`flexible_prev_ckpt` = `ckpt(last_funded_epoch)` PER PULA (moga sie roznic po redystrybucji w jednej puli) | `fund-xnt.js` juz czyta per pula — bez zmian |
| `claim_genesis_window`, `close_day`, `stake` | brak | bez zmian | — |

Layout kont (`PoolConfig`, `XntCheckpoint`, `UserPosition`) bez zmian. Kody bledow bez nowych wariantow (`CheckpointRequired`, `CheckpointMismatch`, `EpochMismatch`, `DayNotClosed`).

## 5. Konsekwencje semantyczne do swiadomej akceptacji

- **Redystrybucja srod-doby (R10 pyt. 4, po korekcie):** przepadek/orphan podnosi indeks TYLKO o swoja kwote (dla shares obecnych teraz) i zapisuje checkpoint biezacej doby; koszyk fundingu doby zostaje NIETKNIETY i domyka go koniec doby wg FINALNYCH shares. Pierwotna wersja B+ domykala caly koszyk — `unstake_early` malej pozycji srod-doby (dozwolony, bo straznik `DayNotClosed` odbija tylko koszyk POPRZEDNIEJ doby) zamykal funding doby na obecnych i odcinal pozniejszych stakerow tej doby (test: C dostawal 0 zamiast 500). Checkpoint doby moze byc nadpisany kilka razy w ciagu doby (zawsze ogon lancucha, zawsze rosnaco); doby minione nigdy.
- **Cel DoS `create_account`:** zasilenie PDA lamportami obslugiwane sciezka transfer + allocate + assign (jak Anchor init).
- **Czynsz:** platnik = signer instrukcji (cranker/owner), ok. 0,0013 SOL za wezel, tylko przy faktycznej redystrybucji > 0 z shares > 0.
- **Straznik `DayNotClosed`** w `unstake_early` zostaje jako obrona w glab (redundantny po B+).
- R6-01: warunek `cap < debt` jest odtad strukturalnie niemozliwy (checkpoint zawsze rowna sie indeksowi po domknieciu); `max(ckpt, debt)` zostaje.

## 6. Testy (oba rezimy)

| Test | Wynik |
|---|---|
| `regresja_v13_xnt02_przepadek_z_checkpointem_b_10_pozny_c_1` (odwrocony PoC Pawla) | live_B 10, **paid_B 10, paid_C 1**, funded 11; `ckpt(4)` utworzony, `ckpt(0).next = 4` |
| `regresja_v12_xnt01_wyplata_niezalezna_od_kolejnosci_unstake_vs_settle` | exit-first == settle-first == 200 |
| `regresja_r6_02_wyplata_niezalezna_od_kolejnosci_fund_vs_settle` | 10 000 w obu kolejnosciach |
| `regresja_h1a/h1b/h5`, `regresja_r6_01`, `ts_early_exit_forfeits_and_redistributes` | pierwotne oczekiwania; R6-01 podaje `ckpt(MIN+1)` (ogon po redystrybucji) |
| `regresja_v13_lancuch_checkpointow_funding_redystrybucja_funding` | `ckpt(E-1).next=E`, `ckpt(E).next=E+1`, `ckpt(E+1).next=NO_EPOCH`; cap dla end_epoch E-1/E/E+1 = 3000/4000/7000; konserwacja 21 000 |
| `regresja_v13_idempotencja_checkpointu_redystrybucja_i_funding_tej_samej_doby` | obie kolejnosci: jeden wezel, `ckpt(0).next = d` raz, B = 23 000 |
| property (state) R6 + R9: indeks staly poza close/redystrybucja, redystrybucja domyka biezaca dobe (koszyk 0, `current_day == epoch`), suma roszczen ≤ funded z granica dustu | 40 seedow × 120/150 |
| property (harness) `property_r9_skarbiec…` | 3 × 80 krokow, skarbiec ≥ roszczenia |
| `test_v13_xnt02_przepadek_domyka_biezaca_dobe_z_checkpointem` (model, w tym analog XNT-01), `test_v13_konserwacja_przepadek_ostatniego_do_revenue_i_wspolne_domkniecie` | PASS |

Bramki: lib 17 / 19 / 18; integracja **62/62** test-periods i **62/62** prod; core 34+2 / 30+2; anl-math 24 / 24; clippy `-D warnings` ×3; fmt; `cargo audit` 0; `Cargo.lock` nietkniety.

## 7. R10 (trojka) i korekta pyt. 4

Werdykty R10 na `27eac40`: DRAINABLE: NO ×3; rdzen B+ (tworzenie konta / lancuch / idempotencja) CZYSTE ×3. Spor o pyt. 4 (domkniecie srod-doby): Kimi/C = Uwaga, GPT = Medium.

Weryfikacja testem `regresja_v13_intraday_nie_odcina_pozniejszych` (oba rezimy) na `27eac40`: `unstake_early` B w dobie D z otwartym koszykiem fundingu 1000 PRZECHODZI (straznik `DayNotClosed` odbija tylko koszyk poprzedniej doby), koszyk → 0 na obecnych, C wchodzacy pozniej w dobie D dostalby 0 z doby D ⇒ scenariusz GPT potwierdzony ⇒ **Medium**, fix: redystrybucja nie domyka koszyka fundingu (`redistribute_with_checkpoint`). Po fixie: koszyk zostaje 1000, przepadek B (100) do A z `ckpt(D)`, po `close_day(D)` A = 1600, C = 500; koszyk poprzedniej doby nadal ⇒ `DayNotClosed`. Wszystkie pozostale testy (XNT-02 odwrocony, XNT-01, R6-02, lancuch, idempotencja, property) bez zmian wynikow.

Bramki po korekcie: lib 17/19/18; integracja **63/63** ×2; core 34+2 / 30+2; anl-math 24/24; clippy ×3; fmt; cargo audit (0 podatnosci; przejsciowy timeout rejestru przy sprawdzaniu yanked); `Cargo.lock` nietkniety.
