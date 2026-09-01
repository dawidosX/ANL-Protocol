# ANL Staking Protocol — Smart Kontrakt (X1 Network)

[![CI](https://github.com/dawidosX/ANL-Protocol/actions/workflows/ci.yml/badge.svg)](https://github.com/dawidosX/ANL-Protocol/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Network: X1](https://img.shields.io/badge/network-X1-7A3BFF.svg)](https://x1.xyz)
[![Faza: Testnet](https://img.shields.io/badge/faza-testnet-FFB11A.svg)](https://testnet.anl-protocol.com)

Program on-chain w **Rust + Anchor**. Niekastodialny staking ANL z **potrójnym
strumieniem nagród** w sieci **X1 Network (x1.xyz)** — łańcuchu zgodnym z Solaną.

Implementacja **White Paper v1.2** — [PL](docs/whitepaper/whitepaper.html) ·
[PDF PL](docs/whitepaper/ANL-Whitepaper-v1.2-PL.pdf) ·
[PDF EN](docs/whitepaper/ANL-Whitepaper-v1.2-EN.pdf) ·
[English README](README.md)

---

## Czym jest

Uczestnik blokuje **ANL** na zadeklarowany okres (min. 7 dni) i otrzymuje
**trzy strumienie nagród**:

- **ANL** — stałe APY ze skończonej, wydzielonej puli **200 000 000 ANL**
- **XNT** — codziennie naliczany udział w realnym przychodzie dedykowanego walidatora X1
- **CAPY** — bonus z wydzielonej puli **20 000 000 CAPY**, naliczany przy odbiorze
  proporcjonalnie do wypłaconej nagrody ANL

Dwa programy: **Genesis** (podwyższone APY zależne od okna, do 20%) i
**Flexible** (stałe 8%). Oprocentowanie przypisane przy otwarciu jest
**niezmienne** przez całe życie pozycji.

## Model (WP v1.2)

- **Oba programy** (Genesis / Flexible): uczestnik deklaruje okres, **7..=3650 dni**.
- **Okna Genesis** (od startu publicznego): dni **0–30 → 20%**, **31–90 → 15%**,
  **od 91 → 8%**. Flexible: zawsze 8%. Immutable APY.
- **Nagroda ANL** rezerwowana przy stake (`GlobalConfig.anl_reward_reserved`) — stake
  bez pokrycia w Reward Vault jest odrzucany (`RewardCoverageExceeded`).
- **Dzienny XNT**: `fund_xnt` dzieli przychód walidatora **65% Genesis / 35% Flexible**,
  posuwa indeksy koszyków (acc-per-share, PRECISION 1e12). Pusty koszyk → udział czeka
  w `xnt_undistributed`.
- **Bonus CAPY**: przy `claim` liczone jest `pending_capy`
  (`nagroda_anl × dostępne_capy / pozostałe_anl`) i **rezerwowane** — wypłacane
  **osobną** instrukcją `claim_capy`. Core claim (ANL+XNT) nigdy nie blokuje się na CAPY.
- **Cap settlementu → ostatnia domknięta doba**: XNT rozliczane do **ostatniej
  domkniętej doby** (≤ `end_epoch`). Pozycja odbierana w bieżącej (trwającej) dobie
  rezygnuje z tej niepełnej doby i **odbiera od razu** — bez czekania na jej domknięcie.
- **`claim`** (po `end_ts`): nagroda ANL + naliczone XNT + kapitał w jednej tx;
  konto pozycji zamykane. CAPY zarezerwowane (odbierane osobno).
- **`unstake_early`** (Flexible, przed `end_ts`): kapitał wraca w całości; **wszystkie
  nagrody przepadają**; brak bonusu CAPY. Pozycje Genesis zablokowane do końca okresu.

## Cztery odizolowane skarbce

| Skarbiec | Zawartość | Reguła |
|---|---|---|
| Principal Vault | kapitał użytkowników | wypłaty tylko do właścicieli pozycji |
| Reward Vault | 200 000 000 ANL | wypłaty tylko jako naliczone nagrody |
| XNT Vault | przychód walidatora | dzienna dystrybucja 65/35 |
| Capy Vault | 20 000 000 CAPY | wypłaty tylko jako zarezerwowane bonusy (`claim_capy`) |

Program nigdy nie wypłaca kapitału ze skarbca nagród ani nagród ze skarbca kapitału
— inwariant zaszyty w kodzie i objęty testami.

## Budowanie

Toolchain, który produkuje działającą binarkę SBF na tym repo:

```bash
# solana-cli 2.3.11 + tools v1.53 (starsze CLI/tools padają na edition2024 / sysroot)
cargo-build-sbf --tools-version v1.53 --features network-testnet,test-periods
```

Matematyka + model referencyjny:

```bash
cargo test -p anl-math          # matematyka (24 unit + 10 property)
cd core && cargo test           # model referencyjny
```

**Nigdy nie wdrażaj buildu `test-periods` na mainnet.** Wymuszone w czasie kompilacji:
`compile_error!` odrzuca `network-mainnet`+`test-periods` oraz dowolny build dwóch
sieci naraz. Zadanie CI `release-guards` dowodzi obu przypadków przy każdym pushu.

## Build testowy — feature `test-periods`

| Parametr | Produkcja | `test-periods` |
|---|---|---|
| Min. okres pozycji | 7 dni | 1 dzień |
| Okno Genesis 1 (20%) | dni 0–30 | dni 0–2 |
| Okno Genesis 2 (15%) | dni 31–90 | dni 3–8 |
| Okno Genesis 3 (8%) | od dnia 91 | od dnia 9 |
| Okno wypłat XNT Genesis | 30 dni | 3 dni |

## Testnet — na żywo

Protokół działa na **testnecie X1**: [testnet.anl-protocol.com](https://testnet.anl-protocol.com).
Pełny cykl zweryfikowany on-chain: stake → `fund_xnt` → `claim` (ANL+XNT) → `claim_capy` (CAPY).
Statystyki live (TVL, salda skarbców, walidator) czytane wprost z łańcucha.

## Bezpieczeństwo

Protokół przeszedł **wiele rund przeglądu bezpieczeństwa**, z ustaleniami naprawionymi
i niezależnie zweryfikowanymi — pełny ślad w
**[docs/SECURITY-AUDITS.pl.md](docs/SECURITY-AUDITS.pl.md)**, raporty audytorów
zarchiwizowane w `docs/audits/`.

**Uwaga (zmiany po audycie 3, do ponownego przeglądu):** cap settlementu zmieniony na
*ostatnią domkniętą dobę*, usunięte strażniki `DayNotClosed` (cap sam ogranicza się do
domkniętej doby). To dotyka rdzenia rozliczania nagród — w kolejce do audytu rundy 4.
Zob. [docs/CHANGES-AFTER-AUDIT3.md](docs/CHANGES-AFTER-AUDIT3.md).

Stan: **faza zamkniętego testnetu; brak wdrożenia na mainnet.** Znalazłeś coś?
Otwórz prywatne security advisory na GitHubie zamiast publicznego issue.

## Struktura repozytorium

```
programs/anl_staking/   program Anchor (instrukcje, stan)
crates/anl-math/        czysta matematyka (APY, indeks XNT, podziały) + testy property
core/                   model referencyjny (okresy, settle, forfeit)
docs/whitepaper/        White Paper v1.2 (web + PDF, PL + EN)
docs/audits/            zarchiwizowane raporty audytorów
website/testnet/        front-end dApp testnet
scripts/                build-testnet / build-mainnet / audit-evidence
```

## Licencja

Na licencji [Apache License, Version 2.0](LICENSE).
