# Bug bounty — publiczny rejestr zgłoszeń / public bounty ledger

Rejestr jest publiczny dla przejrzystości programu bounty (`SECURITY.pl.md` / `SECURITY.md`). **Dane zgłaszających i adresy
wypłat są prywatne** i trzymane poza repozytorium; credit (nick/handle) dopisywany jest **wyłącznie za zgodą zgłaszającego**.
Wypłata następuje po naprawie i re-audycie (nie po zgłoszeniu). Wagi wg §1 `SECURITY.pl.md`, ustalane z jednym z niezależnych audytorów.

| Data | Finding | Waga | Nagroda | Status | Commit-fix |
|---|---|---|---|---|---|
| 2026-09 | EDGE — bufor pustej puli (`xnt_undistributed` przejmowany przez pierwszego stakera po przerwie) | High | 250 000 ANL | potwierdzone, wypłata po re-audycie v1.2 | TBD |
| 2026-09 | XNT-01 — `unstake_early` bez rolla doby (wypłata dojrzałej pozycji zależna od kolejności) | Medium | 50 000 ANL | potwierdzone, wypłata po re-audycie v1.2 | TBD |
| 2026-09 | `fund_capy` permissionless | — | 0 (znane wykluczenie, brak wektora) | odrzucone | n/d |

*Statusy: zgłoszone → potwierdzone → naprawione (commit) → re-audyt → wypłacone / odrzucone. Aktualizacja przy każdej zmianie.*
