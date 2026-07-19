## Dowód wykonania testów — NIE ufaj temu opisowi

## Runda #3 — status potwierdzonych ustaleń kodowych

Niezależny audyt #3 (ocena 6,8/10) potwierdził spójność modelu epok i BRAK ścieżki,
w której funding epoki > end_epoch zwiększa wypłatę. Zgłoszone realne ustalenia
kodowe — NAPRAWIONE w tej wersji:

| ID | Ustalenie | Status | Dowód |
|----|-----------|--------|-------|
| M-01 | brak `version` check obu pul w `FundXnt` | ✅ naprawione | fund.rs — `constraint = *_pool.version == ACCOUNT_VERSION` |
| H-01 | brak twardej blokady `test-periods` na mainnet | ✅ naprawione | Cargo.toml `network-mainnet`/`network-testnet` + `compile_error!` w lib.rs; build `network-mainnet test-periods` NIE kompiluje się |
| L-01 | brak jawnego owner-check dla checkpointów | ✅ naprawione | `require_keys_eq!(*info.owner, *program_id)` w `settlement_cap_index` i `roll_checkpoint` |
| M-02 | sprzeczność README `end_ts` vs `end_epoch` | ✅ naprawione | README.md:19 / README.pl.md:19 wg patcha audytora; akapit o kolejności bota poprawiony |
| — | rozbieżność liczb testów (23→24, 3→4) | ✅ naprawione | README zsynchronizowany z TEST-LOG.txt |

Ustalenia FAZY DEPLOY (nie wady kodu, świadomie otwarte do wdrożenia):
M-03 placeholder Program ID, verifiable build, produkcyjne testy w CI, property/fuzz,
pełna macierz negatywna checkpointów i Token-2022. Blokery "brak reprodukcji testów /
brak commita / cargo not found" wynikają z braku Rust/Cargo w środowisku audytora —
adresowane przez `docs/TEST-LOG.txt` + wymóg odtworzenia na commicie po `git push`.

Residual (rozwodnienie indeksu przez nierozliczone pozycje): audytor potwierdza
LOW security / MEDIUM ekonomiczne, wypłacalność zachowana, NIE wymaga auto-zdejmowania
shares. Mitygacja: permissionless settle + publiczny cranker + metryka
`expired_unsettled_shares` (do wdrożenia operacyjnego).


`docs/TEST-LOG.txt` zawiera surowy wynik + wersje toolchainu. **Log nie jest
dowodem sam w sobie** — audytor powinien odtworzyć go na konkretnym commicie:

```
git rev-parse HEAD
cargo test -p anl-math && cargo test -p anl-math --features test-periods
(cd core && cargo test)
cargo test -p anl_staking --features test-periods --test integration
cargo test -p anl_staking --test integration
```

Oczekiwane: anl-math 24, core 34, integration 4 (oba warianty). Kluczowy test
kryterium bezpieczeństwa #1: `ts_audit_funding_after_end_epoch_not_counted`.

# Odpowiedź na wstępny audyt bezpieczeństwa (18.07.2026)

Werdykt audytu przyjęty: **NIE wdrażamy immutable do czasu zamknięcia punktów.**
Status per punkt:

| # | Ustalenie | Status | Rozwiązanie |
|---|---|---|---|
| 1 | XNT naliczane po `end_ts` przy inline-settle | ✅ **NAPRAWIONE (model epok)** | deterministyczne epoki dzienne + `XntCheckpoint` (snapshot indeksu po każdej epoce fundingu, łańcuch `next_funded_epoch`); settlement liczy XNT z checkpointu ostatniej epoki ≤ `end_epoch` pozycji — funding późniejszej epoki NIE MOŻE zwiększyć wypłaty, niezależnie od kolejności settle/fund/claim; test exploita `ts_audit_funding_after_end_epoch_not_counted` (atak podstawionym checkpointem odrzucany, równoważność ścieżek, inwariant wypłacalności) |
| 2 | funding wymaga `authority` | ✅ naprawione (przed audytem) | rola `operator` (hot key tylko-do-wpłat, `set_operator` z multisig); authority NIE kasujemy |
| 3 | placeholder Program ID | 🟡 świadome (pre-deploy) | pozycja checklisty: keypair → `declare_id!` + `Anchor.toml` → rebuild → weryfikacja PDA |
| 4 | brak kontroli rozszerzeń Token-2022 | ✅ naprawione | `initialize` odrzuca mint z freeze authority oraz KAŻDYM rozszerzeniem poza {MetadataPointer, TokenMetadata}; PermanentDelegate/TransferHook/TransferFee/inne → `ForbiddenMintExtension` |
| 5 | `test-periods` chronione logiem | ✅ wzmocnione | twardy test `production_constants_guard` (build domyślny MUSI mieć 31/91/7); CI odpala oba warianty; procedura: osobny Program ID dla buildów testowych |
| 6 | brak weryfikowalnej binarki | 🟡 checklist | `anchor build --verifiable` + `anchor verify` przed `--final`; potwierdzić semantykę loadera na X1 |
| 7 | brak kontroli wersji kont | ✅ naprawione | `version == ACCOUNT_VERSION` we WSZYSTKICH instrukcjach (GlobalConfig, PoolConfig, UserPosition) |
| 8 | za słabe constraints vaultów | ✅ naprawione | każdy vault w każdej instrukcji: `token::mint` + `token::authority = vault_authority` + `token::token_program` |
| 9 | pauza = kontrola administracyjna | 🟡 komunikacja | sekcja governance w WP (multisig, próg, zakres pauzy, ścieżki wyjścia zawsze otwarte) |


