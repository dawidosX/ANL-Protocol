# docs/audits — indeks audytów / audit index

Oryginalne, niezmienione raporty recenzentów (lub zapisy werdyktów przekazanych konwersacyjnie — oznaczone).
Skonsolidowana historia ustaleń ze statusami: [`../SECURITY-AUDITS.md`](../SECURITY-AUDITS.md) (EN) ·
[`../SECURITY-AUDITS.pl.md`](../SECURITY-AUDITS.pl.md) (PL); zmiany po rundach: `../CHANGES-AFTER-*.md`; freeze: [`../FREEZE.md`](../FREEZE.md).
Original, unmodified reviewer reports (or recorded verdicts where delivered conversationally); consolidated finding history one directory up.

**Stan (2026-09-05):** kod testnetu zamrożony — tag `v1.1-testnet-freeze` (`6113493`, `src_tree 7ab2a745…`, binarka `1d449591…`, slot 185933070).
8 rund audytu + drain-challenge (GPT / Grok / Kimi: zero drenażu) + mini-runda R8 (Kimi, Grok: FREEZE TAK). Mainnet po ceremonii (`../MAINNET-RUNBOOK.md`).

## Indeks rund

### Seria lipiec 2026 (audyty #1–#6, przed testnetem)

| Runda | Data | Audytor | Werdykt / wynik | Plik |
|---|---|---|---|---|
| #1 — audyt wstępny | 2026-07-18 | GPT (OpenAI 5.6sol) | ustalenia R1-01…; Critical #1 (naliczanie XNT po `end_ts`) → model epok | `2026-07-18_audyt-bezpieczenstwa_openai-5.6sol.md` |
| #1b — przegląd równoległy | 2026-07-18 | Claude Fable 5 | R1b-M1…M5 (M-3 naprawione, M-1 → procedura deployu, M-2/M-4/M-5 pre-mainnet) | `2026-07-18_audyt-bezpieczenstwa_claude-fable5.md`, `2026-07-18_analiza-ogolna_claude-fable5.md` |
| #2 — przegląd po poprawkach | 2026-07-18/19 | Grok | **8.5 / 10**; potwierdzenie fixów R1, Critical #1 potwierdzony | — (konwersacyjnie; streszczenie w SECURITY-AUDITS §4) |
| #3 — audyt szczegółowy modelu epok | 2026-07-19 | GPT | **6.8 / 10** (surowsza metodologia); ustalenia R3 + M-02 | — (streszczenie w SECURITY-AUDITS §6) |
| #3 — weryfikacje poprawek | 2026-07-19 | GPT / Grok | GPT: testnet/pilot **gotowy** (osobny Program ID, limit wartości, monitoring); Grok: **warunkowo gotowy** | `audit-3-verification-gpt.md`, `audit-3-verification-grok.pdf` |
| #4 — re-weryfikacja paczki `audit4` | 2026-07-19 | GPT / Grok | V-01…V-05 zamknięte; DOC-01…05 poprawione; GPT: testnet **gotowy bezwarunkowo**, Grok: **warunkowo** | `audit-4-verification-gpt.md`, `audit-4-verification-grok.pdf` |
| #5 — audyt snapshotu working-tree | 2026-07-20 | GPT / Grok | GPT: testnet **gotowy po pełnej suicie na realnym toolchainie**; immutable mainnet **nie** | `2026-07-20_audyt5_gpt.pdf`, `2026-07-20_audyt5_grok.pdf`, `MANIFEST-audyt5.txt` |
| #6 — delta (testy/docs/config) | 2026-07-20 | zespół + dowody | 4-etapowy setup, TS-AUD5, evidence na realnym toolchainie | `MANIFEST-audyt6.txt`, `TEST-LOG-2026-07-20.*`, `2026-07-20_audyt-zmian-po-audycie.md`, `2026-07-20_onchain-sbf-evidence.md` |

### Seria wrzesień 2026 (rundy R4–R8, testnet X1 na żywo)

