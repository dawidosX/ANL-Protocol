# ANL Staking Protocol — audyt bezpieczeństwa OpenAI

> **Data audytu:** 2026-07-18  
> **Audytowany stan:** commit `cf7692b` (`main`)  
> **Zakres:** `programs/anl_staking`, `crates/anl-math`, `core`, testy, konfiguracja Anchor/Cargo/CI, dokumentacja i zależności  
> **Audytor:** OpenAI Codex, model **5.6sol** (audyt AI, niezależny przegląd kodu)  
> **Werdykt:** **NIE GOTOWY DO MAINNETU**

Niniejszy dokument jest audytem kodu źródłowego, a nie certyfikatem bezpieczeństwa. Nie zastępuje audytu wykonanego przez niezależny, wyspecjalizowany zespół ani testów na docelowym środowisku X1 Network.

---

## 1. Podsumowanie wykonawcze

W kodzie nie znalazłem bezwarunkowej ścieżki typu „dowolny użytkownik natychmiast wypłaca cały vault”, błędu PDA umożliwiającego podmianę kont ani prostego double-claim. Podstawowe mechanizmy Anchora, rozdzielenie skarbców, księgowanie principalu netto, rezerwowanie nagród ANL i arytmetyka `checked_*` są wykonane starannie.

Mimo tych zalet protokół **nie powinien zostać wdrożony z realnymi środkami w obecnym stanie**. Potwierdziłem cztery problemy wysokiej wagi:

1. użytkownik może odzyskać „przepadłe” przy wcześniejszym wyjściu XNT przez własną drugą pozycję;
2. naliczanie XNT nie zatrzymuje się on-chain w `end_ts` — wynik zależy od kolejności `settle_expired` i `fund_xnt`;
3. bezpieczeństwo custody i możliwość wyjścia zależą od niezweryfikowanych uprawnień oraz rozszerzeń mintów;
4. pierwszy caller publicznego `initialize` może trwale przejąć konfigurację wdrożenia.

Dodatkowo rzeczywisty build SBF nie powstał w audytowanym środowisku, CI nie buduje artefaktu on-chain, Program ID pozostaje placeholderem, a aktualny skan RustSec zgłasza osiem podatności w głównym `Cargo.lock`.

### Klasyfikacja ustaleń

| Poziom | Liczba | Identyfikatory |
|---|---:|---|
| Krytyczny | 0 | — |
| **Wysoki** | **4** | H-01…H-04 |
| **Średni** | **4** | M-01…M-04 |
| Niski | 7 | L-01…L-07 |
| Informacyjny / pozytywny | kilka | I-01…I-04 |

Ocena `High` oznacza możliwość materialnego błędu ekonomicznego, utraty lub zablokowania aktywów albo trwałego przejęcia/degradacji wdrożenia. Ocena ryzyka uwzględnia zarówno wpływ, jak i warunki konieczne do wykorzystania. H-03 i H-04 są zależne od konfiguracji wdrożenia, ale repozytorium nie zawiera danych pozwalających uznać te warunki za wykluczone.

---

## 2. Zakres, metoda i założenia

Przejrzano ręcznie wszystkie instrukcje on-chain, definicje stanu i funkcje matematyczne. Analiza obejmowała:

- autoryzację, signerów, PDA, seeds/bumpy i wiązanie kont;
- CPI do legacy SPL Token i Token-2022;
- cykl `initialize → stake → fund → settle/claim/unstake`;
- inwarianty principalu, rezerw ANL i dystrybucji XNT;
- overflow/underflow, zaokrąglenia, granice czasu i błędy kolejności;
- ataki ekonomiczne, Sybil, griefing i liveness;
- testy, model referencyjny, CI, build SBF i supply chain;
- statyczny skan sekretów.

Nie audytowano frontendu, SDK, bota rozliczającego, infrastruktury kluczy, realnej konfiguracji ProgramData/upgrade authority ani stanu wdrożenia na X1. W repozytorium nie ma produkcyjnego adresu programu ani adresów docelowych mintów, dlatego H-03/H-04 muszą zostać zweryfikowane ponownie na rzeczywistych kontach on-chain.

---

## 3. Blokery wdrożenia

Poniższe punkty nie są osobnymi exploitami kodu, ale każdy z nich samodzielnie blokuje bezpieczny release:

