# ANL Staking Protocol — analiza ogólna projektu

> **Data analizy:** 2026-07-18 · **Audytowany commit:** `cf7692b` (main) · **Autor:** analiza automatyczna Claude Code (model Claude Fable 5), zlecona przez xenkonieczny · **Zakres:** analiza ogólna — architektura, jakość, ryzyka (to NIE jest formalny audyt bezpieczeństwa)

---

## 1. Czym jest ten projekt

**ANL Staking Protocol** to smart contract (program on-chain) napisany w **Rust + Anchor 0.29**, implementujący protokół stakingu tokena **ANL** na sieci **X1 Network (x1.xyz)** — forku Solany. Kod realizuje White Paper v1.0 (PDF-y PL/EN w `docs/`).

Model w skrócie:

- Użytkownik stakuje ANL na **zadeklarowany przez siebie okres 7–3650 dni** w jednej z dwóch pul:
  - **Genesis** — APY zależne od okna wejścia liczonego od publicznego startu: dni 0–30 → **20%**, dni 31–90 → **15%**, od dnia 91 → **8%**;
  - **Flexible** — zawsze **8%**.
- **Immutable APY** — stawka z chwili otwarcia pozycji obowiązuje do końca, a nagroda ANL jest znana z góry i **rezerwowana** w Reward Vault już przy stake'u (stake bez pokrycia jest odrzucany).
- Drugi strumień nagród: **dzienne XNT** (przychód walidatora) wpłacane instrukcją `fund_xnt` i dzielone **65% Genesis / 35% Flexible** do indeksów typu *acc-per-share* (PRECISION 1e12).
- Model **all-or-nothing**: `claim` po końcu okresu wypłaca principal + nagrodę ANL + naliczone XNT w jednej transakcji; `unstake_early` przed końcem zwraca cały principal, ale **wszystkie nagrody przepadają** (rezerwacja ANL zwalniana, XNT wraca do puli dystrybucji).

## 2. Metryczka

| | |
|---|---|
| Język / framework | Rust (edition 2021), Anchor 0.29, `solana-program-test` |
| Sieć docelowa | X1 Network (fork Solany); ANL = Token-2022, XNT = legacy SPL Token |
| Rozmiar kodu | ~3 150 linii Rust w 14 plikach źródłowych (repo 6,1 MB — większość to PDF-y whitepapera) |
| Historia | **6 commitów, wszystkie z 2026-07-18** — repo świeżo opublikowane, jeden autor (`dawidosX`) |
| Testy | 23 (anl-math) + 34 (core, model referencyjny) + 3 scenariusze integracyjne in-process |
| CI | GitHub Actions: testy math + core + `cargo check` obu wariantów + testy integracyjne |
| Status wg README | **Faza 1 + Faza 2 ukończone**; Faza 3 = testnet X1, fuzzing 24h+, audyt, pilot 100 000 ANL |
| Licencja | **brak pliku LICENSE** |
| Dokumentacja | README EN + PL, White Paper PDF EN + PL |

## 3. Architektura — trzy warstwy

```
crates/anl-math      → czysta matematyka (zero zależności): okna APY, nagroda za okres,
                       indeks XNT, split 65/35, zaokrąglenia floor. Feature `test-periods`.
core/                → model referencyjny protokołu (poza workspace) do differential
                       testingu — 34 testy odtwarzające przykłady z White Papera.
programs/anl_staking → właściwy program Anchor: 9 instrukcji, 4 typy kont, 3 skarbce.
```

Ten układ jest przemyślany: on-chain program i model referencyjny współdzielą **tę samą** bibliotekę matematyczną, więc testy porównawcze rzeczywiście testują to, co pójdzie na chain.

### Konta (state)

