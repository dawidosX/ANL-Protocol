# Security & Bug Bounty — ANL Staking Protocol (X1 testnet)

**PL** | [EN](SECURITY.md)

**Status kodu (aktualny cel bounty):** `v1.3.2-testnet-freeze` — `src_tree 32e1e6f1234314aebe785961cfb4d371428c656a`, program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`, binarka sha256 `cbc34c1b6ecea53b0bcc746657e8c9ea1ff984bb55ba9c1a826b6bc7b6c278e4` (przycięta do `so_size` 714 960 B), slot 186827491, tag `v1.3.2-testnet-freeze`. Poprzednie freeze: `v1.0-testnet-freeze` (`4c225639…`, slot 185899744), `v1.1-testnet-freeze` (`7ab2a745…`, slot 185933070), `v1.3.1-testnet-freeze` (`e82b34b3…`, slot 186156990) — naprawione findingi w `docs/BOUNTY-LEDGER.md`.
**Audyty:** 11 rund (2026-07/09), niezależni audytorzy — raporty w `docs/audits/`. Potwierdzenia v1.3.2 (R11): DRAINABLE: NO ×3, deploy TAK ×3; v1.3.1 (R10 / R10.1): 9,5 / 9,3 / 9,0 z 10.

Nagradzamy znalezienie błędów w **zamrożonym kodzie**, który stanie się bazą mainnetu. Kto znajdzie coś teraz — pomaga naprawić, zanim będzie na to za późno.
**Rejestr zgłoszeń (publiczny, bez danych osobowych):** [`docs/BOUNTY-LEDGER.md`](docs/BOUNTY-LEDGER.md).


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

**W zakresie:** program `anl_staking` na X1 testnet (`programs/anl_staking/`, `crates/anl-math/`) na `src_tree 32e1e6f1…` (tag `v1.3.2-testnet-freeze`). Kod źródłowy, testy i harness: to repozytorium (`programs/anl_staking/tests/integration.rs`, `Env`).

**Freeze a wersja LIVE.** Zamrożony `src_tree` wyznacza cel bounty, ale poprawki wychodzą jako kolejne wersje (v1.1 → v1.2 → v1.3.1) i są wdrażane na testnet po re-audycie. **Zanim zgłosisz, sprawdź, co jest wdrożone na łańcuchu** — finding odtworzony na starym drzewie, a już naprawiony w wersji live, nie kwalifikuje się (patrz `docs/BOUNTY-LEDGER.md`). Wersję live identyfikują trzy rzeczy: slot deployu, sha256 binarki i `src_tree` z `release-manifest-testnet.txt` na `main` (oraz tagi `v*-testnet-freeze`):

```bash
solana program show 4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM -u https://rpc.testnet.x1.xyz   # Last Deployed In Slot
solana program dump 4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM anl.so -u https://rpc.testnet.x1.xyz
head -c <rozmiar_binarki> anl.so | shasum -a 256      # == pole sha256 w release-manifest-testnet.txt
git rev-parse <tag>:programs/anl_staking/src           # == pole src_tree w manifeście
```

**Uwaga do sha256 binarki:** `solana program dump` zwraca CAŁE konto danych programu (dopełnione zerami do rozmiaru zaalokowanego przy pierwszym deployu, np. 813 608 B), więc sha256 pełnego dumpu NIE zgadza się z manifestem. Sumę liczymy z dumpu **przyciętego do rozmiaru wdrożonej binarki** (`head -c <rozmiar>`); rozmiar to pole `so_size` w `release-manifest-testnet.txt` (= długość `target/deploy/anl_staking.so` z reprodukowalnego buildu tagu, `scripts/build-testnet.sh`, platform-tools v1.41; v1.3.2: 714 960 B, sha `cbc34c1b…`, slot 186827491). Reszta dumpu za tym rozmiarem musi być samymi zerami.

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

- Zgłoszenie **wyłącznie prywatnie**: wiadomość prywatna (DM) do admina grupy Telegram **https://t.me/ANLprotocol** (wejdź do grupy, napisz do admina prywatnie).
- **Nie publikuj błędu na grupie ani publicznie przed naprawą — zgłoszenie na grupie traktujemy jak ujawnienie i nie kwalifikuje się do nagrody.**
- Odpowiadamy w 72 h; ocena wagi do 7 dni; naprawa i re-audyt do 30 dni; ujawnienie publiczne po naprawie, nie później niż 90 dni od zgłoszenia — z podziękowaniem (za zgodą).
- Nie testuj na cudzych pozycjach ani środkach na testnecie inaczej niż w sposób, który nie powoduje trwałej szkody; nie DoS-uj publicznego RPC.
- Nagroda w ANL, wypłata na adres zgłaszającego po naprawie. Możliwa wypłata równowartości w XNT — do ustalenia przy zgłoszeniu.

## 6. Ogłoszenie (do strony / X: https://x.com/ANLProtocol / Discord X1)

> **ANL Staking Protocol — bug bounty do 1 000 000 ANL.** Kod stakingu na X1 testnet przeszedł 11 rund audytu i został zamrożony (`src_tree 32e1e6f1…`, tag `v1.3.2-testnet-freeze`). Zanim trafi na mainnet, płacimy za znalezienie w nim błędów: Critical 1 000 000 ANL · High 250 000 · Medium 50 000 · Low 10 000. Zakres, wykluczenia i zasady: `SECURITY.pl.md` (EN: `SECURITY.md`) w repo `github.com/dawidosX/ANL-Protocol`. Zgłoszenia **tylko w wiadomości prywatnej (DM) do admina grupy Telegram https://t.me/ANLprotocol** — post na grupie lub publicznie przed naprawą = ujawnienie, bez nagrody. PoC jako test w naszym harnessie mile widziany.

---
*Wersja 1.2 — 2026-09-05 (kontakt: DM Telegram, link grupy; wersja EN w `SECURITY.md`). Zmiany zakresu/nagród ogłaszane w tym pliku z datą.*