| ID | Bloker | Dowód |
|---|---|---|
| B-01 | Program ID jest domyślnym placeholderem `Fg6PaFpo…` | `programs/anl_staking/src/lib.rs:15-16`, `Anchor.toml:8-9` |
| B-02 | `anchor build` nie tworzy artefaktu: używany `solana-cargo-build-sbf 1.17.0`/Rust 1.68 nie czyta `Cargo.lock` v4 | odtworzone podczas audytu |
| B-03 | CI nie wykonuje `anchor build`/`cargo build-sbf`; testy uruchamiają natywny procesor, a nie `.so` | `.github/workflows/ci.yml:21-26`, `integration.rs:24-35` |
| B-04 | Build `test-periods` zmienia ekonomię, a jedynym bezpiecznikiem jest log podczas `initialize` | `Cargo.toml:11-16`, `initialize.rs:83-84` |
| B-05 | Brak produkcyjnej konfiguracji X1, hashy artefaktu, procedury upgrade authority i weryfikowalnego release | `Anchor.toml:8-17` |

Przed deploymentem należy przypiąć kompatybilny zestaw Rust/Anchor/X1 CLI/platform-tools, budować SBF z `--locked` w CI, uruchamiać testy przeciw skompilowanemu `.so`, zapisywać hash/SBOM oraz użyć osobnych Program ID dla buildów produkcyjnych i testowych.

---

## 4. Ustalenia wysokie

### H-01 — „Przepadłe” XNT można odzyskać przez własną pozycję-sybilę

**Waga:** wysoka  
**Pewność:** wysoka  
**Kod:** `state/mod.rs:77-89,118-128`, `lifecycle.rs:347-359`, `integration.rs:691-743`

`unstake_early` oblicza XNT naliczone dużej pozycji, usuwa jej shares, a całą wartość dopisuje do wspólnego `pool.xnt_undistributed`. Przy następnym `fund_xnt_part` ta wartość zostaje wprowadzona do bieżącego indeksu i przypisana wszystkim aktualnym shares. Mechanizm nie rozróżnia właścicieli ani historii ekspozycji.

#### Scenariusz ataku

1. Atakujący otwiera dużą, długoterminową pozycję A oraz minimalną pozycję B w tej samej puli. B może należeć do tego samego klucza lub do adresu-sybila.
2. Funding XNT sprawia, że niemal cała nagroda przypada dużej pozycji A.
3. Przed terminem A, lecz przed terminem B, atakujący wykonuje `unstake_early(A)`. Principal A wraca, a XNT A trafia do `xnt_undistributed`.
4. Jeżeli B jest jedyną pozostałą pozycją, kolejny funding przypisuje B 100% przepadku A, niezależnie od minimalnego rozmiaru B.
5. Po osiągnięciu terminu B atakujący wypłaca odzyskane XNT.

Istniejący test integracyjny już potwierdza prymityw: po early exit pozycji A jej 175 000 XNT jest w następnym fundingu dopisywane pozycji B (`integration.rs:718-743`). Zmiana B na drugą pozycję tego samego uczestnika daje opisany bypass.

#### Wpływ

Użytkownik uwalnia duży principal przed zadeklarowanym terminem, zachowując ekonomicznie XNT, które według modelu „all-or-nothing” powinno przepaść. Przy niskim TVL odzysk może wynieść praktycznie 100%; przy innych stakerach atakujący odzyskuje część proporcjonalną do swoich pozostałych shares.

#### Rekomendacja

Jeśli early exit ma rzeczywiście odbierać XNT, nie wolno wprowadzać przepadku z powrotem do indeksu dostępnego bieżącym stakerom. Należy skierować go do treasury/burn/protocol reserve albo zdefiniować inną, niesybilowalną karę. Wykluczenie `owner` nie wystarczy — nowy adres obchodzi taką kontrolę. Dodać test tej samej sekwencji z dwiema pozycjami jednego właściciela i z dwoma adresami-sybilami.

---

### H-02 — `end_ts` nie zatrzymuje naliczania XNT; wynik zależy od bota i kolejności transakcji

**Waga:** wysoka  
**Pewność:** wysoka  
**Kod:** `fund.rs:126-159`, `state/mod.rs:77-113`, `lifecycle.rs:50-63,151-177`, `README.md:42-48`