- **GlobalConfig** — authority, minty ANL/XNT, `paused`, `genesis_start_ts` (T0 okien APY), `anl_reward_reserved` (suma zarezerwowanych nagród), bumpy. Wersjonowane + 56 B rezerwy pod migracje.
- **PoolConfig** ×2 (Genesis / Flexible) — TVL, shares (1:1 z principalem), `xnt_reward_index` (u128 × 1e12), `xnt_undistributed` (zasada „pustego koszyka").
- **UserPosition** — PDA per pozycja (`owner + index`), przechowuje kwotę netto, APY, okres, `anl_reward` (znana z góry), `xnt_debt_index` (snapshot indeksu), flagę `settled`. Zamykana przy `claim`/`unstake_early` (rent wraca do usera).
- **UserProfile** — licznik pozycji użytkownika.

### Skarbce (PDA, wspólne `vault_authority`)

`principal_vault` (ANL, Token-2022) · `reward_vault` (ANL — rezerwuar 200M) · `xnt_vault` (XNT, SPL Token). Zasada niezmiennicza: principal nigdy nie miesza się z nagrodami.

### Instrukcje (9)

| Instrukcja | Kto | Rola |
|---|---|---|
| `initialize` | authority | GlobalConfig + vault authority + 3 skarbce; `start_paused` na mainnet (controlled rollout) |
| `create_pool` | authority | dokładnie 2 pule (PDA wyklucza duplikaty) |
| `pause` / `resume` | authority | hamulec bezpieczeństwa — blokuje **tylko** `stake`; ścieżki wyjścia zawsze działają |
| `stake` | user | otwarcie pozycji: *actual received*, Immutable APY, rezerwacja nagrody |
| `fund_rewards` | authority | zasilenie Reward Vault (ANL) |
| `fund_xnt` | authority | dzienny wpływ XNT; split 65/35 do indeksów |
| `settle_expired` | **każdy** (permissionless) | po `end_ts` zamraża XNT pozycji i zdejmuje shares z koszyka |
| `claim` | user | po `end_ts`: ANL + XNT + principal w 1 tx, konto pozycji zamykane |
| `unstake_early` | user | przed `end_ts`: pełny principal, nagrody przepadają |

### Operacje (bot dzienny)

Krytyczna kolejność dnia: **1) `settle_expired`** dla pozycji z `end_ts ≤ now`, **2) dopiero potem `fund_xnt`** — inaczej wygasła pozycja załapałaby się na dystrybucję dnia. `settle_expired` jest permissionless, a `claim` robi inline-settle, więc awaria bota niczego nie blokuje użytkownikom.

## 4. Mocne strony (co widać w kodzie)

