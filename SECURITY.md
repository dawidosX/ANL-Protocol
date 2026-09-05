# Security & Bug Bounty — ANL Staking Protocol (X1 testnet)

**Status kodu:** audit-freeze `v1.0-testnet-freeze` — `src_tree 4c2256398137bb417a1b769316137852d14ec4d5`, program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`, binarka `87b431d4…30a3`, slot 185899744.
**Audyty:** 7 rund (2026-08/09), czterech niezależnych audytorów — raporty w `docs/audits/`. Trzy potwierdzenia freeze (9 / 9 / 9,3 z 10).

Nagradzamy znalezienie błędów w **zamrożonym kodzie**, który stanie się bazą mainnetu. Kto znajdzie coś teraz — pomaga naprawić, zanim będzie na to za późno.

---

## 1. Nagrody

| Waga | Nagroda | Co się kwalifikuje |
|---|---|---|
| **Critical** | **1 000 000 ANL** | wyprowadzenie tokenów z któregokolwiek skarbca (principal / reward / XNT / CAPY) bez uprawnienia; wypłata ponad należność; podwójny claim; przejęcie lub nadpisanie cudzej pozycji; przejęcie `authority` |
| **High** | 250 000 ANL | trwała blokada środków użytkownika lub całego stakingu bez użycia klucza authority (liveness); obejście pinu `initialize`, limitu 200M, cooldownu lub blokady Genesis |
| **Medium** | 50 000 ANL | zaniżenie/zawyżenie należności zależne od kolejności transakcji; nowy wektor griefingu z realnym kosztem dla innych użytkowników; desynchronizacja księgi (rezerwacje, indeksy, checkpointy) |
| **Low** | 10 000 ANL | inne błędy z realnym, odtwarzalnym skutkiem on-chain (w tym niespójne kody błędów, martwe ścieżki umożliwiające nadużycie) |

Wagę ustala zespół wspólnie z jednym z niezależnych audytorów protokołu. Pierwsze poprawne zgłoszenie danego błędu otrzymuje nagrodę; duplikaty nie. Wypłata po naprawie i re-audycie (nie po zgłoszeniu).

## 2. Zakres

**W zakresie:** program `anl_staking` na X1 testnet (`programs/anl_staking/`, `crates/anl-math/`) na `src_tree 4c225639…`. Kod źródłowy, testy i harness: to repozytorium (`programs/anl_staking/tests/integration.rs`, `Env`).

**Poza zakresem:** frontend `website/`, publiczny RPC X1 (limity, dostępność), infrastruktura hostingu, klucze i procedury operacyjne (jeden hot key na testnecie jest znany — F-02), tokeny ANL/XNT/CAPY same w sobie, inżynieria społeczna.

## 3. Znane i świadomie zaakceptowane (NIE kwalifikują się)

Udokumentowane w `docs/CHANGES-AFTER-ROUND{4,5,6,7}.md` i raportach audytu:

- okna Genesis (`claim_genesis_window`) bez rolla doby — samokorekta w kolejnym oknie / finalnym claim
- dust z zaokrągleń (floor) pozostający w skarbcach; brak instrukcji `sweep`
- udział pozycji wygasłej (orphan) rozdzielany stakerom żywym **w chwili** `settle_expired` — zależność od momentu rozliczenia jest inherentna (SLA bota)
- pusta pula ⇒ 100% fundingu XNT dla drugiej puli (M-03)
- pozycje sprzed 4.09.2026 ze starą formułą `end_epoch` (grandfathering, tylko testnet)
- pauza dotyczy wyłącznie `stake` (design: wyjście zawsze działa)
- wejście w środku doby zalicza tę dobę, wyjście w środku doby jej nie zalicza (pozycja na N dni = dokładnie N koszyków)
- Genesis do 3650 dni w oknie 1 rezerwuje do 200% kapitału (kapitał realnie zablokowany)
- brak `sweep` nadmiaru ANL ponad 200M w reward vault (operacyjne)
- `DayNotClosed` — martwy wariant błędu (stabilność kodów)
- advisory RustSec dopuszczone w `.cargo/audit.toml` z uzasadnieniem (dev-deps / host, poza artefaktem SBF)

Jeśli uważasz, że któryś z powyższych jest jednak **exploitowalny** (np. daje kradzież lub trwałą blokadę) — zgłoś z sekwencją; wtedy się kwalifikuje.

## 4. Wymagany dowód

Zgłoszenie musi zawierać **odtwarzalny PoC**: test w harnessie `Env` (preferowane — `cargo test -p anl_staking --features test-periods --test integration`) **albo** sekwencję transakcji na X1 testnet z sygnaturami. Opis: co robi napastnik, jaki jest skutek finansowy (ile, z którego skarbca, czyje środki), jakie założenia. "Wydaje mi się, że…" bez reprodukcji nie kwalifikuje się.

## 5. Zasady odpowiedzialnego ujawniania

- Zgłoszenie **prywatnie**: `security@anl-protocol.com` (lub Telegram: @… — wpisać). Nie publikuj przed naprawą.
- Odpowiadamy w 72 h; ocena wagi do 7 dni; naprawa i re-audyt do 30 dni; ujawnienie publiczne po naprawie, nie później niż 90 dni od zgłoszenia — z podziękowaniem (za zgodą).
- Nie testuj na cudzych pozycjach ani środkach na testnecie inaczej niż w sposób, który nie powoduje trwałej szkody; nie DoS-uj publicznego RPC.
- Nagroda w ANL, wypłata na adres zgłaszającego po naprawie. Możliwa wypłata równowartości w XNT — do ustalenia przy zgłoszeniu.

## 6. Ogłoszenie (do strony / X / Discord X1)

> **ANL Staking Protocol — bug bounty do 1 000 000 ANL.** Kod stakingu na X1 testnet przeszedł 7 rund audytu i został zamrożony (`src_tree 4c225639…`). Zanim trafi na mainnet, płacimy za znalezienie w nim błędów: Critical 1 000 000 ANL · High 250 000 · Medium 50 000 · Low 10 000. Zakres, wykluczenia i zasady: `SECURITY.md` w repo `github.com/dawidosX/ANL-Protocol`. Zgłoszenia prywatnie: security@anl-protocol.com. PoC jako test w naszym harnessie mile widziany.

---
*Wersja 1.0 — 2026-09-05. Zmiany zakresu/nagród ogłaszane w tym pliku z datą.*