| Runda | Data | Audytor | Werdykt / wynik | Plik |
|---|---|---|---|---|
| R4 | 2026-09-03/04 | Kimi / B / C | H-01 (wyścig `close_day`/`settle`), M-01 (stale `current_day`) → fix `roll_day_and_write_checkpoint`, `end_epoch` pełne doby | — (raporty konwersacyjne; `../CHANGES-AFTER-ROUND4.md`, `../AUDIT-BRIEF-round4.md`) |
| R5 — re-audyt zmian po R4 | 2026-09-04/05 | Kimi / B / C | Kimi **TAK**, B **TAK**, C **NIE** (M-01 orphan do bufora, M-02 legacy `end_epoch`, P-01 provenance) → fix `redistribute_to_live` | — (konwersacyjne; `../CHANGES-AFTER-ROUND5.md`, `../AUDIT-BRIEF-round5.md`) |
| R6 — audyt adwersarialny | 2026-09-05 | Claude Code (Fable 5.1) | drain **NO**; R6-01 High (cap<debt → lock), R6-02 Low, I-01…I-07; **8 / 10** po fixach | `2026-09-05_audyt-r6-adwersarialny_claude-fable5.md` |
| R6 | 2026-09-05 | B | freeze logiki **TAK**; F-02 (klucze), P-01 (provenance) otwarte | `2026-09-05_audyt-r6_B.md` |
| R6 | 2026-09-05 | C | testnet TAK / mainnet **NIE**: FREEZE BLOCKER provenance (P-01), HIGH pin `initialize`, MEDIUM limit 200M | `2026-09-05_audyt-r6_C.md` |
| R6 | 2026-09-05 | Kimi | Info: orphan zależny od timingu settle; prowenance tree-hash | — (konwersacyjnie; `../CHANGES-AFTER-ROUND7.md` §2) |
| R7 / R7.1 — po fixach R6 | 2026-09-05 | Kimi / B / C | Kimi **9 / 10 TAK**; B **8.5 / 10 TAK**; C: handoff z trzema hipotezami release-path (A/B/C) | `2026-09-05_audyt-r7.1_B.md`, `2026-09-05_audyt-r7.1_C.md`; Kimi — konwersacyjnie; `../AUDIT-BRIEF-round7.md` |
| R7.2 — release-path (A/B/C) | 2026-09-05 | Kimi / B / C | **FREEZE TAK ×3**: Kimi **9 / 10**, B **9 / 10**, C **9.3 / 10** → tag `v1.0-testnet-freeze` (`272750d`, `src_tree 4c225639…`) | `2026-09-05_audyt-r7.2_kimi.md`, `2026-09-05_audyt-r7.2_B.md`, `2026-09-05_audyt-r7.2_C.md`; `../AUDIT-NOTE-round7.2.md` |
| R8 — drain-challenge (atak) | 2026-09-05 | GPT / Grok / Kimi | **zero drenażu / locku / double-spend**; jedyny punkt: `checked_sub` w `claim` → fix R8. GPT: brak drenażu; Grok: **DRAINABLE NIE**; Kimi: **NOT PROVEN SAFE** | `2026-09-05_r8_gpt.md` (zapis), `2026-09-05_r8_grok.md` (zapis), `2026-09-05_r8_kimi.md` (pełny raport) |
| R8 — mini-runda (fix `saturating_sub`) | 2026-09-05 | Kimi / Grok | Kimi: DRAINABLE **NIEUDOWODNIONE**, FREEZE **TAK**; Grok: DRAINABLE **NIE**, FREEZE **TAK** → tag `v1.1-testnet-freeze` (`6113493`, `src_tree 7ab2a745…`) | `2026-09-05_r8-weryfikacja_kimi.md`, `2026-09-05_r8-weryfikacja_grok.md` |

Legenda: **B**, **C** — dwa niezależne modele recenzujące (oznaczenia z rund wrześniowych, bez nazw handlowych); „zapis” — werdykt przekazany
konwersacyjnie, spisany przez zespół (pełny raport do podmiany po dostarczeniu pliku).

**Klucz atrybucji plików źródłowych (wszystkie rundy):** `AUDYT-RAPORT-runda*.md` („Audytor: Kimi”) = **Kimi** ·
`ANL-Protocol-Audit-Round*.md` („Security Audit Report”) = **B** · `*Analysis*` / `*Handoff*` / `*Freeze-Review*` /
`*Summary-2026-09-05*` = **C** · `ANL_Pentest_Report_RoundA.md` (drain-challenge R8) = **Kimi**.
