# Zmiany po rundzie 4 audytu (baza: commit `6ec2139` / `478d71c`)

**Data:** 2026-09-03 · **Zakres zmian:** 6 plików w `programs/anl_staking/src/`
**Weryfikacja przed publikacją:** `cargo check` (pełny, oryginalny Cargo.lock),
`cargo test --lib` 7/7, `cargo clippy --lib -- -D warnings` czysto,
`rustfmt --check` czysto — wszystko na rustc/cargo **1.89.0**.

Runda 4 = trzy niezależne raporty (Kimi / raport B [EN] / raport C [PL]).
Wspólny rdzeń ustaleń B i C potwierdzony w źródłach linia po linii.

---

## H-01 (B i C, High) + M-01 (C, Medium) — wyścig i stale `current_day` w settlemencie XNT

**Problem (potwierdzony):** `claim` i `settle_expired` liczyły cap od leniwego
`pool.current_day` i — jako jedyne ścieżki dotykające settlementu — NIE wołały
`roll_day_if_needed` (obecnego w `stake`). Skutki:

1. przy stale liczniku (funding w dobie E, cisza, zegar w dobie E+n) pozycja
   traciła **w pełni zapracowaną, historyczną dobę E** — zamrożenie trwałe
   (`settled = true`);
2. wypłata ostatniej doby zależała od kolejności permissionless transakcji
   (`close_day` przed/po `settle`) — obie strony wyścigu sterowalne przez
   osoby trzecie;
3. zaostrzenie znalezione przy weryfikacji: zaniżone zamrożenie mogło być
   NIŻSZE niż suma pobrana wcześniej `claim_genesis_window` → `checked_sub`
   w `claim` rewertuje → **trwały lockout principala** dojrzałej pozycji.

**Fix (jedna spójna zmiana):**

- Nowy wspólny helper `state::roll_day_and_write_checkpoint` — "przekręć dobę
  wg ZEGARA + sfinalizuj checkpoint domkniętej doby" (walidacja owner/PDA/
  wersja/epoka/pool_type, fail-closed). `stake` zrefaktorowany na helper
  (identyczna semantyka, jedna implementacja zamiast trzech kopii).
- `settle_expired` i `claim` wołają helper PRZED liczeniem capu — cap nigdy
  nie jest liczony względem stale stanu.
