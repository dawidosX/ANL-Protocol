# AUDIT BRIEF — RUNDA 5 (re-audyt zmian po rundzie 4)

**Repo:** `dawidosX/ANL-Protocol` · **HEAD do audytu:** `87f358b` (main)
**Data briefu:** 2026-09-04 · **Poprzednia runda:** `6ec2139` (3 niezależne raporty: Kimi, B, C)
**Załączniki:** `docs/CHANGES-AFTER-ROUND4.md`, `release-manifest-testnet.txt`, raporty rundy 4

## 0. Po co ta runda

Runda 4 wykazała (raporty B i C, High) wyścig i stale-day w finalizacji XNT
(`claim`/`settle_expired` nie rolowały doby; `end_epoch` obejmował niepełną
dobę). Naprawa zmienia **serce księgowości XNT** — raport B wprost zażądał
re-audytu tej logiki przed freeze. Prosimy o **zawężony** przegląd: czy
poprawki są szczelne, czy nie otworzyły nowych wektorów, oraz o werdykt
gotowości do audit-freeze.

## 1. Co się zmieniło (diff `6ec2139..87f358b`, tylko `programs/`)

| Plik | Zmiana | Adresuje |
|---|---|---|
| `state/mod.rs` | nowy helper `roll_day_and_write_checkpoint` (roll wg zegara + finalizacja checkpointu; owner/PDA/wersja/epoka fail-closed) | H-01/M-01 (B, C) |
| `lifecycle.rs` | `settle_expired` i `claim` wołają helper PRZED capem; `SettleExpired` += `global_config`, `prev_day_ckpt`(mut); `Claim` += `prev_day_ckpt`(mut) | H-01/M-01 |
| `stake.rs` | `end_epoch = epoch_of(end_ts) - 1` (tylko pełne doby); refaktor na helper; bramka `SetupIncomplete` (capy_mint ≠ default) | H-01 (B), M-01 (B) |
| `initialize.rs` | `init_capy_vault`: `mint_authority == None`; `initialize`: `anl_mint.decimals == 9` | M-02 (C)/L-03 (B), L-02 (B) |
| `fund.rs` | `close_day`: owner-check checkpointu | L-02 (C) |
| `errors.rs` | `SetupIncomplete` (na końcu enumu; kody bez zmian) | — |
| `tests/integration.rs` | naprawa dryfu po Wariancie A/M-03; setup CAPY w harnessie; grupa H (determinizm, stale day, CAPY e2e); `xnt_credit` dla obu buildów; gw3 dedup blockhash | pokrycie |

Świadomie **nie** zmienione: `claim_genesis_window` bez rolla (kumulacja
`xnt_window_claimed` samokorygująca — prosimy o opinię, czy to wystarcza);
dust indeksu (floor); semantyka pauzy; `DayNotClosed` martwy wariant.

## 2. Inwariant, który ma być teraz prawdziwy (do obalenia)

> Dla każdej dojrzałej pozycji wypłata XNT = `(index_final(end_epoch) − debt) × shares / PRECISION`,
> gdzie `end_epoch` = ostatnia w PEŁNI przesiedziana doba — **niezależnie od
> kolejności transakcji** (`close_day` / `settle_expired` / `claim` / cudzy
> `stake`) i od tego, czy ktokolwiek domknął dobę przed rozliczeniem.
> Udział wygasłej pozycji w dobie `end_epoch+1` trafia do żywych w 100%
> (koszyk lub orphan → undistributed), bez wycieku i bez podwójnego liczenia.

## 3. Dowody

- **Testy:** 43/43 integracyjne w OBU reżimach CI (test-periods, prod), 7/7
  jednostkowe (w tym `test_r4_kolejnosc_settle_vs_close_identyczna_wyplata`,
  `test_r4_stale_current_day_roll_oddaje_zafundowana_dobe`), clippy
  `--all-targets -D warnings` czysto, fmt czysto. rustc 1.89.0.
