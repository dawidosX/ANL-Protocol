# R8 — mini-runda: weryfikacja fixu `claim` (`saturating_sub` + `InvariantAlarm kind 2`) — Kimi

**Prompt:** `R8-CLAIM-SATURATING-SUB` (cztery pytania o jedną zmianę) · **Data:** 2026-09-05
**Kod:** `src_tree 7ab2a7455eca9f9efc7a83699f9aee4616017efa` (merge `2582b9d`; tag `v1.1-testnet-freeze` → `6113493`)
**Binarka / slot:** `1d44959176da39a83a42cadd835b108795d7fee4a0aed706c00e9f24b9a6ba0a` / 185933070

## Odpowiedzi (treść przekazana przez Dawida, 2026-09-05)

1. **Czy `saturating_sub` może dać nadpłatę XNT?** — **Nie.** Zwraca `max(0, accrued − claimed)`, nigdy więcej niż `xnt_accrued`; principal, ANL i CAPY liczone obok, niezależnie od tego rachunku.
2. **Czy `emit!(InvariantAlarm kind 2)` zmienia stan lub może revertować?** — **Nie.** `emit!` to `sol_log_data`: brak mutacji, brak revertu.
3. **Czy `checked_sub` w `claim_genesis_window` (`accrued_to_prog − already`) powinno zostać?** — **Tak, słusznie zostaje**: fail-fast dla wypłaty okna, principal nietknięty.
4. **DRAINABLE / FREEZE:** **DRAINABLE: NIEUDOWODNIONE** (NOT PROVEN SAFE — brak znalezionej ścieżki, bez dowodu formalnego) · **FREEZE: TAK** na `src_tree 7ab2a745…`.

Kontekst: pełny raport ofensywny Kimi z drain-challenge R8 — `2026-09-05_r8_kimi.md` (werdykt „NOT PROVEN SAFE — podtrzymany dla tree `7ab2a745`”).