1. **Dyscyplina arytmetyczna** — wszędzie `checked_*`, `overflow-checks = true` także w release, dzielenia zawsze floor („dust zostaje w vaultach, nigdy nie kreuje tokenów" — jest na to test TC-126). `core` ma `#![deny(clippy::arithmetic_side_effects)]` i `#![forbid(unsafe_code)]`.
2. **Wzorzec „actual received"** — principal i fundingi księgowane z różnicy sald *po* transferze, więc protokół jest odporny na transfer fee Token-2022 (ANL to Token-2022).
3. **Wypłacalność by-design** — rezerwacja nagrody ANL przy otwarciu pozycji (`RewardCoverageExceeded` gdy brak pokrycia) eliminuje klasyczny problem „obiecane APY bez pokrycia".
4. **Użytkownik nigdy nie jest uwięziony** — `pause` blokuje tylko wejście; `claim` / `unstake_early` / `settle_expired` działają zawsze.
5. **Solidne PDA i constraints Anchora** — seeds + bump wszędzie, walidacja mintów po adresie z GlobalConfig, rozdzielone programy tokenowe (Token-2022 vs SPL) wymuszone na poziomie kont.
6. **Testowalność** — trzy poziomy testów (60+), testy integracyjne in-process z prawdziwymi CPI i zegarem sterowanym sysvarem; identyfikatory TC-xxx mapują katalog testów ze specyfikacji; CI odpala oba warianty buildu.
7. **Przygotowanie pod migracje** — pola `version` + bufory `reserved` w każdym koncie.
8. **Feature `test-periods` rozwiązany bezpiecznie** — skrócone okna wyłącznie w compile-time, nie w `default`, z ostrzegawczym `msg!` w `initialize` w buildzie testowym.

## 5. Ryzyka i braki (uwagi ogólne — to nie jest audyt)

1. **Centralizacja operacyjna.** `authority` to pojedynczy `Pubkey` — komentarz mówi „multisig administracyjny", ale nic on-chain tego nie wymusza. Fundingi (`fund_rewards`, `fund_xnt`) i pauza zależą od jednego klucza.
2. **Brak instrukcji administracyjnych drugiej fazy życia:**
   - brak `set_authority` / przekazania uprawnień (kompromitacja lub rotacja klucza = brak ścieżki),
   - brak wypłaty nadwyżki z Reward Vault — ANL wpłacone ponad sumę rezerwacji jest **zamknięte na zawsze** (może to być celowe „proof of commitment", ale warto potwierdzić z zespołem),
   - `genesis_start_ts` nieedytowalne po `initialize` (przesunięcie go-live wymaga redeployu).
3. **Zależność ekonomiki od off-chain bota** — poprawność podziału dziennego XNT „co do dnia" zależy od dyscypliny kolejności settle→fund po stronie operatora. Zabezpieczenia (permissionless settle, inline-settle w claim) chronią użytkownika, ale nie chronią *sprawiedliwości podziału* w dniu, w którym bot się spóźni.
4. **Pokrycie testami integracyjnymi jest wciąż wąskie** — 3 scenariusze (pełny cykl 2 użytkowników, early-exit, okna+walidacje+pauza). Brak fuzzingu, brak testów na testnecie — to zaplanowana Faza 3, ale **na dziś kod nie przeszedł żadnego zewnętrznego audytu**.
5. **Placeholder Program ID** — `declare_id!("Fg6Pa…")` to domyślny ID Anchora; przed deploymentem konieczne `anchor keys sync` (README o tym wspomina).
6. **`init_if_needed` na UserProfile** — wzorzec historycznie podatny na błędy w Anchorze; użycie tutaj wygląda poprawnie (re-init nie nadpisuje licznika), ale to typowy punkt uwagi audytora.
7. **Anchor 0.29 (2023)** — nie jest to wersja bieżąca; przy dłuższym życiu projektu warto zaplanować podbicie zależności.
8. **Brak LICENSE** i brak klienta/SDK/IDL w repo — na tym etapie zrozumiałe, ale do uzupełnienia przed publicznym launchem.
9. **Świeżość repo** — cała historia to 6 commitów z jednego dnia; git nie mówi nic o realnej historii rozwoju (kod mógł powstawać poza repo).

## 6. Status i roadmap

- **Zrobione (wg README, potwierdzone w kodzie):** pełna implementacja Fazy 1 (initialize, create_pool, pause/resume) i Fazy 2 (stake, fund_rewards/fund_xnt, settle_expired, claim, unstake_early) z testami.
- **Faza 3 (następna):** pełny cykl na testnecie X1, fuzzing 24h+, audyt AI, pilot 100 000 ANL (start w pauzie), dashboard.
- Ostatni commit wspomina o **potwierdzonym on-chain self-stake'u 15K** walidatora — projekt ma już jakiś ślad operacyjny na sieci.

## 7. Ocena ogólna

Jak na repo opublikowane jednego dnia, poziom inżynierii jest **wyraźnie ponadprzeciętny dla wczesnych projektów DeFi**: architektura trójwarstwowa z modelem referencyjnym, konsekwentna arytmetyka checked/floor, rezerwacja nagród gwarantująca pokrycie, poprawne wzorce Anchora i sensowny CI. Główne ryzyka nie leżą w tym, co jest w kodzie, lecz w tym, czego **jeszcze nie ma**: audytu zewnętrznego, mechanizmów rotacji authority, odzysku nadwyżek z vaultów oraz szerszego pokrycia testami scenariuszy brzegowych. Przed powierzeniem protokołowi realnych środków kluczowe będzie domknięcie Fazy 3 (testnet + fuzzing + audyt).

---

*Analiza ogólna wykonana automatycznie (Claude Code) na podstawie kodu źródłowego i README; nie zastępuje audytu bezpieczeństwa.*