- **Deploy testnet:** program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`,
  slot 185654236, sha256 `8e0403b33c4df2bd7fb8d96c96bd1c345714ba2858e2ffd7e09ff46785771f93`
  (manifest w repo).
- **Weryfikacja na produkcji (2026-09-04):** claim dojrzałej (`Co9TTS…`),
  claim okna Genesis (`2jSRW3…`, Status Ok), split-claim CAPY (pending →
  claim_capy). Audyt naliczeń wszystkich 129 pozycji / 6 portfeli skryptem
  niezależnym od kontraktu (`scripts/audyt-naliczen.js`): 0 rozbieżności.
- **Stan on-chain:** CAPY mint authority wypalona (`spl-token authorize …
  mint --disable`, sig `2gGMsu…`); ANL: authority None, decimals 9; wszystkie
  PDA zgodne z seedami.

## 4. Pytania atakowe (prosimy o werdykt do każdego)

1. **Roll w claim/settle:** czy istnieje sekwencja, w której `roll_day_and_write_checkpoint` domyka dobę, ale checkpoint tej doby NIE istnieje (koszyk > 0 bez `fund_xnt`)? Czy `CheckpointRequired` jest wtedy fail-closed bez blokady liveness?
2. **To samo konto dwa razy:** `prev_day_ckpt == xnt_checkpoint` (roll domyka epokę E, cap czyta E). Zapis przed odczytem, borrowy sekwencyjne — czy jest ścieżka, w której cap czyta indeks BAZOWY zamiast finalnego?
3. **Placeholder Anchor:** konta opcjonalne 17/18 przekazane jako `PROGRAM_ID`. Czy podanie prawdziwego, ale CUDZEGO checkpointu jako `prev_day_ckpt` (innej puli / innej epoki / nie-PDA) jest odrzucane we wszystkich kombinacjach?
4. **`end_epoch` nowa formuła vs stare pozycje:** pozycje sprzed deployu mają `end_epoch` wg `epoch_of(end_ts−1)`. Czy mieszanina starych i nowych pozycji w jednej puli narusza inwariant z §2 dla którejkolwiek z nich?
5. **Wyścig w drugą stronę:** `close_day(E)` wykonany po wygaśnięciu A, przed jej settle. Orphan = `(index − cap) × shares` → `xnt_undistributed`. Czy jest scenariusz, w którym orphan jest zaniżony/zawyżony (np. dwa fundingi w E, częściowy claim okna w E)?
6. **Okna Genesis bez rolla:** `claim_genesis_window` czyta checkpoint ≤ `prog_epoch` bez rolla. Czy przy stale `current_day` może wypłacić z indeksu BAZOWEGO i czy finalny claim zawsze wyrównuje (brak underflow `accrued − window_claimed` przy NOWEJ formule `end_epoch`)?
7. **Bramka `SetupIncomplete`:** czy da się zastakować przed `init_capy_vault` inną ścieżką (np. `capy_mint` ustawiony, vault niezainicjalizowany)? Czy `init_capy_vault` można wykonać dwukrotnie / podmienić mint po fakcie?
8. **Mint authority CAPY:** check tylko w `init_capy_vault`. Czy istnieje ścieżka zmiany `capy_mint` w `GlobalConfig` po inicjalizacji, omijająca check?
9. **`fund_capy` permissionless + skończona podaż:** po wypaleniu authority podaż jest stała. Czy permissionless `fund_capy` z cudzych CAPY może zaburzyć `capy_reserved`/`available` na niekorzyść wcześniejszych claimów (front-running rezerwacji)?
10. **Konserwacja przy `unstake_early` bez rolla:** forfeit liczony przy stale indeksie (udział zostaje w koszyku). Czy suma `wypłaty + koszyk + undistributed + index×shares` jest zachowana we wszystkich kolejnościach z `close_day`?
11. **Griefing settle (R4-01 Kimiego):** po zmianach wymuszony `settle_expired` przy otwartym koszyku daje tę samą kwotę co po `close_day`. Prosimy o potwierdzenie lub kontrprzykład.
12. **Compute budget:** roll + zapis checkpointu + settle w jednej instrukcji. Czy `claim` mieści się w limicie CU przy najgorszym przypadku (roll + checkpoint + 3 transfery + CAPY)?
13. **Klient:** reguła doboru `xnt_checkpoint` = ostatni zafundowany ≤ `end_epoch`; `prev_day_ckpt` gdy `basket>0 && current_day≠epoka_zegara`. Czy jest stan, w którym poprawny klient nie jest w stanie zbudować przechodzącej transakcji (liveness)?
14. **Build/provenance:** lockfile v3 (platform-tools 1.18), IDL generowany osobno (`--no-idl`, anchor-syn 0.30.1 vs proc-macro2 ≥1.0.96). Czy proces budowy jest deterministyczny i czy sha256 w manifeście odpowiada HEAD?
15. **Dependabot:** 13 podatności (3 high) na main — prosimy o ocenę, czy któraś dotyczy kodu on-chain (nie dev-deps).

## 5. Znane, świadome i do decyzji

- Testnet: pozycja `3sva…#11` ma APY 2000 z buildu sprzed 1.09 (prod okna) — artefakt, nie bug.
- RPC testnetu X1 trzyma ~2 dni historii — historia na stronie wymaga własnego indeksera (poza zakresem audytu).
- Mainnet checklist: mint 20M CAPY → wypalenie authority w ceremonii; strażnik `Cargo.lock v3` w CI; wyłączenie bypassu status-checków przed freeze.

## 6. Prośba o format odpowiedzi

Jak w rundzie 4: werdykt per pytanie (Czyste / Uwaga / Finding z wagą), plus
jednozdaniowa konkluzja: **gotowe do audit-freeze — TAK / NIE (co blokuje)**.
