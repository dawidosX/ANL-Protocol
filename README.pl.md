# ANL Staking Protocol — Smart Contract (X1 Network)

[![CI](https://github.com/dawidosX/ANL-Protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/dawidosX/ANL-Protocol/actions/workflows/ci.yml)
[![Licencja: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)

Program on-chain w Rust + Anchor 0.29. Implementacja wg **[White Paper v1.0](docs/ANL_White_Paper_PL.pdf)**
([English White Paper](docs/ANL_White_Paper_EN.pdf) · [English README](README.md))
(model: dzienne XNT, okres deklarowany, całość-albo-nic).
Sieć docelowa: **X1 Network (x1.xyz)** — fork Solany.

## Model (WP v1.0)

- **Oba programy** (Genesis / Flexible): okres deklaruje uczestnik, **7..=3650 dni**.
- **Okna Genesis** (od publicznego startu): dni **0–30 → 20%**, **31–90 → 15%**,
  **od 91 → 8%**. Flexible: zawsze 8%. Immutable APY — stopa z chwili otwarcia.
- **Nagroda ANL** znana z góry i **rezerwowana** przy stake'u
  (`GlobalConfig.anl_reward_reserved`) — stake bez pokrycia w Reward Vault
  jest odrzucany (`RewardCoverageExceeded`, WP §11).
- **XNT dziennie**: `fund_xnt` dzieli wpływ walidatora **65% Genesis / 35% Flexible**
  i podbija indeksy koszyków (`acc-per-share`, PRECISION 1e12). Pusty koszyk →
  część czeka w `xnt_undistributed` i wchodzi przy najbliższym fundingu.
- **Koniec okresu**: nagroda ANL kończy się dokładnie na `end_ts`; XNT jest rozliczane epokowo i obejmuje pełną `end_epoch` (patrz WP §8.1). `settle_expired`
  (permissionless) zamraża XNT pozycji i zdejmuje shares z koszyka.
- **`claim`** (po `end_ts`): nagroda ANL + naliczone XNT + principal w **jednej
  transakcji**; konto pozycji zamykane (rent wraca do właściciela).
- **`unstake_early`** (przed `end_ts`): principal wraca w całości; **całość nagród
  przepada** — rezerwacja ANL zwolniona (tokeny nie opuściły Reward Vault),
  naliczone XNT wracają do `xnt_undistributed` koszyka.

## Stan: Faza 1 + Faza 2 ✅ (do testów integracyjnych)

| Moduł | Zakres | Testy |
|---|---|---|
| `crates/anl-math` | okna APY (31/91), nagrody okresowe, indeks XNT, podział 65/35, dust | **34/34** |
| `core/` | model referencyjny: okres deklarowany, `settle`, `forfeit`, przykłady z WP | **34/34** |
| `initialize` | GlobalConfig + VaultAuthority + 3 skarbce; ANL=Token-2022, XNT=SPL | TC-001…006 |
| `create_pool` | dokładnie 2 pule, udziały XNT 65/35 | TC-010…016 |
| `pause` / `resume` | hamulec awaryjny | TC-100…105 |
| `stake` | actual received, Immutable APY, okres 7..=3650, rezerwacja nagrody | ✅ integ. |
| `fund_rewards` / `fund_xnt(amount, epoch)` | depozyty NETTO; split 65/35; checkpoint epoki | ✅ integ. |
| `settle_expired` | permissionless; XNT z checkpointu ≤ `end_epoch` (audyt #1 ✅) | ✅ integ. |
| `claim` | ANL+XNT+principal w 1 tx, zwolnienie rezerwacji, close | ✅ integ. |
| `unstake_early` | principal w całości; przepadek ANL (rezerwacja) i XNT (undistributed) | ✅ integ. |

## Operacyjnie (bot dzienny) — WAŻNE

Kolejność zalecana każdego dnia: **1) `settle_expired` dla pozycji z `end_ts` ≤ teraz,
2) dopiero potem `fund_xnt`.** Ta kolejność MINIMALIZUJE rozwodnienie indeksu przez
pozycje po terminie, ale **poprawność wypłaty nie zależy od niej**: nawet gdy funding
wyprzedzi settle, rozliczenie używa historycznego checkpointu ≤ `end_epoch`, więc
pozycja nigdy nie dostanie XNT z epoki > `end_epoch` (audyt #1/#3). Nierozliczona
pozycja po terminie wciąż trzyma shares i rozcieńcza bieżący indeks do czasu settle —
nadwyżka zostaje w skarbcu (inwariant wypłacalności). `settle_expired` jest
permissionless (awaria bota niczego nie blokuje), a `claim` robi settle inline.

Pauza (`pause`) blokuje `stake`; ścieżki wyjścia (`claim`, `unstake_early`,
`settle_expired`) działają zawsze — użytkownik nigdy nie jest uwięziony.

## Build testowy — feature `test-periods`

Do fazy testów (testnet X1) parametry czasowe są skracane w **compile-time**:

| Parametr | Produkcja | `test-periods` |
|---|---|---|
| Min. okres pozycji | 7 dni | **1 dzień** |
| Okno 1 Genesis (20%) | dni 0–30 | **dni 0–2** |
| Okno 2 Genesis (15%) | dni 31–90 | **dni 3–8** |
| Okno 3 Genesis (8%) | od dnia 91 | **od dnia 9** |

```bash
cargo test -p anl-math --features test-periods
scripts/build-testnet.sh        # JEDYNA droga do artefaktu TESTNETOWEGO
```

**Nigdy nie wdrażać buildu `test-periods` na mainnet.** Egzekwuje to
compile-time, nie procedura: crate definiuje feature'y sieci
`network-mainnet` / `network-testnet`, a `compile_error!` odrzuca zarówno
`network-mainnet`+`test-periods`, jak i build z dwiema sieciami naraz.
Job release-guards w CI dowodzi obu przypadków negatywnych przy każdym
pushu. Artefakty release powstają **wyłącznie** przez
`scripts/build-mainnet.sh` i `scripts/build-testnet.sh` — oba odmawiają
pracy na brudnym drzewie (`git status --porcelain`) i zapisują manifest
(HEAD, features, sha256 binarki, wersja rustc). Deploy artefaktu
zbudowanego lokalnie z pominięciem tych skryptów jest zabroniony.
Dodatkowo feature nie jest w `default`, a `initialize` loguje
ostrzegawczy `msg!` w każdym buildzie testowym.

## Budowanie

```bash
cargo test -p anl-math          # matematyka (34: 24 jednostkowe + 10 property)
cd core && cargo test           # model referencyjny (36: 34 jednostkowe + 2 property)
scripts/build-testnet.sh        # artefakt release TESTNET + manifest
scripts/build-mainnet.sh        # artefakt release MAINNET + manifest
anchor keys sync                # właściwy Program ID (faza deployu)
```

**Polityka release:** artefakty do wdrożenia pochodzą **wyłącznie** z dwóch
skryptów `scripts/build-*.sh` (bramka czystego drzewa + manifest). Zwykły
`anchor build` jest w porządku do lokalnego developmentu, ale jego wynik
nigdy nie trafia na sieć. Pełny bieg dowodowy (fmt, clippy `-D warnings`,
wszystkie zestawy testów, negatywne buildy strażników, audit/deny) to
`scripts/audit-evidence.sh` — fail-closed, powiązany z `git rev-parse HEAD`.

Toolchain: **Rust ≥ 1.80** (weryfikowane na 1.89). `Cargo.lock` wygenerowany
na 1.89 — stare piny pod rustc 1.75 zdjęte.


> **Semantyka XNT — epoka, nie sekunda.** Strumień XNT rozlicza się w jednostce epoki dziennej: `end_epoch = epoch_of(end_ts − 1)`. Pozycja dostaje XNT za każdą epokę, w której choć sekunda jej okresu jest aktywna (łącznie z całą epoką końca); funding epoki `> end_epoch` nigdy nie zwiększa wypłaty. Nagroda ANL liczy się do dokładnego `end_ts`. To model zamierzony (WP §8.1) — nie rozjazd dok↔kod.

## Testy integracyjne (solana-program-test)

```bash
cargo test -p anl_staking --features test-periods --test integration   # 4/4
```

In-process, z prawdziwymi CPI do Token-2022 i SPL Token, zegar sterowany sysvarem.
Scenariusze: pełny cykl 2 użytkowników z dziennym XNT (proporcje 2:1, settle mrozi,
claim = ANL+XNT+principal w 1 tx, konto zamykane) · zerwanie (principal 100%,
przepadek do puli koszyka, redystrybucja kolejnym fundingiem, guardy PeriodNotEnded /
PeriodAlreadyEnded) · okna Immutable APY + pokrycie nagród + walidacje + pauza
(stake blokowany, claim działa). Suite używa stałych z anl-math — działa w obu
wariantach builda.


## Bezpieczeństwo — audyt #1 (18.07.2026)

Kontrakt przeszedł wstępny audyt bezpieczeństwa. Status ustaleń i wdrożone poprawki:
**[docs/AUDIT-RESPONSE.md](docs/AUDIT-RESPONSE.md)**. W skrócie: rola **operatora**
(gorący klucz wyłącznie do fundingu, `set_operator` z multisig), bramka rozszerzeń
Token-2022 minta ANL w `initialize`, kontrola `version` we wszystkich instrukcjach,
pełne constraints skarbców, twardy test stałych produkcyjnych. **Otwarte: #1**
(koszyki wygaśnięć XNT) — do zamknięcia przed jakimkolwiek wdrożeniem immutable.

## Faza 3 (następna)

Testy integracyjne pełnego cyklu na testnecie X1 (Volume 10B), fuzzing 24h+,
audyt AI, pilotaż 100 000 ANL (start w pauzie), dashboard.

## Bezpieczeństwo

Protokół przeszedł **cztery rundy przeglądu bezpieczeństwa**, a każde
ustalenie zostało naprawione i niezależnie zweryfikowane — pełna ścieżka
(ustalenia, naprawy, dowody plik:linia, werdykty i Definition of Done dla
immutable mainnetu) znajduje się w
**[docs/SECURITY-AUDITS.pl.md](docs/SECURITY-AUDITS.pl.md)**, a oryginalne
raporty recenzentów w `docs/audits/`. Status: faza zamkniętego testnetu;
program **nie jest jeszcze wdrożony** na żadnej publicznej sieci.
Znalazłeś coś? Zgłoś przez prywatne security advisory na GitHubie,
nie przez publiczne issue.

## Licencja

Na licencji [Apache License, Version 2.0](LICENSE).
