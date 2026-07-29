# Model XNT — ANL Staking (FINALNY, zatwierdzony 27.07.2026)

## Cykl
- **Doba rozliczeniowa = 24h**, koniec o **2:00** (codziennie)
- Walidator wrzuca XNT w ciągu doby (kumuluje)
- Na koniec doby (2:00) XNT dzieli się między obecnych

## Podział w dobie (Model A)
- Kto jest w puli gdy doba się zamyka (2:00) → łapie udział = `pula_doby × shares / total_shares`
- Nowe wejście w ciągu doby ROZWADNIA obecnych (proporcjonalnie)
- Czas wejścia w dobie bez znaczenia — liczy się udział w total_shares na koniec doby

## Podział 65/35 + pusta pula
- Genesis 65% / Flexible 35% (gdy obie mają stakerów)
- Jedna pusta → druga bierze 100%
- OBIE puste → XNT KUMULUJE na następną dobę (bufor)

## Kumulacja przez stake
- Pozycja zbiera XNT z **KAŻDEJ doby**, w której była obecna
- Stake 3-dniowy = XNT z 3 dób (każda liczona osobno, proporcjonalnie)
- **Odbiór: dopiero gdy własny stake się kończy** (suma ze wszystkich dób)

## To jest DOKŁADNIE model akumulatora per-share (reward_index)!
Kluczowe spostrzeżenie:
- "udział = pula_doby × shares/total_shares, kumulowane przez doby" 
  = KLASYCZNY reward_index (akumulator XNT-na-share)
- reward_index rośnie o `pula_doby/total_shares` każdej doby
- pozycja dostaje `(reward_index_teraz − reward_index_przy_wejściu) × shares`

ALE obecny kontrakt ma debt_index = snapshot PRZY WEJŚCIU, przez co
pozycja wchodząca PO fundingu dostaje 0. To jest błąd IMPLEMENTACJI,
nie modelu — bo w Modelu A pozycja MA łapać dobę, w której weszła.

## Różnica do naprawy
Obecny: `debt_index = reward_index` w chwili stake → traci bieżącą dobę
Poprawny: pozycja łapie KAŻDĄ dobę od wejścia do końca stake'u,
          w tym dobę wejścia (proporcjonalnie do shares na koniec tej doby)

---

# WERDYKT TECHNICZNY (po analizie kontraktu)

## Co kontrakt JUŻ robi dobrze:
- epoka = doba (86400s), leniwy akumulator reward_index ✅
- kumulacja XNT przez doby, wypłata przy claim ✅
- pusta pula → xnt_undistributed (bufor kumulacyjny) ✅

## BŁĄD 1: debt_index = bieżący index przy wejściu (lib stake.rs:193)
- `pos.xnt_debt_index = pool.xnt_reward_index` (po fundingach doby)
- skutek: kto wszedł PO fundingu w danej dobie, NIE łapie tej doby (dostaje 0)
- ZASADA: każdy obecny na koniec doby (2:00) łapie CAŁĄ dobę wg shares,
  niezależnie kiedy w dobie wszedł i kiedy był funding
- FIX: debt_index musi = index na POCZĄTKU doby wejścia (sprzed fundingów tej doby)
  → pozycja łapie pełną dobę wejścia
  → nowe wejście rozwadnia obecnych (zgodne z Modelem A)

## BŁĄD 2: pusta pula → bufor zamiast 100% do drugiej (state mod.rs ~103)
- `if total_shares == 0 { xnt_undistributed = part_total; return }`
- to trzyma w buforze TEJ puli
- ZASADA: jedna pula pusta → jej 65%/35% idzie 100% do DRUGIEJ puli tego samego dnia
- tylko gdy OBIE puste → bufor (kumuluje na następną dobę)
- FIX: w fund_xnt, gdy jedna pula ma total_shares==0 a druga >0:
  cały XNT (part_total obu) → do niepustej puli
  gdy obie ==0 → bufor (jak teraz)

## Do zaprojektowania przy implementacji:
- jak liczyć "index na początku doby" (checkpoint dobowy startu?)
- redystrybucja pustej puli: policzyć w fund_xnt przed update_xnt_index
- testy: scenariusz #1 rano / #2 po fundingu → oboje dzielą
- testy: Flexible pusty → Genesis bierze 100%