## Semantyka rozliczenia: `end_epoch`, nie `end_ts` (specyfikacja, nie luka)

**Zamierzona semantyka = model B (epoka).** Naliczanie XNT rozlicza się w jednostce
**epoki dziennej**, nie pojedynczej sekundy. Formalnie:

- `end_epoch = epoch_of(end_ts − 1)` (stake.rs) — epoka zawierająca ostatnią
  aktywną sekundę pozycji;
- `settlement_cap_index` bierze indeks z **ostatniego fundingu w epoce ≤ end_epoch**;
- pozycja uczestniczy w dystrybucji XNT **każdej epoki, w której choć sekunda jej
  okresu jest aktywna**, włącznie z całą epoką końca — także funding tej samej
  epoki wykonany po `end_ts` jest wliczany (należny, bo epoka końca należy do pozycji);
- funding **jakiejkolwiek epoki > end_epoch** nigdy nie zwiększa wypłaty (gwarancja
  bezpieczeństwa #1, wymuszona matematyką checkpointów).

**To NIE jest rozjazd dokumentacja↔kod.** WP §8.1, README i ten dokument opisują
model B jednoznacznie. Nagroda **ANL** (stopa ciągła) jest liczona do dokładnego
`end_ts` i nie podlega tej zasadzie — dotyczy ona wyłącznie strumienia XNT, którego
źródło (przychód walidatora) jest z natury wielkością epokową.

**Świadome następstwo (rezydualne, akceptowane):** pozycja, której `end_ts` mija w
środku epoki, „widzi" pełną epokę końca. Jest to sprawiedliwe (przychód epoki nie
jest dzielony na sekundy) i deterministyczne. Alternatywa (proporcja do sekundy)
wymagałaby księgowania sub-epokowego, które ponownie otwierałoby wektor z audytu #1.

## Model epok XNT — semantyka (do review #2)
- Epoka = 1 dzień, kotwica `genesis_start_ts` (granice zbieżne z oknami, 02:00 UTC).
- `fund_xnt(amount, epoch)` wymaga `epoch == epoch_of(now)`; brak przypisań wstecz.
- Checkpoint puli×epoki: indeks PO wszystkich fundingach epoki + `next_funded_epoch`.
- Pozycja: `end_epoch = epoch_of(end_ts − 1)` — nalicza PEŁNY dzień końcowy
  (funding tej samej epoki po `end_ts` wlicza się; kolejnych epok — nigdy).
