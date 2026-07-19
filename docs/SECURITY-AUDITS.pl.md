# Historia audytów bezpieczeństwa — ANL Staking Protocol

**Status:** dokument żywy · ostatnia aktualizacja **19.07.2026**
**Zakres:** program on-chain `anl_staking` (Rust / Anchor), crate matematyczny `anl-math`, model referencyjny w `core/`, CI oraz narzędzia release/dowodowe w `scripts/`.
**Wersja angielska (główna):** [SECURITY-AUDITS.md](SECURITY-AUDITS.md)

> Ten dokument zastępuje stare, dopisywane rundami notatki `AUDIT-RESPONSE.md`. Przedstawia pełną ścieżkę audytową chronologicznie: każdą rundę, każde ustalenie, jego naprawę i dowód. Zbiorcza tabela ustaleń — [§8](#8-zbiorcza-tabela-ustaleń); zadania otwarte — [§9](#9-zadania-otwarte).

---

## 1. Metodyka

Protokół przechodzi iteracyjne audyty bezpieczeństwa wspierane przez AI, wykonywane przez niezależnych recenzentów (inne modele niż ten, który implementuje kod), każdorazowo na świeżej migawce repozytorium. Każda runda kończy się pisemnym raportem; zespół odpowiada poprawkami w kodzie, odnotowuje je tutaj i przekazuje zaktualizowaną migawkę do ponownej weryfikacji. Materiał dowodowy żyje w samym repozytorium: pipeline CI (4 joby: lint / test / release-guards / supply-chain), `scripts/audit-evidence.sh` (fmt, clippy `-D warnings`, wszystkie zestawy testów, buildy negatywne, cargo audit/deny) oraz `docs/TEST-LOG.txt`.

Konwencja wag za raportami recenzentów: **Krytyczne** (środki zagrożone w realistycznych warunkach), **Wysokie/H**, **Średnie/M**, **Niskie/L** oraz **ustalenia procesowe** (pipeline dowodowy/release, nie logika on-chain).

---

## 2. Runda #1 — audyt wstępny (GPT), 18.07.2026

Pierwszy zewnętrzny przegląd kompletnej implementacji Fazy 1+2 (wszystkie instrukcje cyklu życia, trzy skarbce, rezerwacja nagród ANL, dzienny silnik XNT). Raport zawierał **9 ustaleń**. Ocena zespołu: solidny, uczciwy audyt — każdy punkt do działania, a ustalenie #1 to prawdziwa perła.

| # | Ustalenie | Waga | Pierwotna decyzja |
|---|-----------|------|-------------------|
| 1 | Naliczanie XNT po `end_ts` zależy od dyscypliny bota: gdy dzienny funding wejdzie po końcu pozycji, inline settle liczy z zawyżonego indeksu i wypłaca XNT należne innym | **Krytyczne** | Uznane; wymagało przebudowy księgowości (§4–5) |
| 2 | `fund_xnt` wymagał podpisu `authority` — gorący klucz multisig/Ledger w codziennej, automatycznej ścieżce | Średnie | Naprawione tego samego wieczoru (rola operatora); audytowana migawka była sprzed poprawki |
| 3 | `declare_id!` to placeholder Program ID | Info | Świadomy stan pre-deploy; przeniesione do checklisty wdrożeniowej |
| 4 | Rozszerzenia Token-2022 minta ANL bez walidacji (PermanentDelegate / TransferHook / TransferFee mogłyby podważyć księgowość skarbców) | Wysokie | Uznane; naprawione (bramka rozszerzeń) |
| 5 | Zabezpieczenia builda `test-periods` niewystarczające — log ostrzegawczy to nie zabezpieczenie | Wysokie | Uznane; naprawione (twardy test-strażnik, później blokady compile-time) |
| 6 | Brak verifiable/reproducible build | Średnie | Przeniesione do checklisty wdrożeniowej |
| 7 | Brak kontroli `version` kont w instrukcjach | Średnie | Uznane; naprawione |
| 8 | Niepełne constraints kont skarbców | Średnie | Uznane; naprawione |
| 9 | Polityka pauzy niekomunikowana użytkownikom wprost | Niskie | Uznane; sekcja governance w whitepaperze |

## 3. Naprawy po rundzie #1 (18.07.2026)

* **Rola operatora (ust. 2):** `set_operator(new_operator)` wywoływane przez `authority` (multisig/Ledger); `fund_rewards`/`fund_xnt` przyjmują authority **lub** operatora. Operator to gorący klucz wyłącznie do wpłat — jego kompromitacja nie zagraża środkom użytkowników. (`instructions/fund.rs`, `state`)
* **Bramka rozszerzeń Token-2022 (ust. 4):** `initialize` rozpakowuje mint ANL przez `StateWithExtensions` i egzekwuje allowlistę — akceptowane są wyłącznie pasywne rozszerzenia metadanych (`MetadataPointer`, `TokenMetadata`); `PermanentDelegate`, `TransferHook`, `TransferFee`, każde nieznane rozszerzenie oraz ustawione freeze authority są odrzucane (`ForbiddenMintExtension`, `MintHasFreezeAuthority`). (`instructions/initialize.rs`)
* **Wersjonowanie kont (ust. 7):** każdy kontekst instrukcji egzekwuje `version == ACCOUNT_VERSION` (`InvalidAccountVersion`).
* **Pełne constraints skarbców (ust. 8):** każde konto skarbca w każdym kontekście związane jest mintem + PDA authority + programem tokenowym.
* **Strażnik stałych produkcyjnych (ust. 5, etap pierwszy):** test `production_constants_guard`, kompilowany tylko w wariancie domyślnym (produkcyjnym), asertuje okna 31/91 dni i min. okres 7 dni; CI odpala go przy każdym pushu — artefakt release, który go nie przechodzi, nie jest artefaktem produkcyjnym.
* Ustalenia 3 i 6 weszły do twardej checklisty wdrożeniowej; ustalenie 9 do whitepapera (sekcja governance/pauzy).
* Stan testów po poprawkach: anl-math 24/24 (oba warianty), core 34/34, integracja zielona. Ustalenie **#1 pozostało świadomie otwarte**, z projektem naprawy (koszyki wygaśnięć per pula×dzień) przekazanym audytorowi razem ze zaktualizowaną migawką.

---

## 4. Runda #2 — przegląd poprawionej migawki (Grok), 18–19.07.2026

Niezależny drugi przegląd repozytorium po poprawkach. **Ocena: 8,5/10.** Poprawki rundy #1 potwierdzone; ustalenie **#1 (XNT po `end_ts`)** potwierdzone jako jedyny pozostały krytyk; rekomendowane testy property-based/fuzz księgowości XNT (rekomendacja powtórzona później przez każdego recenzenta — §9). Odpowiedzią na tę rundę nie była łatka, lecz przeprojektowanie: model epok XNT poniżej.

## 5. Model epok XNT — zamknięcie krytyka #1

Księgowość dziennych koszyków została przebudowana wokół natywnej jednostki rozliczeniowej sieci X1 — **epoki**:

* **Checkpointy per pula×epoka.** Dedykowane konta PDA zapisują skumulowany indeks XNT (`acc-per-share`) na zamknięcie każdej epoki dla każdej puli.
* **`fund_xnt(amount, epoch)`.** Funding jest teraz jawnie przypisany do epoki i domyka wymagane checkpointy (`roll_checkpoint`); instrukcja przyjmuje konta checkpointów, których dotyka.
* **`end_epoch = epoch_of(end_ts − 1)`.** Pozycja nalicza XNT za **pełne epoki** do epoki końca okresu włącznie; strumień ANL nadal kończy się dokładnie na `end_ts`. Oba README opisują tę asymetrię wprost.
* **`settlement_cap_index`.** Rozliczenie (przez `settle_expired`, inline settle w `claim` czy `unstake_early`) liczy XNT z indeksu **ograniczonego checkpointem epoki końcowej pozycji**, nigdy z indeksu bieżącego. Spóźniony funding nie może więc przypisać zakończonej pozycji XNT z epok po jej końcu — gwarancję wymusza arytmetyka kontraktu, nie dostępność bota.
* **`epoch_of` zwraca `Option<u64>`** — znaczniki czasu sprzed genesis mapują się na jawny błąd `BeforeGenesis` zamiast cichego fallbacku.

W tym modelu awaria bota degraduje się łagodnie: opóźnia dystrybucję, ale nie może już jej błędnie przypisać.

---

## 6. Runda #3 — audyt szczegółowy, 19.07.2026

Pogłębiony przegląd implementacji modelu epok. **Ocena: 6,8/10** (ostrzejsza metodyka i szerszy zakres niż runda #2; ocena odzwierciedla dojrzałość procesu w równym stopniu co kod). Cztery ustalenia:

| ID | Ustalenie | Waga |
|----|-----------|------|
| **M-01** | Kontekst `FundXnt` nie egzekwował `ACCOUNT_VERSION` na dwóch kontach pul | Średnie |
| **M-02** | Dokumentacja rozjechana z modelem epok: niespójna semantyka `end_ts` vs `end_epoch`; nieaktualne liczby testów | Średnie (dokumentacja) |
| **L-01** | Konta checkpointów czytane bez jawnej kontroli właściciela-programu (ścieżki `settlement_cap_index`, `roll_checkpoint`) | Niskie |
| **H-01** | Brak wykluczenia `test-periods` na buildach mainnet w compile-time — strażnik był wyłącznie proceduralny | Wysokie |

## 7. Naprawy po rundzie #3 + dwie niezależne weryfikacje (19.07.2026)

Wszystkie cztery ustalenia naprawione i ponownie zweryfikowane przez **dwóch niezależnych recenzentów**, każdy na finalnej paczce: weryfikacja źródłowa (GPT, w repo jako `docs/audits/audit-3-verification-gpt.md`) oraz weryfikacja procesowa (Grok, *„ANL Protocol — weryfikacja zmian po audycie #3"*, w repo jako `docs/audits/audit-3-verification-grok.pdf`). Obie potwierdzają poprawki kodowe; rozjeżdżają się wyłącznie w kwestii pozostałości dokumentacyjno-procesowych — oba stanowiska są odnotowane niżej.

* **M-01 — naprawione.** `constraint = genesis_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion` oraz odpowiednik dla Flexible. Dowód: `programs/anl_staking/src/instructions/fund.rs:124-140`.¹
* **L-01 — naprawione.** `require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch)` w obu miejscach odczytu checkpointów. Dowód: `programs/anl_staking/src/instructions/lifecycle.rs:69-72` (`settlement_cap_index`) i `programs/anl_staking/src/instructions/fund.rs:196-208` (`roll_checkpoint`).¹
* **H-01 — naprawione na poziomie cfg.** Blokady `compile_error!`: `network-mainnet` + `test-periods` nie mogą współistnieć, a wybrany musi być dokładnie jeden feature sieci. Dowód: `programs/anl_staking/src/lib.rs:11-15`, `programs/anl_staking/Cargo.toml:11-18`.¹ `docs/TEST-LOG.txt` zawiera surowy dowód negatywny (`cargo check … --features network-mainnet,test-periods` → dokładny komunikat `compile_error!`). Job release-guards w CI buduje zabronioną kombinację, asertuje niezerowy kod wyjścia **i** dokładny komunikat (`.github/workflows/ci.yml:46-59`¹), a dodatkowo kompiluje oba warianty pozytywne, żeby wadliwy cfg nie zablokował wszystkich buildów.
* **M-02 — naprawione.** Oba README formułują teraz regułę jednoznacznie: strumień ANL kończy się dokładnie na `end_ts`, a XNT rozlicza się pełnymi epokami do `end_epoch = epoch_of(end_ts − 1)` (blockquote w obu językach); stare sformułowanie „oba strumienie stają na `end_ts`" zniknęło, a liczby testów w tabeli podsumowującej są zsynchronizowane (24/24 anl-math, 4/4 integracja). Dowód: `README.md:19-20,87`, `README.pl.md:19-20,89`.¹ Jedna pozostałość znaleziona przez weryfikację Groka: komentarz w sekcji build wciąż brzmi `# math (23)` (`README.md:74-80`¹) — śledzone pod V-05.
* **Brak regresji** w modelu checkpointów, powierzchni instrukcji (zmiany nazw handlerów były wyłącznie wewnętrzne; nazwy funkcji w `#[program]` bez zmian, więc discriminatory instrukcji nietknięte — porównanie finalnego IDL przed deployem nadal zalecane) ani w zmianie `epoch_of → Option<u64>`.

Kluczowe zdanie weryfikacji Groka: *problemem nie jest już logika stakingu, lecz łańcuch dowodowy od czystego commita do wdrożonej binarki.* Nowe **ustalenia procesowe** z tej weryfikacji (wszystkie naprawione tego samego dnia — lista napraw niżej i w §8):

* **M-EVIDENCE-01** — job supply-chain w CI uruchamia `cargo audit || true` i `cargo deny … || true`, więc podatność ani zabroniona zależność nie zapalają czerwonego CI (`.github/workflows/ci.yml:71-83`¹).
* **M-EVIDENCE-02** — `scripts/audit-evidence.sh` nie jest fail-closed: `set -uo pipefail` (bez `-e`), brak bramki czystego drzewa, nadpisuje śledzony `docs/TEST-LOG.txt` przed sprawdzeniem `git status` i wypisuje `GOTOWE` (kod 0) nawet po nieudanych krokach.
* `scripts/build-mainnet.sh` sprawdza czystość przez `git diff --quiet` (nie łapie zmian staged i plików untracked); `scripts/build-testnet.sh` nie sprawdza czystości wcale. Poprawna bramka: `test -z "$(git status --porcelain)"`.
* Drugi release-guard (mainnet+testnet naraz) asertuje tylko niezerowy kod wyjścia, bez konkretnego komunikatu `select exactly one network feature`.
* README (oba języki) nadal dokumentuje gołe ścieżki `anchor build` omijające skrypty release, nosi nieaktualną liczbę testów „23" (faktycznie: 24) i nie opisuje jeszcze polityki feature'ów sieci / skryptów release.
* `docs/TEST-LOG.txt` załączony do audytowanej paczki zaczynał się od realnego diffu `cargo fmt --check` — i ten diff był prawdziwy: `instructions/initialize.rs` pozostał niesformatowany, co wywalało też job lint w CI od commita `27cd983` aż do sformatowania pliku 19.07.2026. Stary skrypt dowodowy maskował dokładnie tę klasę awarii (M-EVIDENCE-02 w praktyce).

**Naprawy pipeline'u dowodowego (19.07.2026, odpowiedź tego samego dnia):** `scripts/audit-evidence.sh` przepisany na fail-closed (`set -euo pipefail`, bramka czystego drzewa przez `git status --porcelain` przed czymkolwiek, log do pliku tymczasowego poza repo, buildy negatywne asertują kod wyjścia **i** dokładny komunikat `compile_error!`, stopka wiąże bieg z `git rev-parse HEAD`, `docs/TEST-LOG.sha256` zapisuje hash logu, niezerowy kod wyjścia po dowolnym błędzie); bramki czystego drzewa (`git status --porcelain`) w obu `scripts/build-*.sh`; `|| true` usunięte z joba supply-chain w CI i zatwierdzony `deny.toml` w repo; drugi release-guard asertuje komunikat `select exactly one network feature`; README (EN+PL) przepisane na politykę skryptów release z poprawką „23" → 24.

**Pierwszy uczciwy bieg supply-chain (19.07.2026):** po zdjęciu `|| true` `cargo audit` przeskanował 606 zależności z locka i zgłosił **8 podatności + 16 ostrzeżeń informacyjnych** — wszystkie w stosie kliencko-testowym Solany 1.x, żadna nie pochodzi z tego kodu: `ed25519-dalek 1.0.1` (RUSTSEC-2022-0093), `curve25519-dalek 3.2.1` (RUSTSEC-2024-0344), `ring 0.16.20` (RUSTSEC-2025-0009), `rustls-webpki 0.101.7` (RUSTSEC-2026-0098/0099/0104), `quinn-proto 0.10.6` (RUSTSEC-2026-0037, high 8,7; RUSTSEC-2026-0185). Triaż: te crate'y to warstwa sieciowa klienta RPC/QUIC/TLS i operacje sekretne SDK; artefakt on-chain (SBF) nie wykonuje operacji kluczem prywatnym ani połączeń TLS/QUIC, więc ekspozycja dotyczy narzędzi deweloperskich i testów, nie wdrożonego programu. Środek przejściowy: **udokumentowane wyjątki z kwartalnym przeglądem** (identyczne listy w `.cargo/audit.toml` i `deny.toml`, najbliższy przegląd 2026-10-19); joby pozostają blokujące dla wszystkiego nowego. Naprawa docelowa — upgrade stosu Solana/Anchor — śledzona w §9 i bramkuje DoD mainnetu (cel: puste listy wyjątków).

**Werdykty (19.07.2026).** *Weryfikacja GPT:* testnet / zamknięty pilot **gotowy** (przy osobnym Program ID, ściśle limitowanej wartości aktywów i monitoringu); żadne otwarte ustalenie kodowe rundy #3 nie blokuje immutable — immutable mainnet staje się osiągalny po spełnieniu 9-punktowego Definition of Done (§9). *Weryfikacja Groka:* zamknięty testnet **warunkowo gotowy** po naprawie pipeline'u dowodowego; immutable mainnet **niegotowy**, dopóki łańcuch commit→binarka nie jest fail-closed. **Stanowisko zespołu (przyjęte):** wygrywa ostrzejsza interpretacja — V-01…V-05 naprawione tego samego dnia (wyżej), a mainnet pozostaje bramkowany pełnym Definition of Done.

¹ Numery linii za raportami weryfikacyjnymi z 19.07.2026; mogą dryfować z kolejnymi commitami — wiążące są symbole i ścieżki plików.

---

## 8. Zbiorcza tabela ustaleń

Waga: C = krytyczne, H = wysokie, M = średnie, L = niskie, I = info, P = procesowe. Status: ✅ naprawione i zweryfikowane, 🟡 otwarte (śledzone w §9), 📋 checklista wdrożeniowa.

| ID | Runda | Waga | Ustalenie | Status | Dowód / miejsce naprawy |
|----|-------|------|-----------|--------|--------------------------|
| R1-01 | 1 | C | Naliczanie XNT po `end_ts` zależne od bota | ✅ | Model epok (§5): checkpointy per pula×epoka, `end_epoch = epoch_of(end_ts−1)`, `settlement_cap_index`; `instructions/fund.rs`, `instructions/lifecycle.rs`, `state/mod.rs` |
| R1-02 | 1 | M | Dzienny funding wymagał klucza authority | ✅ | Rola operatora: `set_operator`; `instructions/fund.rs`, `lib.rs` |
| R1-03 | 1 | I | Placeholder Program ID | 📋 | `anchor keys sync` przy deployu; osobne ID dla buildów testnet/mainnet |
| R1-04 | 1 | H | Rozszerzenia Token-2022 minta ANL bez walidacji | ✅ | Bramka-allowlista w `instructions/initialize.rs` (`ForbiddenMintExtension`, `MintHasFreezeAuthority`) |
| R1-05 | 1 | H | Zabezpieczenia `test-periods` tylko logiem | ✅ | Test `production_constants_guard` (`crates/anl-math/src/lib.rs`) + blokady compile-time H-01 (`lib.rs:11-15`¹) |
| R1-06 | 1 | M | Brak verifiable build | 📋 | Checklista wdrożeniowa (§9) |
| R1-07 | 1 | M | Brak kontroli wersji kont | ✅ | `version == ACCOUNT_VERSION` w każdym kontekście instrukcji |
| R1-08 | 1 | M | Niepełne constraints skarbców | ✅ | Constraints mint + PDA authority + program tokenowy w każdym kontekście |
| R1-09 | 1 | L | Transparentność polityki pauzy | ✅ | Sekcja governance w whitepaperze; ścieżki wyjścia (`claim`, `unstake_early`, `settle_expired`) działają zawsze |
| R3-M-01 | 3 | M | `FundXnt` bez constraints wersji pul | ✅ | `instructions/fund.rs:124-140`¹ |
| R3-M-02 | 3 | M | Dokumentacja rozjechana: semantyka `end_ts`/`end_epoch`, stare liczby testów | ✅ | `README.md:19-20,87`, `README.pl.md:19-20,89`¹; pozostałość `# math (23)` → V-05 |
| R3-L-01 | 3 | L | Odczyt checkpointów bez kontroli właściciela | ✅ | `instructions/lifecycle.rs:69-72`, `instructions/fund.rs:196-208`¹ |
| R3-H-01 | 3 | H | Brak wykluczenia mainnet×test-periods w compile-time | ✅ | `lib.rs:11-15`, `Cargo.toml:11-18`¹; release-guards CI `.github/workflows/ci.yml:46-59`¹ |
| V-01 | 3-wer | P/M | Supply-chain CI nieblokujący (`\|\| true`) | ✅ | `\|\| true` usunięte, zatwierdzony `deny.toml` w repo; `.github/workflows/ci.yml` (job supply-chain), `deny.toml` |
| V-02 | 3-wer | P/M | `audit-evidence.sh` nie fail-closed | ✅ | Przepisany fail-closed: `set -euo`, bramka czystego drzewa, log tymczasowy, HEAD + `TEST-LOG.sha256`; `scripts/audit-evidence.sh` |
| V-03 | 3-wer | P | Niewystarczające kontrole czystego drzewa w skryptach build | ✅ | Bramka `git status --porcelain` w obu; `scripts/build-mainnet.sh`, `scripts/build-testnet.sh` |
| V-04 | 3-wer | P | Drugi release-guard nie asertuje komunikatu błędu | ✅ | Asertuje `select exactly one network feature`; `.github/workflows/ci.yml` (release-guards) |
| V-05 | 3-wer | P | README: gołe ścieżki `anchor build`, stara „23", brak polityki release | ✅ | Sekcje build przepisane na politykę skryptów release, „23"→24, feature'y sieci opisane; `README.md`, `README.pl.md` |

---

## 9. Zadania otwarte

**Pipeline dowodowy i release — ✅ ukończone 19.07.2026** (V-01…V-05; warunek zamkniętego testnetu z obu weryfikacji spełniony): fail-closed `audit-evidence.sh`, bramki czystego drzewa w obu skryptach build, blokujący supply-chain z zatwierdzonym `deny.toml`, asercja komunikatu w drugim release-guardzie, polityka release w README z poprawionymi liczbami testów. Szczegóły w §7 i tabeli wyżej.

**Stos zależności (z pierwszego blokującego biegu supply-chain):**
1. Upgrade stosu Solana/Anchor tak, by 8 ignorowanych advisories RUSTSEC (§7) wypadło z drzewa; opróżnienie obu list wyjątków. Wymagane przed DoD immutable mainnet; wyjątki wygasają przy przeglądzie kwartalnym (najbliższy: 2026-10-19).

**Testy (rekomendacja wszystkich trzech rund audytu):**
2. Testy property-based / fuzz inwariantów księgowości XNT (monotoniczność indeksu, zachowanie sumy przez fund/settle/claim/forfeit, spójność checkpointów), różnicowo względem modelu referencyjnego `core/`.

**Operacje:**
3. Aktualizacja bota dziennego (`/opt/anl-bot/`, prywatne środowisko W5) pod nową sygnaturę `fund_xnt(amount, epoch)`, konta checkpointów i checkpointowanie przy settle — obecny bot pochodzi sprzed modelu epok i jest niekompatybilny z kontraktem.

**Checklista wdrożeniowa (przed testnetem):**
4. Finalny Program ID przez `anchor keys sync` (+ `declare_id!`, `Anchor.toml`, rebuild, ponowna weryfikacja wszystkich PDA); osobne Program ID dla buildu testnetowego (`test-periods` + `network-testnet`) i mainnetowego (`network-mainnet`) (R1-03).
5. Otagowany, czysty commit; pełny bieg CI dokładnie na tym commicie; `docs/TEST-LOG.txt` wygenerowany naprawionym skryptem dowodowym na realnym checkout'cie Gita z zapisanym HEAD.
6. Porównanie IDL sprzed i po zmianie nazw handlerów przed deployem (discriminatory instrukcji oczekiwane bez zmian; zweryfikować).
7. Okres obserwacji na testnecie na aktywach bez wartości lub o ściśle ograniczonej ekspozycji; upgrade authority przez cały czas przy multisig — **zero `--final`, zero kasowania kluczy** na tym etapie.

**Definition of Done — immutable mainnet** (za weryfikacją GPT z 19.07.2026, przyjęte przez zespół; każdy punkt musi być zielony, po kolei):
1. Finalny Program ID (`anchor keys sync` + `declare_id!` + `Anchor.toml` + rebuild + weryfikacja wszystkich PDA).
2. `anchor build --verifiable` + `anchor verify` na X1 (potwierdzenie semantyki loadera) (R1-06).
3. Pełny bieg **naprawionego** `scripts/audit-evidence.sh` na czystym drzewie Gita, wynik w `docs/TEST-LOG.txt` powiązany z `git rev-parse HEAD`.
4. Wszystkie testy zielone na toolchainie 1.89: anl-math 24, core 34, integracja 4 — w obu wariantach.
5. Negatywne buildy strażników (`network-mainnet`+`test-periods` oraz `network-mainnet`+`network-testnet`) nadal się nie kompilują.
6. `cargo clippy --workspace --all-targets -- -D warnings` czysty w obu wariantach.
7. `cargo audit` i `cargo deny check` bez ustaleń critical/high — jako **blokujące** joby CI (V-01) i z **pustymi listami wyjątków** (upgrade stosu z §9 ukończony).
8. Manifest release ze `scripts/build-mainnet.sh` (HEAD + features + sha256 binarki + wersja rustc) dołączony do release notes.
9. Upgrade authority pozostaje aktywna, dopóki punkty 1–8 nie są w całości zielone; `--final` jest **ostatnim** krokiem tej listy i nigdy nie jest wykonywany wcześniej (zasada floty: zero `--final` w obecnej fazie — decyzję o finalizacji podejmuje jawnie, na końcu, posiadacz authority).

---

## 10. Historia dokumentu

| Data | Zmiana |
|------|--------|
| 19.07.2026 | Pierwsze wydanie skonsolidowane (EN+PL); zastępuje dopisywany rundami `AUDIT-RESPONSE.md`; obejmuje rundy #1–#3 i **oba** niezależne raporty weryfikacyjne po rundzie #3 (GPT + Grok), w tym ustalenie M-02 i Definition of Done dla immutable mainnet |
| 19.07.2026 (później) | V-01…V-05 naprawione (pipeline dowodowy, CI, README); zidentyfikowana i usunięta przyczyna czerwonego lintu w CI (niesformatowany `initialize.rs`); statusy i §9 zaktualizowane |
| 19.07.2026 (później) | Pierwszy blokujący bieg supply-chain: 8 advisories RUSTSEC w stosie narzędziowym Solany 1.x striażowane; udokumentowane wyjątki w `.cargo/audit.toml` + `deny.toml`; zadanie upgrade'u stosu dodane i wpięte w DoD |
