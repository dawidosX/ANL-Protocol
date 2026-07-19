# ANL Staking Protocol — audyt bezpieczeństwa

> **Data audytu:** 2026-07-18 · **Zakres:** commit `cf7692b` (main), program `programs/anl_staking` + `crates/anl-math` · **Metoda:** ręczny przegląd całości kodu on-chain (100% instrukcji i stanu), analiza inwariantów wypłacalności, wektorów czasowych, PDA/kont i wektorów ekonomicznych. Kontynuacja [analizy ogólnej](2026-07-18_analiza-ogolna_claude-fable5.md).
>
> **Uwaga:** audyt wykonany przez Claude Code (AI, model **Claude Fable 5**). Nie zastępuje niezależnego audytu zewnętrznego wymaganego przed mainnetem — jest wkładem do Fazy 3.

---

## 1. Podsumowanie wykonawcze

Kod jest **jak na tę fazę projektu wyraźnie ponadprzeciętny**: nie znalazłem żadnej podatności krytycznej ani wysokiej — żadnej ścieżki kradzieży środków, podwójnej wypłaty, manipulacji nagrodami ani trwałego zablokowania wyjścia użytkownika. Kluczowe inwarianty (pokrycie nagród, rozdzielność skarbców, monotoniczność indeksu XNT) są utrzymane we wszystkich przeanalizowanych ścieżkach.

Znalazłem natomiast **5 ustaleń średnich** — wszystkie dotyczą warstwy zaufania i procedur (przejęcie `initialize`, pojedynczy klucz authority, zaufanie do rozszerzeń mintów Token-2022, ekonomiczny griefing rezerwacji nagród, brak ścieżek odzysku) — oraz **7 niskich/hardeningowych** i garść informacyjnych.

| Poziom | Liczba | Numery |
|---|---|---|
| Krytyczny | 0 | — |
| Wysoki | 0 | — |
| **Średni** | **5** | M-1…M-5 |
| Niski | 7 | L-1…L-7 |
| Informacyjny | 5 | I-1…I-5 |

## 2. Co zweryfikowałem (inwarianty — wynik pozytywny)

**Wypłacalność ANL (nagrody).** Jedyny odpływ z Reward Vault to `claim`, który płaci dokładnie `anl_reward` i o tyle samo zmniejsza `anl_reward_reserved`; `stake` rezerwuje z wymogiem pokrycia saldem (`RewardCoverageExceeded`), `unstake_early` zwalnia rezerwację bez wypłaty. Inwariant `reward_vault.amount ≥ anl_reward_reserved` **zachodzi zawsze** — `InsufficientRewardVault` w `claim` jest martwym zabezpieczeniem głębi (dobrze, że jest).

**Wypłacalność principalu.** Wpłaty księgowane jako *actual received* (diff salda po transferze — poprawnie obsługuje transfer fee Token-2022), wypłaty debetują skarbiec dokładnie o `position.amount`. Saldo Principal Vault ≥ suma otwartych pozycji — zawsze.

**Wypłacalność XNT.** Dystrybucja floor do indeksu (dust zostaje w vaulcie), `pending ≤ distributed`, forfeit wraca do `xnt_undistributed` i jest re-dystrybuowany. Brak ścieżki wypłaty ponad naliczenie.

**Podwójne rozliczenia.** `settled` + `PositionStatus::Closed` + `close = owner` wykluczają double-settle i double-claim; zamknięte konto pozycji nie przejdzie ponownej deserializacji. Licznik `next_position_index` w nigdy niezamykanym `UserProfile` wyklucza reużycie PDA pozycji.

**Wyścig settle ↔ unstake_early.** `settle_expired` wymaga `now ≥ end_ts`, `unstake_early` wymaga `now < end_ts`; zegar Solany jest monotoniczny — podwójne zdjęcie `shares` jest nieosiągalne (patrz jednak L-7).

