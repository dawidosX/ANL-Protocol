# Bug bounty — publiczny rejestr zgłoszeń / public bounty ledger

Rejestr jest publiczny dla przejrzystości programu bounty (`SECURITY.pl.md` / `SECURITY.md`). **Dane zgłaszających i adresy
wypłat są prywatne** i trzymane poza repozytorium; credit (nick/handle) dopisywany jest **wyłącznie za zgodą zgłaszającego**.
Wypłata następuje po naprawie i re-audycie (nie po zgłoszeniu). Wagi wg §1 `SECURITY.pl.md`, ustalane z jednym z niezależnych audytorów.

| Data | Finding | Waga | Nagroda | Status | Commit-fix |
|---|---|---|---|---|---|
| 2026-09 | EDGE — bufor pustej puli (`xnt_undistributed` przejmowany przez pierwszego stakera po przerwie) | High | 250 000 ANL | naprawione, deploy 186156990 (re-audyt R9/R10/R10.1: 3× TAK) | v1.3.1 (`5d4fe7d` → `v1.3.1-testnet-freeze`) |
| 2026-09 | XNT-01 — `unstake_early` bez rolla doby (wypłata dojrzałej pozycji zależna od kolejności) | Medium | 50 000 ANL | naprawione, deploy 186156990 (re-audyt R9/R10/R10.1: 3× TAK) | v1.3.1 (`5d4fe7d` → `v1.3.1-testnet-freeze`) |
| 2026-09 | XNT-02 — przepadek/orphan podnosił indeks bez checkpointu (pozycja żywa w chwili przepadku traciła udział na rzecz późniejszego stakera) | Medium | 50 000 ANL | naprawione, deploy 186156990 (re-audyt R10/R10.1: DRAINABLE NO ×3) | v1.3.1 (`7412df1`, `45ee285` → `v1.3.1-testnet-freeze`) |
| 2026-09 | Hazard wdrożeniowy — horyzont fundingu 9 d w buildzie testnetowym vs genesis starszy niż 9 d (upgrade zabiłby `fund_xnt`) | Low (heads-up) | 10 000 ANL (uznaniowe) | naprawione przed deployem, deploy 186156990 | v1.3.1 (`a1eaf59`) |
| 2026-09 | `fund_capy` permissionless | — | 0 (znane wykluczenie, brak wektora) | odrzucone | n/d |

*Statusy: zgłoszone → potwierdzone → naprawione (commit) → re-audyt → wypłacone / odrzucone. Aktualizacja przy każdej zmianie.*
