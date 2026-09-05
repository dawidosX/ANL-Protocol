# R8 — weryfikacja fixu `claim` (saturating_sub + InvariantAlarm kind 2) — Kimi

**Runda:** R8 mini-runda (recenzja jednej zmiany, prompt `R8-CLAIM-SATURATING-SUB`) · **Data:** 2026-09-05
**Kod:** `src_tree 7ab2a7455eca9f9efc7a83699f9aee4616017efa` (merge `2582b9d`, tag `v1.1-testnet-freeze` → `6113493`)
**Binarka / slot:** `1d44959176da39a83a42cadd835b108795d7fee4a0aed706c00e9f24b9a6ba0a` / 185933070

## Werdykt (przekazany konwersacyjnie przez Dawida, 2026-09-05)

| Pytanie | Werdykt |
|---|---|
| Zmiana `checked_sub` → `saturating_sub` w `claim` (XNT do wypłaty) | zweryfikowana: principal + ANL nie zależą od rachunku XNT; przy `window_claimed > accrued` XNT = 0, alarm `kind 2` |
| **DRAINABLE** | **NIEUDOWODNIONE** (NOT PROVEN SAFE) — brak znalezionej ścieżki drenażu, bez dowodu formalnego |
| **FREEZE** na `src_tree 7ab2a745…` | **TAK** |

*Pełna treść raportu audytora nie została dostarczona jako plik — ten dokument jest zapisem werdyktu z komunikatu Dawida.
Po otrzymaniu pliku (klucz atrybucji: `AUDYT-RAPORT-runda*.md`) treść zostanie podmieniona w całości.*