- Dowód „ostatni funding ≤ end_epoch": `ckpt.epoch ≤ end_epoch ∧
  (ckpt.next == NONE ∨ ckpt.next > end_epoch)` + PDA; podstawienie odrzucane.
- Zero fundingu ≤ end_epoch → wypłata XNT = 0 bez konta checkpointu
  (`first_funded_epoch` w puli).
- Rezydualne (świadome): pozycja nierozliczona po terminie do czasu settle
  trzyma shares → rozwadnia indeks kolejnych epok; nadwyżka zostaje w vaultcie
  (inwariant wypłacalności trzyma). Mitygacja: permissionless `settle_expired`
  + bot dzienny; do dyskusji w #2 ewentualne koszyki auto-zdejmowania.

## Dodatkowo wdrożone w tej rundzie
- `initialize`: wymóg `mint_authority == None` (fixed supply) dla ANL;
  twarda kotwica `EXPECTED_XNT_MINT` (wrapped native) w buildzie produkcyjnym.
- `set_operator`: odrzuca `Pubkey::default()` i wartość bieżącą.
- Kontrola wersji kont w `create_pool`, `set_pause`, pulach `fund_xnt`.
- Testy integracyjne w OBU wariantach buildu; wariant produkcyjny używa
  minta natywnego pod właściwym adresem i fundingu przez wrap+sync_native.

## Twarde zasady (bez zmian)
- ZERO `solana program set-upgrade-authority --final` do końca pełnego review
- ZERO kasowania kluczy (`GlobalConfig.authority` wymagany operacyjnie)
- mainnet wyłącznie po: review #2 → testnet kilka epok → fuzzing → verifiable build

## Świadomie odłożone (kolejna iteracja)
dwustopniowe przekazanie operatora · wersjonowanie `UserProfile` ·
macierz negatywnych testów rozszerzeń Token-2022 · property/fuzz (proptest,
cargo-fuzz 24h+) · cargo audit/deny w CI · verifiable build w CI

---

# Runda #3 (Grok 6,8/10 + niezależna weryfikacja) — status poprawek

Werdykt rdzenia: **model checkpointów epok logicznie spójny; brak ścieżki, w której
funding epoki > end_epoch zwiększa wypłatę** (potwierdzone statycznie wszystkie
wektory: późniejsza epoka, inna pula, starszy checkpoint, dziura między fundingami,
sfabrykowane konto, manipulacja next_funded_epoch). Brak rug-pull. Inwarianty
wypłacalności zachowane.

## Realne ustalenia kodowe — NAPRAWIONE w tej rundzie
| ID | Ustalenie | Status | Dowód |
|----|-----------|--------|-------|
| M-01 | Brak `version` check pól `genesis_pool`/`flexible_pool` w `FundXnt` | ✅ naprawione | `fund.rs` — dodano `constraint = *.version == ACCOUNT_VERSION` |
| H-01 | Brak twardej blokady `test-periods` na mainnet | ✅ naprawione | `Cargo.toml` features `network-mainnet`/`network-testnet` + `compile_error!` w `lib.rs`; build `network-mainnet+test-periods` **NIE kompiluje się** |
| L-01 | Brak jawnego owner check dla `UncheckedAccount` checkpointów | ✅ naprawione | `require_keys_eq!(*info.owner, *program_id)` w `settlement_cap_index` i `roll_checkpoint` |

## Nieaktualne w bieżącym kodzie (audytor pracował na starszym ZIP-ie)
- **M-02** (sprzeczność README end_ts/end_epoch): już poprawione — README:19 PL/EN
  mówi „ANL do end_ts; XNT epokowo do end_epoch". Linie 44-46 PL: dopisano wprost,
  że poprawność wypłaty NIE zależy od kolejności settle/fund.
- **Liczby testów**: README pokazuje aktualne 24/24 i 4/4 (nie 23/3).

## Faza deploy (nie blokery kodu — do wykonania przy wdrożeniu)
M-03 Program ID (`anchor keys sync`), verifiable build + `anchor verify`,
reprodukcja testów na konkretnym commicie (audytor nie miał Rust/cargo w środowisku).

## Rekomendacje (nie blokery) — do rozważenia przed immutable
Property/fuzz testy księgowości XNT (24h+), pełna macierz negatywna Token-2022
i checkpointów, wersjonowanie `UserProfile`, rozszerzenie CI (fmt/clippy -D warnings/
audit/deny/prod integration), metryka `expired_unsettled_shares` + publiczny cranker
dla rezydualnego rozwodnienia indeksu (LOW security / MEDIUM ekonomiczne — świadome).


---

# Weryfikacja po rundzie #3 — hardening i dowody (patrz CHANGES-AFTER-AUDIT3.md)

- `cargo fmt --all --check` — czysty; `cargo clippy --workspace --all-targets
  -- -D warnings` — czysty w OBU wariantach feature (log: TEST-LOG.txt).
- Dowody negatywne w CI (`release-guards`): mainnet+test-periods oraz
  mainnet+testnet NIE kompilują się (oczekiwane sukcesy testów negatywnych).
- Polityka feature'ów sieci: bez trzeciej blokady "brak sieci" — świadomie
  (zepsułaby wszystkie domyślne pipeline'y); gwarancja mainnetu wynika z:
  network-mainnet ⟹ ¬test-periods ⟹ kontrola EXPECTED_XNT_MINT aktywna.
  Release wyłącznie skryptami z jawnym zestawem feature'ów + manifest
  (HEAD, features, sha256 binarki).
- Pełny, reprodukowalny bieg dowodowy na commicie: `scripts/audit-evidence.sh`
  (git rev-parse HEAD + fmt + clippy + wszystkie testy + dowody negatywne
  + audit/deny) — zapisuje TEST-LOG powiązany z HEAD.