**PDA i konta.** Wszystkie seeds kanoniczne, bumpy z konfiguracji lub przeliczane, minty walidowane po adresie z `GlobalConfig`, programy tokenowe wymuszone typami (`Token2022` vs `Token`) — podstawienie konta nie przechodzi. Brak arbitralnych CPI; reentrancja na Solanie wykluczona (brak self-CPI).

**Arytmetyka.** Wszędzie `checked_*` + `overflow-checks = true` w release; jedyne nie-`checked` działania (`split_xnt`) są dowodliwie bezpieczne (część ≤ całość). Zaokrąglenia konsekwentnie floor — na korzyść protokołu. Granice okien APY (dzień 31/91) zgodne z White Paperem, pokryte testami TC-040…044. Zero `unwrap`/`panic!`/`unsafe` w programie.

**Wyjście użytkownika.** `pause` blokuje wyłącznie `stake`; `claim`/`unstake_early`/`settle_expired` nie czytają flagi `paused` — użytkownik nie może zostać uwięziony przez operatora.

## 3. Ustalenia średnie

### M-1 · Front-running `initialize` — przejęcie roli authority
[initialize.rs:14-76](../../programs/anl_staking/src/instructions/initialize.rs) nie wiąże sygnatariusza z niczym — **pierwszy, kto wywoła `initialize` po deployu, zostaje trwałym authority** (PDA `global_config` jest jednorazowe, a `set_authority` nie istnieje). Atakujący obserwujący sieć może wyścignąć transakcję zespołu; jedyny ratunek to redeploy pod nowym Program ID.
**Rekomendacja:** zahardkodować oczekiwany pubkey authority w programie **lub** wymagać, by sygnatariusz był upgrade authority programu (weryfikacja przez `program_data`), **lub** proceduralnie: deploy + initialize w jednej transakcji/bundlu.

