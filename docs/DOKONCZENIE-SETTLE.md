# Dokończenie modelu XNT — ostatni element (nowa sesja)

## MODEL FINALNY (zatwierdzony): Sposób 2 — "index raz" + leniwe domknięcie
- doba = epoka (86400s), koniec ~2:00 (bez sekundowej precyzji)
- XNT wpada do KOSZYKA doby (add_to_basket) — NIE dzieli od razu
- domknięcie doby LENIWE: pierwsza operacja po granicy doby (fund/stake/claim)
  wywołuje close_day → index += koszyk/total_shares (jeden zapis, skaluje się)
- staker liczy XNT z index: (index − debt) × shares, odbiera przy claim
- środki XNT leżą w skarbcu do claim (nie wysyłane o 2:00, tylko księgowane w index)
- BEZ dodatkowych kont w stake, BEZ iteracji po pozycjach

## STAN OBECNY (commit c34564a na gałęzi wariant-a-xnt, zbudowany):
KROKI 1-5 GOTOWE:
- BŁĄD 2: pusta pula → 100% do drugiej (fund.rs)
- PoolConfig: current_day_basket + current_day (reserved 48→32, LEN bez zmian)
- metody close_day / add_to_basket / roll_day_if_needed (state/mod.rs)
- fund_xnt: add_to_basket (koszyk dobowy)
- stake: roll_day_if_needed przed shares, debt z końca poprzedniej doby
KROK 6a COFNIĘTY (rozszerzenie checkpointu basket/shares — NIEPOTRZEBNE w Sposobie 2)

## DO ZROBIENIA (ostatni element):

### 1. Zapis finalnego indexu doby E do jej checkpointu
PROBLEM: checkpoint doby E musi trzymać index PO domknięciu doby E.
W modelu koszyka domknięcie E następuje przy pierwszej operacji doby E+1
(add_to_basket woła close_day E). Więc finalny index E znany PO add_to_basket.

MIEJSCE: fund.rs, handler fund_xnt, PO add_to_basket (linia ~308).
prev_ckpt (genesis_prev_ckpt/flexible_prev_ckpt) to checkpoint doby E —
Option<UncheckedAccount> z #[account(mut)], zapisywalne.

KOD (dla obu pul):
```
// po add_to_basket, gdy index zawiera już domknięcie doby E:
if let Some(prev) = ctx.accounts.genesis_prev_ckpt.as_ref() {
    let info = prev.to_account_info();
    // tylko jeśli to realny checkpoint (nie placeholder PROGRAM_ID)
    if *info.owner == *ctx.program_id {
        let mut ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
        ck.index = ctx.accounts.genesis_pool.xnt_reward_index; // finalny po E
        ck.try_serialize(&mut &mut info.data.borrow_mut()[..])?;
    }
}
// to samo dla flexible_prev_ckpt
```
UWAGA: prev_ckpt może być None (pierwsza doba) albo placeholder PROGRAM_ID
(gdy last_funded_epoch=NO_EPOCH). Sprawdzać owner == program_id przed zapisem.
UWAGA: cur.index (checkpoint E+1, linia 316-317) zostaje jako baza tymczasowa —
nadpisze się finalnym przy domknięciu E+1.

### 2. Settle/claim — cap z checkpointu (deterministyczny, do doby end_epoch)
Obecny settlement_cap_index (lifecycle.rs:60) czyta ck.index z checkpointu
doby ≤ end_epoch. Po zmianie #1 checkpoint trzyma FINALNY index po dobie →
cap poprawny. settle_position_at już obsługuje orphan (nadmiar po end_epoch → bufor).

ALE: sprawdzić przypadek, gdy doba end_epoch NIE została jeszcze domknięta
(brak fundingu E+1, pozycja settluje jako pierwsza operacja po E).
Wtedy checkpoint E nie ma finalnego indexu. ROZWIĄZANIE: settle sam domyka —
przed cap, wywołać na pool close_day jeśli current_day == end_epoch i minęła
(now >= koniec doby end_epoch). To wymaga pool mut w settle + zapis do checkpointu E.
ALTERNATYWA prostsza: wymagać, by przed settle ktoś domknął dobę (funding E+1
albo dedykowana instrukcja close_day publiczna). Rozważyć w nowej sesji.

### 3. Opcjonalnie: publiczna instrukcja close_day (bot o 2:00 jako gwarancja)
Instrukcja domykająca dobę: bierze pool + prev_ckpt, woła close_day, zapisuje
finalny index do checkpointu. Ktokolwiek może wywołać (deterministyczne).
Bot cron o 2:00 wywołuje ją jako gwarancję punktualności (opcjonalne — leniwe
domknięcie działa bez niej).

### 4. TESTY (obowiązkowe przed deploy):
- #1 rano (1M), funding 10 XNT, #2 po południu (1M) → po domknięciu doby oba mają 5
- Flexible pusty → Genesis bierze 100% (BŁĄD 2)
- obie puste → koszyk kumuluje na następną dobę
- pozycja przez 3 doby → suma XNT z 3 dób
- pozycja kończąca się, potem funding → orphan (nadmiar) do bufora
- pozycja settlująca w niezamkniętej dobie → domknięcie wymuszone

## BUILD (działa!):
toolchain: solana 2.1.0 (anza), Cargo.lock z przypiętymi wersjami (blake3 1.5.5,
borsh 1.5.1, proc-macro-crate 3.1.0, toml_datetime, zeroize_derive 1.4.2,
indexmap 2.5.0, jobserver 0.1.32, unicode-segmentation 1.11.0).
Backup: Cargo.lock.DZIALAJACY.
Komenda: touch programs/anl_staking/src/lib.rs; cargo-build-sbf --features network-testnet,test-periods

## PO UKOŃCZENIU:
1. cargo test (testy jednostkowe) — aż zielone
2. deploy na testnet 49vhBow (upgrade albo nowy program)
3. test scenariuszy na żywo (staking + fund_xnt + domknięcie + claim)
4. dopiero gdy wszystko gra → ten sam kod na mainnet