Indeks XNT nie zna czasu ani dat końca pozycji. Shares są usuwane dopiero przez osobne `settle_expired` lub inline-settle w `claim`. Jeżeli `fund_xnt` wykona się po `end_ts`, lecz przed którymkolwiek settle, wygasła pozycja nadal uczestniczy w podziale i pobiera nagrody po terminie.

#### Minimalny ślad stanu

- A: 100 shares, już po `end_ts`, nadal `settled == false`;
- B: 100 shares, aktywna;
- funding puli: 1 000 XNT przed settle;
- indeks rośnie o `1000 × PRECISION / 200`;
- późniejszy inline-settle A odczytuje bieżący indeks i przypisuje A 500 XNT, mimo że cała transza pojawiła się po jego terminie.

Problem działa też w przeciwną stronę: revenue ekonomicznie należne za wcześniejszy dzień, ale wpłacone po settle, całkowicie omija pozycję, która była wtedy aktywna. Protokół nie ma `funding_epoch`, `last_funded_day`, cutoffu ani dowodu okresu, za który pochodzi wpłata. O wyniku decyduje wyłącznie kolejność transakcji.

Dokumentacja wymaga od bota wykonania wszystkich settle przed fundingiem. To nie jest inwariant on-chain. Atakujący może tanio utworzyć wiele minimalnych pozycji z podobnym terminem, zmuszając operatora do O(N) transakcji settle przed każdym fundingiem. Operator musi wtedy opóźnić funding albo zaakceptować błędny podział.

Model `core::XntPool::settle` również nie przyjmuje `now` ani `end_ts`; test `wp_s7_settle_freezes_accrual_at_period_end` jedynie ręcznie umieszcza settle między fundingami i nie może wykryć tego błędu (`core/src/lib.rs:525-538`).

#### Wpływ

Bezpośrednia redystrybucja XNT od prawidłowo aktywnych pozycji do wygasłych, manipulowalność timingiem przez operatora oraz liveness/DoS warstwy rozliczającej. Narusza to deklarację WP §7–8, że opóźnienie claim nie zwiększa nagrody, a pozycja po terminie już nie zarabia.

#### Rekomendacja

Przebudować księgowość na checkpointy per dzień/epoch albo agregowane expiry buckets. Pozycja powinna rozliczać się względem indeksu obowiązującego dla jej `end_ts`, niezależnie od chwili claim. `fund_xnt` powinien zawierać monotoniczny identyfikator okresu i nie może wymagać nieograniczonej listy wygasłych pozycji do zachowania poprawności. Redundantne boty i monitoring mogą być mitygacją przejściową, ale nie zamykają podatności.

---

### H-03 — Niezweryfikowane uprawnienia i rozszerzenia mintów naruszają custody oraz możliwość wyjścia

**Waga:** wysoka, warunkowa  
**Pewność:** wysoka co do kodu; konfiguracja realnych mintów nieznana  
**Kod:** `initialize.rs:32-74,78-96`, `lifecycle.rs:116-149,196-244,318-378`

`initialize` sprawdza, że ANL jest mintem Token-2022, a XNT mintem legacy SPL Token, lecz nie sprawdza ich uprawnień ani profilu rozszerzeń.

Konsekwencje:

