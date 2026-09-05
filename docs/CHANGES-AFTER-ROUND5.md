# Zmiany po rundzie 5 audytu (baza: `87f358b` → fix `e16c634`)

**Data:** 2026-09-05 · **Raporty rundy 5:** Kimi (TAK), B (TAK), C (NIE — M-01, M-02, P-01)

Wspólny werdykt trójki: naprawa H-01/M-01 z rundy 4 jest szczelna, inwariant
determinizmu przyjęty bez kontrprzykładu. Rozbieżność dotyczyła wyłącznie
**wtórnej dystrybucji orphana** (C: Medium; Kimi/B: residual). Zdecydowaliśmy
się naprawić, bo koszt jest mały, a wektor na immutable mainnecie realny.

---

## M-01 (C) — historyczny orphan trafiał do późniejszych stakerów — NAPRAWIONE

**Problem (potwierdzony, C ma rację):** `settle_position_at` przenosił orphan
(udział wygasłej pozycji w dobie po jej `end_epoch`) do bezczasowego bufora
`xnt_undistributed`, który następny `close_day` dzielił według **ówczesnych**
shares — także stakerów, którzy weszli po tamtej dobie. Przewidywalny bufor
(publiczny na stronie) = zachęta do wejścia kapitałem "na jeden dzień" przed
`close_day`. To samo dotyczyło przepadku przy `unstake_early` (Q10).

**Fix (`state/mod.rs`):** nowa metoda `PoolConfig::redistribute_to_live(amount)`
— podnosi `xnt_reward_index` o `amount / total_shares` **natychmiast**, dla
shares obecnych w chwili wywołania (po zdjęciu wychodzącego). Do
`xnt_undistributed` trafia wyłącznie, gdy `total_shares == 0` (nie ma komu
dać — zasada M-03). Użyta w `settle_position_at` (orphan) i `forfeit_position`
(przepadek). Bez zmian w instrukcjach, kontach ani kliencie.

**Własności:** odbiorcy = żywi w momencie settle (przy bocie `close_day →
settle` w jednym batchu ≈ żywi w danej dobie). Pozycje dojrzałe z niższym
capem nie skorzystają z podbicia (ich nadwyżka wraca tą samą drogą przy ich
settle — kaskada zbieżna). Konserwacja: suma wypłat + indeks×shares + bufor
niezmieniona (testy). Dust: floor jak w `close_day` (I-05, bez zmian).

**Testy:** jednostkowe `test_r5_orphan_natychmiast_do_zywych_nie_do_pozniejszych`
(Adam/Beata/Celina: Beata 150, Celina 50, bufor 0, suma 200),
`test_r5_orphan_pusta_pula_idzie_do_bufora`, `test_r5_przepadek_early_exit_do_zywych`;
integracyjny `regresja_h5_orphan_do_zywych_nie_do_pozniejszego_stakera`
(scenariusz żądany przez C na prawdziwych transakcjach). Wynik: 10/10 lib,
**44/44 integracja w obu reżimach**, clippy `--all-targets -D warnings` czysto.

## M-02 (C) — pozycje sprzed R4 ze starą formułą `end_epoch` — GRANDFATHERING (testnet)

**Fakt:** pozycje utworzone przed deployem R4 (4.09) mają `end_epoch =
epoch_of(end_ts − 1)`; nowe: `epoch_of(end_ts) − 1`. Różnica: przy końcu w
środku doby stara pozycja ma prawo do jednej niepełnej doby więcej.

**Decyzja:** jawny grandfathering na testnecie; **na mainnecie problem nie
istnieje** (świeży deploy = wyłącznie nowe pozycje). Snapshot legacy: 
`scripts/audyt-naliczen.js` oznacza każdą taką pozycję znacznikiem
`[end_epoch wg STAREJ formuly]`; maksymalny koszt = ≤ 1 doba XNT na pozycję
(udział w koszyku doby `end_epoch`), pokrywany z fundingu tej doby, bez
wpływu na inne pozycje poza proporcją w tej jednej dobie. Kimi i B: nie-finding.

## P-01 (C) / I-07 (B) / Zadania 1–2 (Kimi) — provenance — DOMKNIĘTE W FREEZE

Uznane w całości. Błąd po naszej stronie: skrypt pakujący liczył sha256 zipa,
a potem **dopakowywał manifest z tym sha do środka** — stąd rozjazd
`70d707…` vs `448b40…`. Poprawka: `scripts/pack-audit.sh` — manifest generowany
**obok** archiwum, zawiera commit, sha256 `Cargo.lock`, features, toolchain,
sha256 **binarki `.so`** i sha256 zipa.

Procedura freeze (§5): jeden commit → `audit-evidence.sh` na nim → 
`build-testnet.sh` → manifest → deploy tej samej binarki → paczka. Jeden HEAD,
jeden rustc w manifeście (1.89.0 — toolchain hosta z CI; poprzedni manifest
podawał rustc hosta developera 1.97.1, co C słusznie wytknął).

## Q12 — compute units / Q15 — Dependabot — DO UZUPEŁNIENIA (nie blokują)

CU: brak pętli po pozycjach, limit domyślny 200 000 — pomiar z logów
testnetowych dołączymy do delta-rundy. Dependabot: mapowanie advisory →
dev-dependency vs SBF dołączymy tamże (Kimi/B: nie-blocker; C: prośba o listę).

## Residual świadomy (Kimi I-01…I-06, C Q6/Q10) — bez zmian

- `claim_genesis_window` bez rolla: samokorekta przez `xnt_window_claimed`,
  dowód braku underflow przy nowej formule (B Q6, Kimi Q6). UX retry — post-freeze.
- `DayNotClosed` martwy wariant (stabilność kodów), pauza tylko na `stake`
  (design "wyjście zawsze działa"), dust floor — bez zmian.
- Komentarz `end_epoch` (I-06 B) — poprawiony w `e16c634`.

## §5. Freeze — checklista wykonawcza

1. `git status` pusty na commicie freeze; `audit-evidence.sh` → `docs/TEST-LOG.*`.
2. `build-testnet.sh` → `release-manifest-testnet.txt` (sha `.so`).
3. Deploy binarki o tym sha; `solana program show` (slot) do manifestu.
   **Wykonane 2026-09-05:** `e16c634`, sha `6ae00e64…17c4`, slot 185873680.
4. Paczka: zip + manifest OBOK (sha zipa liczone po finalnym zapisie).
5. Delta-runda: trójka dostaje diff `87f358b..freeze`, ten dokument, TEST-LOG,
   CU, Dependabot. Prośba: werdykt M-01/M-02/P-01 + potwierdzenie freeze.
6. Post-freeze (Kimi Zadanie 5): upgrade authority → multisig/immutable;
   build mainnet bez `test-periods` (strażnik CI); ceremonia CAPY (mint 20M →
   wypalenie authority).
