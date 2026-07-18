# ANL Staking Protocol — Smart Contract (X1 Network)

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
- **Koniec okresu**: naliczanie OBU strumieni staje na `end_ts`. `settle_expired`
  (permissionless) zamraża XNT pozycji i zdejmuje shares z koszyka.
- **`claim`** (po `end_ts`): nagroda ANL + naliczone XNT + principal w **jednej
  transakcji**; konto pozycji zamykane (rent wraca do właściciela).
- **`unstake_early`** (przed `end_ts`): principal wraca w całości; **całość nagród
  przepada** — rezerwacja ANL zwolniona (tokeny nie opuściły Reward Vault),
  naliczone XNT wracają do `xnt_undistributed` koszyka.

## Stan: Faza 1 + Faza 2 ✅ (do testów integracyjnych)

| Moduł | Zakres | Testy |
|---|---|---|
| `crates/anl-math` | okna APY (31/91), nagrody okresowe, indeks XNT, podział 65/35, dust | **23/23** |
| `core/` | model referencyjny: okres deklarowany, `settle`, `forfeit`, przykłady z WP | **34/34** |
| `initialize` | GlobalConfig + VaultAuthority + 3 skarbce; ANL=Token-2022, XNT=SPL | TC-001…006 |
| `create_pool` | dokładnie 2 pule, udziały XNT 65/35 | TC-010…016 |
| `pause` / `resume` | hamulec awaryjny | TC-100…105 |
| `stake` | actual received, Immutable APY, okres 7..=3650, rezerwacja nagrody | ✅ integ. |
| `fund_rewards` / `fund_xnt` | depozyty NETTO; dzienny split 65/35 do indeksów | ✅ integ. |
| `settle_expired` | permissionless; mrozi XNT, zdejmuje shares po `end_ts` | ✅ integ. |
| `claim` | ANL+XNT+principal w 1 tx, zwolnienie rezerwacji, close | ✅ integ. |
| `unstake_early` | principal w całości; przepadek ANL (rezerwacja) i XNT (undistributed) | ✅ integ. |

## Operacyjnie (bot dzienny) — WAŻNE

Kolejność każdego dnia: **1) `settle_expired` dla pozycji z `end_ts` ≤ teraz,
2) dopiero potem `fund_xnt`.** Settle przed fundingiem gwarantuje, że pozycja
po terminie nie uczestniczy w dziennej dystrybucji (WP §8 co do dnia).
`settle_expired` jest permissionless — awaria bota niczego nie blokuje,
a `claim` robi settle inline.

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
anchor build -- --features test-periods     # artefakt TESTNETOWY
```

**Nigdy nie wdrażać buildu `test-periods` na mainnet.** Zabezpieczenia:
feature nie jest w `default`, a `initialize` loguje ostrzegawczy `msg!`
w każdym buildzie testowym — widoczny w logach pierwszej transakcji.

## Budowanie

```bash
cargo test -p anl-math          # matematyka (23)
cd core && cargo test           # model referencyjny (34)
anchor build                    # artefakt SBF
anchor keys sync                # właściwy Program ID
```

Toolchain: **Rust ≥ 1.80** (weryfikowane na 1.89). `Cargo.lock` wygenerowany
na 1.89 — stare piny pod rustc 1.75 zdjęte.

## Testy integracyjne (solana-program-test)

```bash
cargo test -p anl_staking --features test-periods --test integration   # 3/3
```

In-process, z prawdziwymi CPI do Token-2022 i SPL Token, zegar sterowany sysvarem.
Scenariusze: pełny cykl 2 użytkowników z dziennym XNT (proporcje 2:1, settle mrozi,
claim = ANL+XNT+principal w 1 tx, konto zamykane) · zerwanie (principal 100%,
przepadek do puli koszyka, redystrybucja kolejnym fundingiem, guardy PeriodNotEnded /
PeriodAlreadyEnded) · okna Immutable APY + pokrycie nagród + walidacje + pauza
(stake blokowany, claim działa). Suite używa stałych z anl-math — działa w obu
wariantach builda.

## Faza 3 (następna)

Testy integracyjne pełnego cyklu na testnecie X1 (Volume 10B), fuzzing 24h+,
audyt AI, pilotaż 100 000 ANL (start w pauzie), dashboard.
