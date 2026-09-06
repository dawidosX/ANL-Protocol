# AUDIT NOTE — runda 9.1 (domkniecia po R9, przed deployem v1.2)

**Data:** 2026-09-06 · **Branch:** `fix/v1.2-r9.1` (baza: `main` @ `a0af512` = v1.2 po merge PR #12, NIE wdrozone)
**Wdrozone na X1 testnet:** `src_tree 7ab2a745…` (v1.1-testnet-freeze, slot 185933070, `.so 1d449591…`)

Werdykty R9 (trojka, kod `src_tree ceed4876…`): v1.2 bezpieczne — 3× brak drenazu; XNT-01 / EDGE / sweep / horyzont PASS.
Trzy domkniecia zlecone przed deployem:

| # | Zrodlo | Waga | Temat | Status |
|---|---|---|---|---|
| 1 | B | Medium | legacy bufor `xnt_undistributed` (stan sprzed v1.2) | **odczyt on-chain = 0 na obu pulach → migracja zbedna; test + dokumentacja** |
| 2 | C | Low | off-by-one horyzontu fundingu (`<=` → `<`) | **naprawione, test granicy co do sekundy** |
| 3 | C (I-02) | — | property-test: skarbiec ≥ roszczenia po kazdym kroku (z sweep) | **dodane (state + harness)** |

---

## 1. Legacy bufor `xnt_undistributed` (B, Medium)

### 1.1 Layout `PoolConfig` (offsety od poczatku konta, LEN = 126)

| Offset | Pole | Typ |
|---|---|---|
| 0 | discriminator | [u8; 8] |
| 8 | version | u8 |
| 9 | pool_type | u8 (0 = Flexible, 1 = Genesis) |
| 10 | status | u8 |
| 11 | xnt_share_bps | u16 |
| 13 | total_staked | u64 |
| 21 | total_shares | u64 |
| 29 | xnt_reward_index | u128 |
| **45** | **xnt_undistributed** | **u64** |
| 53 | position_count | u64 |
| 61 | last_funded_epoch | u64 |
| 69 | first_funded_epoch | u64 |
| 77 | bump | u8 |
| 78 | current_day_basket | u64 |
| 86 | current_day | u64 |
| **94** | **xnt_protocol_revenue** (v1.2, wyciete z `reserved`) | **u64** |
| 102 | reserved | [u8; 24] |

### 1.2 Odczyt on-chain (X1 testnet `https://rpc.testnet.x1.xyz`, slot **186117962**, 2026-09-06 ~11:40 UTC)

Program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`; `genesis_start_ts = 1787045043` (2026-08-18 09:24:03Z).

| Pole | Flexible (pool_type 0) `GB9DLxAhpgfYxJAnUqWxst3eYbc9b45jA2d6bBAJSdkc` | Genesis (pool_type 1) `Hm2czyrXMiec6VBTNwwmMHvsLpQ1PFtXwNC81r5fx4CN` |
|---|---|---|
| xnt_share_bps | 3500 | 6500 |
| total_staked = total_shares | 19 684 511 000 000 000 (19 684 511 ANL) | 141 550 411 000 000 000 (141 550 411 ANL) |
| xnt_reward_index | 958 738 | 852 470 |
| **xnt_undistributed @45** | **0** | **0** |
| position_count | 286 | 101 |
| first_funded_epoch / last_funded_epoch | 13 / 15 | 13 / 15 |
| current_day_basket @78 / current_day @86 | 0 / 16 | 0 / 16 |
| **xnt_protocol_revenue @94** | **0** (v1.2 niewdrozone) | **0** |
| reserved @102..126 | same zera | same zera |

Wniosek: **oba bufory legacy = 0**, obie pule maja shares > 0 (nie sa i — od pierwszego fundingu — nie byly puste w momencie domkniecia doby, inaczej bufor bylby > 0 albo indeks nie bylby monotoniczny). Po wdrozeniu v1.2 pole `xnt_undistributed` nie ma juz zadnego zapisu zwiekszajacego (jedyne zapisy: `create_pool` = 0, `close_day` = 0 po uwolnieniu) — pozostaje polem martwym o wartosci 0.

### 1.3 Rekomendacja: wariant „bez migracji” (dokumentacja + test)

Odrzucone: instrukcja migracyjna legacy → revenue (nowa sciezka authority, nowy kod bledu, nowy test negatywny — po to, zeby przeniesc 0). Zbedna powierzchnia ataku i zmiana `src_tree` bez efektu on-chain.

Przyjete:
- **Dokumentacja** (ta notatka + komentarz w `state/mod.rs`): `xnt_undistributed` = pole legacy, on-chain 0, po v1.2 tylko malejace (uwolnienie przy pierwszym domknieciu z shares > 0).
- **Test jednostkowy** `test_r9_legacy_bufor_przy_shares_uwalnia_sie_raz_bez_podwojenia`: legacy 7 + koszyk 5 przy shares > 0 → jeden podzial (12), kolejna doba tylko 3 (bez podwojenia), legacy przy shares == 0 czeka (koszyk → revenue), orphan nie dotyka legacy/revenue.
- **Test integracyjny** `regresja_r9_legacy_bufor_on_chain_uwalnia_sie_raz`: wstrzykniecie legacy = 1 000 pod offset 45 (jak stan on-chain sprzed v1.2, pokryte skarbcem), staker dostaje legacy + koszyk dokladnie raz, `xnt_undistributed → 0`, revenue nietkniete; oba rezimy.
- **Property** (§3) — co drugi seed startuje z legacy > 0.

Gdyby na mainnecie (nowy deploy, pule od zera) lub przed deployem v1.2 bufor jednak urosl (> 0): sciezka `close_day` z shares > 0 uwalnia go do zywych bez podwojenia (dowod: testy wyzej) — nadal bez migracji.

## 2. Off-by-one horyzontu (C, Low)

`fund_xnt`: `now <= genesis + H` → `now < genesis + H`. Okno polotwarte [T0, T0+H) = dokladnie H dob (0..H−1), spojnie z `epoch_of`. Przed fixem pierwsza sekunda doby H (T0+H) przechodzila.
Stale bez zmian (`XNT_FUNDING_HORIZON_SECS` = 3·365 d prod / 9 d test-periods; asercje w `production_constants_guard` i `test_periods_feature_uses_short_windows` poprawne — dotycza wartosci, nie porownania); dopisano semantyke w doc-komentarzu `anl-math`.
Test `regresja_r9_horyzont_granica_t0_plus_h` (nowy helper `Env::set_time`, zegar co do sekundy): T0+H−1 PASS (doba H−1), T0+H FAIL `XntFundingEnded` 6047 (nie `EpochMismatch`), T0+H+1 FAIL 6047; `total_xnt_funded` = tylko udane fundingi. Oba rezimy.

## 3. Property-test skarbca (I-02 C)

- **State** `test_r9_property_skarbiec_pokrywa_roszczenia_ze_sweep_i_legacy`: 40 seedow × 150 krokow: stake / fund / close / settle+claim (R8: minus okna) / forfeit (tylko pozycje bez okien — unstake_early = Flexible, okna = Genesis) / claim okna Genesis (cap = domknieta doba ≤ cap_epoch) / **sweep** (1..revenue). Model skarbca `vault = funded − paid − swept`. Po kazdym kroku: `vault ≥ pending_zywych(netto okien) + legacy + revenue + koszyk` oraz tozsamosc `vault == funded − paid − swept`. Co drugi seed startuje z legacy > 0.
- **Harness** `property_r9_skarbiec_xnt_pokrywa_roszczenia_losowe_sekwencje`: 3 seedy × 80 krokow na PRAWDZIWYM `xnt_vault`: stake (obie pule, MIN..MIN+2 d) / `fund_xnt` (w horyzoncie) / uplyw czasu (per rezim) / `close_day` / `settle_expired` / `claim` / `unstake_early` (po cooldownie, z domknieciem doby wg XNT-01) / `sweep_revenue`. Po kazdym kroku: `xnt_vault.amount ≥ Σ(pending zywych lub xnt_accrued rozliczonych − okna) + legacy + revenue + koszyk (obie pule)`, `vault + wyplacone + sweep == total_xnt_funded`, saldo authority == sweep. Asercja: kazdy typ operacji wystapil (w tym claim i sweep). Nowy helper `Env::last_ckpt_epoch_le` (logika bota: cap = ostatni funding ≤ end_epoch).

Uwaga modelowa (znaleziona przy pisaniu, NIE bug programu): w modelu state polaczenie „claim okna” + „forfeit” tej samej pozycji podwaja wyplate — w programie niemozliwe (okna tylko Genesis, `unstake_early` tylko Flexible); model odwzorowuje to ograniczenie.

## 4. Bramki (branch `fix/v1.2-r9.1`)

lib 15/15 · 17/17 `network-mainnet` · 16/16 `network-testnet,test-periods`; integracja **59/59** `test-periods` i **59/59** prod; core 34+2 / 30+2; anl-math 24 / 24; clippy `-D warnings` ×3 (workspace, `test-periods`, `network-mainnet`); fmt; `cargo audit` 0 podatnosci (22 dopuszczone ostrzezenia wg `audit.toml`); `Cargo.lock` nietkniety (`8b0f5d39…`).

## 5. Zmiany w `programs/` i `crates/` (nowy `src_tree` ⇒ mini-audyt przed deployem)

- `programs/anl_staking/src/instructions/fund.rs`: jedna linia logiki (`<=` → `<`) + komentarz.
- `programs/anl_staking/src/state/mod.rs`: wylacznie `#[cfg(test)]` (2 testy) — bez zmian logiki.
- `crates/anl-math/src/lib.rs`: wylacznie doc-komentarz.
- `programs/anl_staking/tests/integration.rs`: 3 testy + 2 helpery + `custom_code`.

Bez nowych wariantow bledow, bez zmian layoutu kont, bez zmian frontendu.
