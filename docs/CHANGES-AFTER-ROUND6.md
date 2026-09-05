# Zmiany po rundzie 6 (audyt adwersarialny wewnetrzny, 2026-09-05) — commit `bb2deb8`

Pelny raport: `docs/audits/ADVERSARIAL-AUDIT-e16c634.md`. Werdykt po naprawach: brak sciezki
kradziezy rewards/principal przez nieautoryzowanego; jedyny finding (F-01, liveness) naprawiony.

## F-01 (Medium, liveness) — griefing rezerwacja reward pool — NAPRAWIONE
Problem: rezerwacja ANL liniowa w dniach (do 3650) + `unstake_early` Flexible bez kosztu
=> pozycja 1,25x wolnej rezerwy blokowala `stake` wszystkim, wychodzila za darmo, w petli.
Fix: `MAX_PERIOD_DAYS_FLEXIBLE = 365` (rezerwacja <= 8% kapitalu zamiast 80%) +
`EARLY_EXIT_COOLDOWN_SECS = 3 dni` (test-periods: 1 h) przed `unstake_early`.
Koszt ataku: 0 -> ~12,5x wolnej rezerwy zablokowanej >= 3 dni na cykl. Genesis bez zmian
(kapital realnie zablokowany). Nowy blad `EarlyExitCooldown` (na koncu enumu).
Testy: `atak_f01a/b/c` (47/47 w obu rezimach). UWAGA produktowa: WP musi opisac
cooldown 3 dni dla Flexible (bylo: wyjscie w kazdej chwili).

## Inwariant 10 (bilans skarbca >= ksiega) — z NOT PROVEN na PASS
`test_r6_property_konserwacja_xnt_losowe_sekwencje`: 40 ziaren x 120 losowych operacji
{stake, fund, roll, settle(cap), forfeit}; po kazdym kroku
`wyplacone + pending(zywi) + bufor + koszyk <= zafundowane` oraz dust ograniczony
liczba dzielen. Zero naruszen.

## Q12 — Compute Units (pomiar z testnetu, program 4Cpx..., 4-5.09.2026)
| Instrukcja | max CU | limit |
|---|---|---|
| Claim | 60 237 | 200 000 |
| Stake | 41 264 | 200 000 |
| ClaimGenesisWindow | 28 945 | 200 000 |
| ClaimCapy | 22 714 | 200 000 |
Brak petli po pozycjach; zapas > 3x.

## F-02 (klucze) — organizacyjne, PRZED mainnetem
Jeden hot key = upgrade authority + authority + operator. Wymagane: upgrade authority ->
multisig >= 2/3 z timelockiem (docelowo immutable), rozdzielenie trzech rol, operator na
serwerze z prawem tylko do `fund_xnt`, alerty TG na pause/set_operator/upgrade/fund_rewards.

## Residualy bez zmian
F-03 dust floor (brak wektora), F-04 Genesis 200% rezerwacji w oknie 1 (kapital
zablokowany 10 lat — decyzja produktowa), okna Genesis bez rolla (samokorekta).