### M-2 · Pojedynczy klucz authority bez rotacji i timelocka
`GlobalConfig.authority` to jeden `Pubkey` (komentarz „multisig" nie jest niczym wymuszony on-chain). Kompromitacja klucza = wieczna pauza wejść (grief), zatrzymanie fundingu XNT (zanik strumienia nagród) i brak jakiejkolwiek ścieżki rotacji (brak `set_authority`). Klucz nie może ukraść środków (to potwierdzam), ale może trwale zdegradować protokół.
**Rekomendacja:** instrukcja `transfer_authority` (two-step: propose/accept), realny multisig (np. Squads) przed mainnetem; rozważyć timelock na `resume`→`pause` nadużycia nie ma, ale rotacja to minimum.

### M-3 · Zaufanie do rozszerzeń mintu ANL (Token-2022)
Program sprawdza tylko, że mint ANL należy do Token-2022, **nie inspekcjonuje rozszerzeń**. Jeśli mint ma (lub kiedyś dostanie — część rozszerzeń jest konfigurowalna przez authority mintu): **permanent delegate** → możliwość wyssania skarbców poza logiką programu; **freeze authority** → zamrożenie kont skarbców = zablokowanie wszystkich wyjść; **transfer hook** → wybiórcze blokowanie wypłat. Analogicznie freeze authority na XNT. Cała gwarancja „user nigdy nie jest uwięziony" wisi na konfiguracji mintów, nie na programie.
**Rekomendacja:** w `initialize` odrzucić minty z permanent delegate / transfer hook / freeze authority (lub jawnie udokumentować i zrenuncjować te uprawnienia na mintach przed startem); dodać do checklisty deploymentu weryfikację rozszerzeń.

### M-4 · Griefing pojemności nagród przez w pełni zwrotny `unstake_early`
Model all-or-nothing czyni atak tanim: whale stake'uje duży principal na **3650 dni** w oknie 20% → rezerwuje ~2× principal z pojemności Reward Vault → kolejni użytkownicy dostają `RewardCoverageExceeded` → whale w dowolnej chwili robi `unstake_early` i **odzyskuje 100% kapitału** (traci tylko fee transakcji). Cykl można powtarzać, trwale tłumiąc wejścia w najcenniejszym oknie Genesis. Przy rezerwie 200M ANL zablokowanie całej pojemności w W1 wymaga ~100M ANL kapitału — dużo, ale częściowa degradacja skaluje się liniowo i jest darmowa.
**Rekomendacja (do decyzji produktowej):** limit kwoty/pozycji per wallet, minimalny okres karencji przed `unstake_early`, symboliczna opłata za zerwanie, albo świadoma akceptacja ryzyka + monitoring i reakcja pauzą.

### M-5 · Brak ścieżek odzysku i administracji drugiej fazy życia
Potwierdzam ustalenia analizy ogólnej w randze audytowej: nadwyżka ANL w Reward Vault ponad rezerwacje jest **nieodzyskiwalna na zawsze** (w tym każda darowizna wysłana wprost na skarbiec — L-6); `genesis_start_ts` jest niemutowalne (literówka = redeploy); `PoolStatus::Paused/Closed` istnieją, ale żadna instrukcja ich nie ustawia. To nie są podatności, lecz **jednokierunkowe drzwi operacyjne** — każda pomyłka konfiguracyjna eskaluje do redeployu.
**Rekomendacja:** przed mainnetem świadomie rozstrzygnąć: dodać `withdraw_reward_surplus` (tylko ponad `anl_reward_reserved` — inwariant pokrycia nie ucierpi) i `update_genesis_start` (tylko przed startem), albo udokumentować nieodwracalność jako feature („proof of commitment").

## 4. Ustalenia niskie

- **L-1 · `MIN_STAKE_AMOUNT` zakłada 9 decimals** ([constants.rs:13](../../programs/anl_staking/src/constants.rs)): `1_000_000_000` = „1 ANL" tylko przy decimals = 9; `initialize` nie waliduje `anl_mint.decimals`. Przy innym mincie min. stake będzie 1000× za duży/za mały. Dodać `require!(anl_mint.decimals == 9)` lub wyliczać z decimals.
- **L-2 · Brak górnej granicy `genesis_start_ts`** — fat-finger (np. rok 3026) blokuje staking na zawsze (`NotStarted`), bez możliwości korekty (patrz M-5). Dodać sanity-check (np. ≤ now + 90 dni).
- **L-3 · Sprawiedliwość dzienna XNT zależy od off-chain bota** — spóźniony `fund_xnt` po niewykonanym `settle_expired` oddaje wygasłym pozycjom udział w dystrybucji dnia. Permissionless settle sprawia, że **każdy** może wyegzekwować poprawność (dobry design), ale nikt nie ma bodźca. Rozważyć inline-settle sweep lub po prostu redundancję botów.
- **L-4 · Przechwycenie `xnt_undistributed` z okresu pustego koszyka** (D-5, by-design): XNT sparkowane, gdy pula była pusta, trafia w całości do stakerów obecnych przy **następnym** fundingu — pierwszy staker w pustej puli może celowo wejść tuż przed fundingiem i przejąć zaległości. Wymaga jednak dotrwania do `end_ts` (min. 7 dni, `unstake_early` = przepadek), więc opłacalność ograniczona. Zaakceptować świadomie lub dystrybuować zaległości proporcjonalnie w czasie.
- **L-5 · `init_if_needed` na `UserProfile`** ([stake.rs:50-57](../../programs/anl_staking/src/instructions/stake.rs)): obecnie bezpieczne (konto nigdy nie jest zamykane, discriminator chroni przed re-init, licznik tylko rośnie, a inicjalizacja pól za `if owner == default` jest poprawna). Utrzymać niezmiennik „profil nigdy nie jest zamykany" przy przyszłych zmianach — to warunek bezpieczeństwa PDA pozycji.
- **L-6 · Darowizny wprost na skarbce są wieczne** — transfer ANL/XNT bezpośrednio na PDA vaultów omija księgowość: na Reward Vault **zwiększa pojemność stake'ów** (benign, wręcz użyteczne), na pozostałych — martwy balast. Brak skim/sweep. Powiązane z M-5.
- **L-7 · Hardening: brak `require!(!pos.settled)` w `unstake_early`** — dziś nieosiągalne (rozłączność predykatów czasowych + monotoniczny zegar), ale jedna linia guardu uodparnia funkcję na przyszłe zmiany semantyki `settled`/`end_ts`. Analogicznie można jawnie sprawdzać `version == ACCOUNT_VERSION` przy deserializacji (pole dziś zapisywane, nigdy nie czytane).

## 5. Informacyjne

- **I-1 · Martwy kod:** `PoolStatus::Paused/Closed` nigdy nie ustawiane; nieużywane errory (`ClaimFirst`, `NothingToClaim`, `PoolNotEmpty`, `PendingObligations`, `InvalidAccountVersion`, `PoolClosed`). Usunąć albo dosztukować logikę — martwe warianty sugerują audytorowi funkcje, których nie ma.
- **I-2 · Placeholder Program ID** (`Fg6Pa…`) — znane, wymaga `anchor keys sync` przed deployem.
- **I-3 · Stos zależności:** Anchor 0.29 (2023) + solana 1.18 w dev-deps; brak znanych podatności trafiających w używane wzorce, ale plan podbicia wersji powinien być częścią Fazy 3.
- **I-4 · `cranker` w `settle_expired` jako czysty Signer** — poprawne (permissionless z płatnikiem fee); settle „dokładnie o czasie" przez osobę trzecią to egzekwowanie spec, nie grief.
- **I-5 · Pozytywy warte utrzymania:** actual-received na obu kierunkach, floor-only, rezerwacja przy otwarciu, rozdzielność trzech skarbców, wyjścia niezależne od pauzy, testy graniczne okien i dust (TC-126). Model referencyjny + wspólny crate matematyczny to realna wartość przy differential testingu.

## 6. Priorytety przed mainnetem

1. **Procedura deploymentu odporna na M-1** (bundle deploy+initialize albo weryfikacja upgrade authority w programie).
2. **M-2 + M-5:** `transfer_authority` (two-step) + decyzja o `withdraw_reward_surplus` / `update_genesis_start`; authority na realnym multisigu.
3. **M-3:** weryfikacja/renuncjacja rozszerzeń mintów w checkliście (najlepiej: walidacja w `initialize`).
4. **M-4:** decyzja produktowa o limitach/karencji dla `unstake_early`.
5. L-1/L-2 jako tanie `require!` w `initialize`; L-7 jako jednolinijkowy hardening.
6. Dokończenie Fazy 3 zgodnie z planem: fuzzing 24h+, testnet X1, **niezależny audyt zewnętrzny** — niniejszy dokument go nie zastępuje.

## 7. Werdykt

W przeanalizowanym kodzie **nie ma znanych mi ścieżek utraty środków użytkownika**: inwarianty wypłacalności są spójne, ścieżki wyjścia bezwarunkowe, arytmetyka dyscyplinowana, konta poprawnie związane PDA. Ryzyko koncentruje się w warstwie **zaufania i operacji**: jednoosobowe authority bez rotacji, zaufanie do konfiguracji mintów Token-2022, jednorazowe i nieodwracalne decyzje konfiguracyjne oraz ekonomiczny grief rezerwacji. Wszystkie ustalenia średnie mają tanie, dobrze znane mitygacje — nadają się do domknięcia w ramach Fazy 3, przed powierzeniem protokołowi realnych środków.

---

*Audyt AI (Claude Code, 2026-07-18) — przegląd ręczny 100% kodu on-chain na commicie `cf7692b`. Nie stanowi audytu zewnętrznego w rozumieniu roadmapy Fazy 3.*
