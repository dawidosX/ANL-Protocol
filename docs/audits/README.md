# docs/audits — archiwum raportów / report archive

Oryginalne, niezmienione raporty recenzentów. Skonsolidowana historia ze
statusami wszystkich ustaleń: [`../SECURITY-AUDITS.md`](../SECURITY-AUDITS.md)
(EN) · [`../SECURITY-AUDITS.pl.md`](../SECURITY-AUDITS.pl.md) (PL).
Original, unmodified reviewer reports; the consolidated history with the
status of every finding lives one directory up.

| Plik / File | Co to jest / What it is |
|---|---|
| `2026-07-18_audyt-bezpieczenstwa_openai-5.6sol.md` | Runda #1 — audyt wstępny (OpenAI 5.6sol) / Round #1 preliminary audit |
| `2026-07-18_audyt-bezpieczenstwa_claude-fable5.md` | Przegląd równoległy (Claude Fable 5) — ustalenia R1b-M1…M5 / parallel review |
| `2026-07-18_analiza-ogolna_claude-fable5.md` | Analiza ogólna towarzysząca przeglądowi równoległemu / companion general analysis |
| `audit-3-verification-gpt.md` | Weryfikacja poprawek po rundzie #3 (GPT) / round-3 fix verification |
| `audit-3-verification-grok.pdf` | Weryfikacja poprawek po rundzie #3 (Grok) / round-3 fix verification |
| `audit-4-verification-gpt.md` | Runda #4 — re-weryfikacja paczki `audit4` (GPT) / round-4 re-verification |
| `audit-4-verification-grok.pdf` | Runda #4 — re-weryfikacja paczki `audit4` (Grok) / round-4 re-verification |
| `2026-09-05_audyt-r6-adwersarialny_claude-fable5.md` | Runda #6 — audyt adwersarialny paczki `91418cc` (Claude Fable 5.1): R6-01 cap<debt (High), R6-02 nadpisanie checkpointu (Low), PoC w harnessie / round-6 adversarial audit |
| `2026-09-05_audyt-r6_B.md` | Runda #6 — raport B (delta po R5: orphan do żywych, F-01; freeze logiki TAK, F-02/P-01 otwarte) / round-6 report (B) |
| `2026-09-05_audyt-r6_C.md` | Runda #6 — analiza C (FREEZE BLOCKER provenance P-01; HIGH pin `initialize`, MEDIUM limit 200M) / round-6 analysis (C) |
| `2026-09-05_audyt-r7.1_B.md` | Runda #7.1 — raport B (R6-01/R6-02, pin, 200M; freeze logiki TAK, 8.5/10) / round-7.1 report (B) |
| `2026-09-05_audyt-r7.1_C.md` | Runda #7.1 — handoff C (trzy hipotezy release-path A/B/C → zamknięte w 7.2) / round-7.1 handoff (C) |
| `2026-09-05_audyt-r7.2_kimi.md` | Runda #7.2 (release-path) — werdykt Kimi: weryfikacja wprost na bundle, A/B/C Czyste, FREEZE TAK, 9/10 / round-7.2 verdict (Kimi) |
| `2026-09-05_audyt-r7.2_B.md` | Runda #7.2 (release-path) — werdykt B: A/B/C Czyste, FREEZE TAK, 9/10; odhaczenie zadań R5/R7 / round-7.2 verdict (B) |
| `2026-09-05_audyt-r7.2_C.md` | Runda #7.2 (release-path) — werdykt C: A/B/C CLOSED, FREEZE TAK, 9.3/10 / round-7.2 verdict (C) |

Raport rundy #2 (Grok, 8,5/10) został przekazany w formie konwersacyjnej i nie
zachował się jako plik; jego wnioski są streszczone w §4 historii. / The
round-#2 report (Grok, 8.5/10) was delivered conversationally and is
summarized in §4 of the consolidated history.

**Klucz atrybucji plików źródłowych (wszystkie rundy):** `AUDYT-RAPORT-runda*.md` („Audytor: Kimi”) = **Kimi** ·
`ANL-Protocol-Audit-Round*.md` („Security Audit Report”) = **B** · `*Analysis*` / `*Handoff*` / `*Freeze-Review*` /
`*Summary-2026-09-05*` = **C**. / Source-file attribution key: see above.