- `pos.end_epoch` liczony jako **wyłącznie pełne doby**:
  `epoch_of(end_ts) - 1` zamiast `epoch_of(end_ts - 1)`. Pozycja kończąca się
  w środku doby K nie jest uprawniona do koszyka K (reguła WP „tylko pełne
  doby"), a mechanizm orphan w `settle_position_at` gwarantuje, że udział
  doby K wraca do żywych **niezależnie od kolejności** transakcji.
- Konta: `SettleExpired` += `global_config` (read-only) i `prev_day_ckpt`
  (mut, Option); `Claim` += `prev_day_ckpt` (mut, Option). Konto wymagane
  TYLKO, gdy transakcja faktycznie domyka dobę.

**Wynik:** wypłata dojrzałej pozycji jest deterministyczna — zawsze pełne doby
do `end_epoch`, w obu kolejnościach `close_day`/`settle`/`claim`. Wektor
R4-01 Kimiego (griefing settle przy otwartym koszyku) znika w konstrukcji:
wymuszony settle daje tę samą kwotę co settle po `close_day`.

**Testy:** `test_r4_kolejnosc_settle_vs_close_identyczna_wyplata` (obie
sekwencje z identycznego stanu → identyczne wypłaty A i B + asercja
konserwacji sumy XNT), `test_r4_stale_current_day_roll_oddaje_zafundowana_dobe`
(funding doba 5, zegar doba 10 → roll oddaje pełny koszyk doby 5).

## M-01 (B, Medium) — stake przed inicjalizacją CAPY

**Problem:** `stake` przyjmował depozyty, zanim istniała infrastruktura CAPY
wymagana przez dojrzały `claim` → kapitał zależny od dokończenia setupu przez
authority (lockout przy utracie klucza).

**Fix:** bramka w `stake`: `require!(capy_mint != Pubkey::default(),
SetupIncomplete)` — `capy_mint` ustawiany wyłącznie w `init_capy_vault`,
więc stake jest niemożliwy, dopóki ścieżka wyjścia nie jest wykonywalna.
Nowy wariant błędu `SetupIncomplete` dopisany NA KOŃCU enumu (kody
istniejących błędów bez zmian).

## M-02 (C) / L-03 (B) — mint authority CAPY

**Problem:** walidacja mintu CAPY nie wymagała `mint_authority == None`
(w przeciwieństwie do ANL) — „skończona pula 20M CAPY" nie była inwariantem
on-chain; dodruk + permissionless `fund_capy` rozwadniałby entitlement.

**Fix:** `init_capy_vault` odrzuca mint z aktywną mint authority
(`MintHasMintAuthority`) — symetrycznie do checku ANL w `initialize`.

## L-02 (B) — decimals ANL

**Fix:** `initialize` wymaga `anl_mint.decimals == 9` (stałe bazowe
MIN_STAKE / ANL_REWARD_POOL / mianownik CAPY zakładają 9 miejsc); dotąd
check istniał tylko dla CAPY.

## L-02 (C) — checkpoint w `close_day` bez owner-checku

**Fix:** jawny `require_keys_eq!(*info.owner, *ctx.program_id, …)` przed
deserializacją — spójnie z `settlement_cap_index` i helperem.

## Drobne

- `claim`: `checked_sub(...).unwrap_or(0)` → `saturating_sub` (clippy 1.89,
  semantyka identyczna).
- `AnlError::DayNotClosed` pozostaje (martwy wariant, stabilność kodów błędów).

## Świadomie NIE zmienione w tej iteracji (do decyzji / rundy 5)

- **L-01 (C) — semantyka pauzy** (`pause` blokuje tylko `stake`; wypłaty i
  funding działają): pozostawione jako świadomy design „wyjście zawsze
  działa" — do udokumentowania w WP zamiast zmiany kodu. Martwy wariant
  `PoolStatus::Paused` — do decyzji.
- **L-01 (B) — dust indeksu** (floor, nieodwzorowane resztki w vaultcie):
  kierunek zaokrągleń wyłącznie w dół, bez wektora ataku; carry-forward
  resztek to zmiana księgowa do rozważenia osobno, z własnymi testami.
- **`claim_genesis_window` bez rolla:** kumulacja `xnt_window_claimed` jest
  samokorygująca (niedopłata okna wyrównuje się po `close_day` / w finalnym
  claim, po fixie H-01 zawsze); roll w oknach = zmiana zestawu kont — do
  rozważenia w rundzie 5.
- **I-01 (B) — provenance dowodów:** świeży bieg `audit-evidence.sh` na
  finalnym HEAD po commicie tych zmian (checklista deployowa, jak dotąd).

## Wpływ na klienty (WYMAGANE zmiany poza kontraktem)

1. **Frontend + bot:** `settle_expired` wymaga teraz konta `global_config`;
   `settle_expired` i `claim` przyjmują opcjonalne `prev_day_ckpt` — należy
   je przekazać (checkpoint `current_day` puli), gdy
   `pool.current_day != epoka_zegara && pool.current_day_basket > 0`.
   Najprościej: przekazywać zawsze, gdy warunek spełniony; w pozostałych
   przypadkach None.
2. **IDL:** przebudować i podmienić po `anchor build`.
3. **Istniejące pozycje testnetowe** mają `end_epoch` wg starej formuły
   (włącznie z niepełną dobą) — po wdrożeniu ich ostatnia doba rozliczy się
   deterministycznie (roll wymusza domknięcie); nowe pozycje dostają regułę
   „tylko pełne doby". Na testnecie bez znaczenia ekonomicznego.

## Status po zmianach

Zamknięte w kodzie: H-01 (B+C), M-01 (C), M-01 (B), M-02 (C)/L-03 (B),
L-02 (B), L-02 (C). Wymagana **runda 5** (re-audyt zmienionej logiki
finalizacji XNT) przed jakimkolwiek freeze — zgodnie z żądaniem raportu B.