- **Permanent Delegate** ANL może autoryzować transfer lub burn z dowolnego token accountu tego mintu, w tym Principal i Reward Vault, poza logiką ANL Protocol. Potwierdza to [oficjalna dokumentacja Solany](https://solana.com/docs/tokens/extensions/permanent-delegate).
- **Freeze authority** ANL albo XNT może zamrozić vault. Ponieważ `claim` jest atomowy i obejmuje XNT, awaria transferu XNT blokuje również principal oraz ANL po terminie; użytkownik nie może już użyć `unstake_early`. Zob. [dokumentacja freeze/thaw](https://solana.com/docs/tokens/basics/thaw-account).
- Zmienny **TransferFeeConfig** może zostać podniesiony przed wyjściem. Program wysyła zaksięgowany principal jako kwotę brutto, więc odbiorca otrzymuje mniej, mimo że pozycja i `total_staked` są zamykane pełną kwotą. Wzorzec `actual received` chroni wypłacalność wejścia, nie kwotę netto wyjścia. Fee authority może następnie odebrać withheld fees; zob. [oficjalny opis transfer fees](https://solana.com/de/docs/tokens/extensions/transfer-fees).
- **Transfer Hook** wymaga przekazania wszystkich dodatkowych kont hooka. Kod używa CPI z czterema podstawowymi kontami i nie przekazuje `remaining_accounts`, więc taki mint może trwale odrzucać transfery; wymaganie dodatkowych kont opisuje [oficjalny przewodnik Transfer Hook](https://solana.com/pt/developers/guides/token-extensions/transfer-hook).
- `DefaultAccountState::Frozen`, `NonTransferable` i inne nieobsługiwane rozszerzenia mogą uniemożliwić inicjalizację lub użycie vaultów.

Przykład dla fee: przy 5% fee użytkownik wysyła 100, pozycja zapisuje 95 netto, a przy wyjściu transfer 95 daje odbiorcy 90,25. Dokumentowane „principal wraca w całości” nie zachodzi.

#### Rekomendacja

W `initialize` sparsować pełny stan mintu i egzekwować jawną allowlistę rozszerzeń. Co najmniej:

- brak Permanent Delegate i Transfer Hook;
- `freeze_authority == None` dla ANL i XNT;
- brak TransferFeeConfig albo opłata zerowa, niezmienna i zgodna z jednoznaczną semantyką brutto/netto;
- wymagane `decimals` i stan aktywny;
- odrzucenie rozszerzeń nieobjętych testami.

Jeżeli któryś uprzywilejowany mechanizm jest wymagany biznesowo, musi być jawnie wpisany do modelu zaufania, kontrolowany przez multisig/timelock i obsłużony w programie. Przed release należy dołączyć zrzut oraz niezależną weryfikację realnych kont mintów i wszystkich authorities.

---

### H-04 — Publiczny, jednorazowy `initialize` może zostać przejęty po deployu

**Waga:** wysoka, zależna od fazy deploymentu  
**Pewność:** wysoka  
**Kod:** `initialize.rs:14-96`

`authority` jest dowolnym signerem. Pierwszy caller tworzący singleton PDA `global_config`:

- zostaje trwałym `GlobalConfig.authority`;
- wybiera minty ANL/XNT;
- wybiera `genesis_start_ts` i stan pauzy;
- tworzy wszystkie vaulty.

Nie ma związania z upgrade authority programu, zahardkodowanym inicjalizatorem ani wcześniejszym commitmentem. Ponieważ nie istnieje rotacja authority ani re-initialize, front-running pierwszej transakcji po deployu powoduje trwałe przejęcie/bricking konfiguracji; naprawa wymaga redeployu pod nowym Program ID. Aktualne uprawnienia authority nie dają prostego withdrawu vaultów, ale pozwalają trwale zdegradować protokół i uniemożliwić prawidłowy launch.

Jeśli konkretne wdrożenie zostało już prawidłowo zainicjalizowane, okno ataku dla tego Program ID jest zamknięte. Repozytorium nie pozwala tego potwierdzić.

#### Rekomendacja

Wymagać z góry określonego inicjalizatora albo zweryfikować BPF Upgradeable Loader `ProgramData` i podpis upgrade authority. Dodać dwuetapowe `propose_authority`/`accept_authority`. Procedura deployu powinna dodatkowo monitorować powstanie PDA i sprawdzać wszystkie pola po inicjalizacji, lecz sama kolejność operacyjna nie powinna zastępować kontroli on-chain.

---

## 5. Ustalenia średnie

### M-01 — Griefing pojemności Reward Vault przez długi stake i bezkosztowy early exit

**Kod:** `stake.rs:128-140`, `lifecycle.rs:335-395`

W pierwszym oknie Genesis pozycja na 3 650 dni przy 20% APY rezerwuje około `2 × principal` nagrody. Staker dysponujący 100 mln ANL może zarezerwować praktycznie cały rezerwuar 200 mln ANL, powodując `RewardCoverageExceeded` u kolejnych użytkowników. Następnie może odzyskać cały principal przez `unstake_early`; rezerwacja zostaje natychmiast zwolniona.

Atak kosztuje opłaty, potencjalne transfer fees i czasowe zaangażowanie kapitału, lecz nie wymaga dotrzymania zadeklarowanego okresu. Może być używany do front-runningu lub do blokowania najatrakcyjniejszego okna wejścia.

**Rekomendacja:** ekonomiczna kara/cooldown dla early exit, limit globalnej ekspozycji jednej transakcji/epoki, admission buckets albo rezerwacja narastająca w czasie. Limit per wallet sam w sobie jest sybilowalny.

### M-02 — Authority nie ma rotacji, a model governance nie jest egzekwowany

**Kod:** `state/mod.rs:27-43`, `set_pause.rs:9-31`, `fund.rs:20-43,85-124`

Komentarz nazywa authority multisigiem, lecz on-chain jest to tylko jeden `Pubkey`. Może to być PDA prawdziwego multisiga, ale repozytorium tego nie wymusza ani nie dokumentuje. Brak instrukcji rotacji oznacza, że kompromitacja lub utrata klucza może trwale zatrzymać nowe wejścia i funding XNT. Obecny authority nie może bezpośrednio wypłacić vaultów, więc wpływ jest głównie liveness/operacyjny.

Osobno, jeżeli program pozostaje upgradeable, upgrade authority jest faktyczną najwyższą rolą custody i może wymienić kod. Jej realny stan nie został dostarczony do audytu.

**Rekomendacja:** dwuetapowa rotacja authority, realny multisig, monitoring, runbook utraty klucza oraz jawna decyzja: niezmienny program albo upgrade authority w multisigu/timelocku.

### M-03 — Pierwszy staker pustej puli przechwytuje cały historyczny `xnt_undistributed`

**Kod:** `state/mod.rs:75-89`, `stake.rs:143-183`

Gdy `total_shares == 0`, cała część XNT czeka w `xnt_undistributed`. Po wejściu pierwszej pozycji następny, nawet minimalny funding wprowadza całą zaległość do indeksu aktualnych shares. Użytkownik może wejść tuż przed znanym fundingiem, zostać jedynym stakerem i po minimalnym okresie odebrać 100% nagród z czasu, gdy nie ponosił ekspozycji.

Zachowanie jest opisane jako zasada pustego koszyka, więc może być świadomą decyzją produktową. Jest jednak łatwo przechwytywalne i używa tego samego niehistorycznego mechanizmu co H-01.

**Rekomendacja:** jawnie zaakceptować i komunikować regułę albo kierować pusty koszyk do treasury, amortyzować backlog przez kolejne epoki bądź zapisywać eligibility snapshot.

### M-04 — Główny lockfile zawiera osiem znanych podatności RustSec

Skan wykonany przez `cargo-audit 0.22.2` na bazie 1 166 advisories wykazał 8 podatności oraz 16 ostrzeżeń:

| Graf | Pakiet | Advisory |
|---|---|---|
| normalny programu | `curve25519-dalek 3.2.1` | [RUSTSEC-2024-0344](https://rustsec.org/advisories/RUSTSEC-2024-0344) |
| normalny programu | `ed25519-dalek 1.0.1` | [RUSTSEC-2022-0093](https://rustsec.org/advisories/RUSTSEC-2022-0093) |
| dev/test | `quinn-proto 0.10.6` | [RUSTSEC-2026-0185](https://rustsec.org/advisories/RUSTSEC-2026-0185), [RUSTSEC-2026-0037](https://rustsec.org/advisories/RUSTSEC-2026-0037) |
| dev/test | `ring 0.16.20` | [RUSTSEC-2025-0009](https://rustsec.org/advisories/RUSTSEC-2025-0009) |
| dev/test | `rustls-webpki 0.101.7` | [RUSTSEC-2026-0104](https://rustsec.org/advisories/RUSTSEC-2026-0104), [RUSTSEC-2026-0098](https://rustsec.org/advisories/RUSTSEC-2026-0098), [RUSTSEC-2026-0099](https://rustsec.org/advisories/RUSTSEC-2026-0099) |

Sześć advisories dotyczy przede wszystkim narzędzi testowych/sieciowych i nie trafia do logiki SBF. Dwa stare pakiety kryptograficzne są obecne w normalnym grafie przez Solana/Anchor, ale ich podatne ścieżki nie zostały wykazane jako osiągalne w tym kontrakcie. Brak udanego builda SBF uniemożliwił ostateczną inspekcję binarki. RustSec dodatkowo oznacza szereg pakietów jako unmaintained/unsound.

**Rekomendacja:** migrować Anchor/Solana/SPL jako przetestowany, kompatybilny z X1 zestaw; ograniczyć domyślne features `anchor-spl`; uruchamiać `cargo audit`/OSV w CI i utrzymywać udokumentowane wyjątki wyłącznie po analizie osiągalności.

---

## 6. Ustalenia niskie i hardening

### L-01 — Minimalny stake na sztywno zakłada 9 decimals

`MIN_STAKE_AMOUNT = 1_000_000_000` oznacza 1 ANL tylko przy `decimals == 9`, ale `initialize` tego nie wymusza (`constants.rs:12-17`, `initialize.rs:32-38`). Przy 6 decimals minimum wynosi 1 000 ANL; przy 12 decimals — 0,001 ANL, co ułatwia także spam pozycjami. `core::min_stake_amount` liczy wartość dynamicznie, lecz program jej nie używa.

**Naprawa:** wymusić dokładne decimals albo wyliczać minimum z bezpiecznym `checked_pow` i zapisać przyjętą semantykę w konfiguracji.

### L-02 — Brak ochrony użytkownika przed zmianą APY/fee podczas oczekiwania transakcji

`stake` przyjmuje tylko `amount` i `declared_days`. Nie ma `expected_apy_bps`, `min_net_received` ani deadline. Transakcja wchodząca na granicy dnia 31/91 może otrzymać niższe APY niż podpisywane przez użytkownika; zmiana TransferFeeConfig może obniżyć zaksięgowany principal. Blockhash ogranicza czas oczekiwania, ale nie usuwa ryzyka granicznego.

**Naprawa:** argumenty ochronne użytkownika i `require!` na minimalny wynik/oczekiwane APY oraz test transakcji na granicy okna.

### L-03 — `genesis_start_ts` nie egzekwuje wymaganej godziny ani rozsądnego horyzontu

Kod sprawdza wyłącznie `genesis_start_ts >= now` (`initialize.rs:78-82`). Komentarze mówią o pełnej godzinie, a WP o 02:00 UTC, lecz nie ma `ts mod 86400 == 7200`; bardzo odległy timestamp również przechodzi. Parametr jest potem niemutowalny.

**Naprawa:** walidacja 02:00 UTC, maksymalnego horyzontu oraz możliwość korekty wyłącznie przed otwarciem pierwszej pozycji.

### L-04 — Pierwsze zaokrąglenie indeksu może zgubić całą małą transzę

`delta = floor(distributed × 1e12 / total_shares)`, po czym `xnt_undistributed` jest zawsze zerowane (`crates/anl-math/src/lib.rs:93-105`, `state/mod.rs:77-89`). Przy `total_shares = u64::MAX` każda transza do 18 446 744 bazowych jednostek daje `delta == 0`; tokeny pozostają w vault, ale przestają być zaksięgowane do przyszłej dystrybucji. Strata może kumulować się z liczbą fundingów.

**Naprawa:** przenosić resztę licznika albo obliczać faktycznie wprowadzoną kwotę i zostawiać różnicę w `xnt_undistributed`. Dodać testy `total_shares >> PRECISION` i `delta == 0`.

### L-05 — Nadwyżki i bezpośrednie transfery do vaultów mogą zostać zablokowane na zawsze

Nie ma bezpiecznej ścieżki odzysku nadwyżki Reward Vault ponad `anl_reward_reserved`, darowizn do Principal Vault ani XNT przesłanego bezpośrednio z pominięciem `fund_xnt`. W przypadku XNT prosty sweep byłby niebezpieczny, bo program nie przechowuje globalnej wartości zobowiązań settled/active.

**Naprawa:** najpierw formalnie zdefiniować zobowiązania każdego vaultu; ewentualny rescue może wypłacać wyłącznie dowodliwy surplus, z timelockiem i eventem. Alternatywnie jawnie udokumentować nieodzyskiwalność jako celową politykę.

### L-06 — „Model referencyjny” nie jest differential modelem i nie wykrywa najważniejszego błędu czasu

`core` jest wykluczony z workspace (`Cargo.toml:1-4`), ma zero zależności i duplikuje matematykę zamiast używać `anl-math`. Nie istnieje harness podający obu implementacjom te same losowe sekwencje. Integracja wylicza część oczekiwanych wyników tą samą funkcją, której używa program, co tworzy kołowy oracle.

**Naprawa:** pełny stanowy model pozycji z czasem/epochami oraz property/state-machine fuzzing porównujący model z programem. Kluczowy inwariant: funding po `end_ts` nigdy nie zwiększa należności zakończonej pozycji.

### L-07 — Pola wersji/statusu i część constraints nie pełnią deklarowanej funkcji

`version` jest zapisywane, ale nigdy walidowane; `PoolStatus::Paused/Closed` istnieje bez instrukcji administracyjnej; kilka błędów jest martwych. Vaulty są bezpiecznie związane stałymi PDA, ale dla defense-in-depth warto dodać jawne constraints mint/authority/token program również przy późniejszych użyciach. Zmniejszy to ryzyko regresji podczas migracji.

---

## 7. Wyniki pozytywne

### I-01 — PDA, signerzy i wiązanie właścicieli

Nie znaleziono praktycznej podmiany kont, złych seeds, braku wymaganego signera ani dowolnego CPI. Pozycje są związane z ownerem i indeksem; profil tylko zwiększa `next_position_index`, a zamknięcie pozycji uniemożliwia double-claim.

### I-02 — Inwarianty principalu i rezerw ANL

Przy bezpiecznym mincie:

- stake zapisuje rzeczywisty przyrost Principal Vault;
- principal i reward mają oddzielne vaulty;
- `stake` rezerwuje nagrodę wyłącznie przy pokryciu;
- `claim` zmniejsza rezerwację dokładnie o wypłatę;
- `unstake_early` zwalnia rezerwację bez odpływu Reward Vault.

Nie znaleziono ścieżki podwójnej wypłaty ani księgowego tworzenia ANL.

### I-03 — Arytmetyka

Program używa `u128`, `checked_*`, kontrolowanych konwersji do `u64` i zaokrągleń w dół. `overflow-checks = true` jest w release. Nie znaleziono praktycznego overflow/underflow ani dzielenia przez zero w dostępnych ścieżkach. L-04 dotyczy utraty dystrybucyjnego dustu, nie tworzenia tokenów.

### I-04 — Pause nie więzi użytkownika

Globalna pauza blokuje nowe stake, ale nie `claim`, `unstake_early` ani permissionless `settle_expired`. Jest to dobry wzorzec. Gwarancja wyjścia nadal zależy jednak od bezpiecznych mintów opisanych w H-03.

---

## 8. Testy i kontrole wykonane podczas audytu

| Kontrola | Wynik |
|---|---|
| `cargo +1.89.0 test -p anl-math` | 23/23 pass |
| `cargo +1.89.0 test -p anl-math --features test-periods` | 24/24 pass |
| `cargo +1.89.0 test` w `core` | 34/34 pass |
| integracja programu z `test-periods` | 3/3 pass |
| dodatkowa integracja wariantu produkcyjnego | 3/3 pass |
| `cargo check` obu wariantów | pass z ostrzeżeniami |
| `anchor build` | **fail** — lockfile v4 nieobsługiwany przez SBF toolchain |
| `cargo audit` | **fail** — 8 vulnerabilities, 16 warnings |
| `clippy -D warnings` programu | **fail** — głównie stare cfg makr Anchor + ambiguous glob re-export |
| `cargo fmt --all -- --check` | **fail** — formatowanie workspace i `core` nie jest czyste |
| statyczny skan sekretów | brak trafień |

Zielone testy nie obejmują H-01/H-02/H-03. W szczególności brakuje:

- funding po `end_ts` przed settle;
- dwóch pozycji jednego użytkownika odzyskujących forfeiture;
- Token-2022 z transfer fee, freeze, permanent delegate i transfer hook;
- różnych decimals;
- opóźnionych, zdublowanych i pominiętych funding epochs;
- dużego `total_shares/PRECISION` i `delta_index == 0`;
- fuzzingu stanowego permutacji stake/fund/settle/forfeit/claim;
- testów dokładnych kodów błędów — wiele przypadków sprawdza wyłącznie `is_err()`;
- testu rzeczywistego produkcyjnego pliku SBF.

Testowy adapter `integration.rs:24-35` używa `unsafe transmute`; znajduje się wyłącznie w kodzie testowym. W kodzie produkcyjnym nie znaleziono `unsafe`, `unwrap`, `expect`, `panic!`, `todo!` ani `unimplemented!`.

---

## 9. CI, release i supply-chain hardening

Poza B-01…B-05 zalecane są:

- pełne SHA zamiast ruchomych tagów GitHub Actions;
- `permissions: contents: read`;
- `--locked` we wszystkich poleceniach CI;
- `rust-toolchain.toml` i przypięte X1/Solana platform-tools;
- joby `fmt`, `clippy`, `cargo audit`/OSV, produkcyjna integracja i SBF build;
- podpisany hash/SBOM/provenance artefaktu;
- `SECURITY.md`, CODEOWNERS, skaner sekretów i dependency bot.

README deklaruje Rust `≥1.80`, ale domyślny Cargo 1.75 nie czyta lockfile v4, a Cargo 1.80 nie kompiluje części aktualnie zablokowanych transitive crates wymagających edition 2024. Rust 1.89 działa dla testów natywnych. Należy podać jedną rzeczywiście wspieraną, przypiętą wersję zamiast otwartego minimum.

`Anchor.toml` zawiera skrypt Yarn/TS, mimo że repo nie ma `package.json`, `tsconfig.json` ani `tests/**/*.spec.ts`. To dodatkowy sygnał, że deklarowana procedura release nie została odtworzona end-to-end.

---

## 10. Priorytet napraw

### P0 — przed jakimkolwiek pilotażem z realną wartością

1. Przebudować mechanizm XNT tak, aby:
   - forfeiture nie było sybilowalnie odzyskiwalne;
   - entitlement kończył się według checkpointu `end_ts`, a nie momentu settle;
   - funding miał jawny, monotoniczny epoch/cutoff.
2. Egzekwować bezpieczny profil mintów oraz zweryfikować rzeczywiste mint/fee/freeze/delegate/hook authorities.
3. Zabezpieczyć `initialize` i dodać bezpieczną rotację authority.
4. Ustawić finalny Program ID, naprawić SBF toolchain i zbudować odtwarzalny produkcyjny artefakt bez `test-periods`.
5. Dodać testy regresyjne dla wszystkich H-01…H-04.

### P1 — przed mainnetem

1. Rozstrzygnąć M-01/M-03 na poziomie tokenomiki.
2. Wdrożyć multisig/timelock i procedury upgrade/incident response.
3. Zaktualizować i odchudzić dependency graph; zamknąć lub formalnie zaakceptować każdy advisory.
4. Dodać stanowy fuzzing i rzeczywiste differential testing.
5. Przeprowadzić testy na X1 testnet oraz niezależny audyt zewnętrzny poprawionej wersji.

### P2 — hardening

Zamknąć L-01…L-07, dodać monitoring inwariantów vaultów, eventy administracyjne i dokumentację wszystkich ról zaufanych.

---

## 11. Kryteria zgody na wdrożenie

Rekomenduję zgodę dopiero, gdy jednocześnie:

- wszystkie H-01…H-04 są naprawione i mają testy regresyjne;
- nie ma otwartego zgłoszenia RustSec bez udokumentowanej analizy osiągalności;
- produkcyjny SBF buduje się w czystym CI z `--locked`, ma zapisany hash i jest testowany jako `.so`;
- Program ID, minty, wszystkie token authorities, GlobalConfig authority i upgrade authority są udokumentowane i zweryfikowane on-chain;
- pipeline dowodzi, że artefakt nie zawiera `test-periods`;
- testnet przejdzie pełny cykl, testy awarii bota i co najmniej stanowy fuzzing sekwencji;
- poprawiony commit przejdzie niezależny audyt człowieka.

---

## 12. Werdykt końcowy

ANL Protocol ma dobre fundamenty implementacyjne: poprawne podstawowe constraints Anchora, dyscyplinę arytmetyczną, separację vaultów i sensowny model rezerwacji ANL. Najważniejsze ryzyka leżą jednak w logice ekonomicznej XNT i w warstwie deployment/custody. H-01 i H-02 pokazują, że obecny model nie realizuje deklarowanego „all-or-nothing” oraz cutoffu `end_ts`; H-03/H-04 sprawiają, że poprawność samego kodu nie wystarcza do ochrony środków.

**Rekomendacja OpenAI: NO-GO dla mainnetu i dla pilotażu z realną wartością do czasu zamknięcia P0.** Po poprawkach wymagany jest re-audyt, ponieważ proponowana zmiana księgowości XNT będzie zmianą architektoniczną, nie kosmetycznym patchem.

---

*Audyt AI wykonany na commicie `cf7692b` w dniu 2026-07-18. Wyniki odnoszą się wyłącznie do dostarczonego repozytorium i opisanych narzędzi; nie stanowią gwarancji braku innych podatności.*
