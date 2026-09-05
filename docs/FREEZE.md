# AUDIT-FREEZE — kod testnetowy v1.0 (2026-09-05)

**Tag:** `v1.0-testnet-freeze` → commit `272750da8b748e3faa5a26029e516500f374c883` (`main`).
Kod na tym drzewie jest **finalnym kodem testnetu i bazą mainnetu**. Każda późniejsza zmiana w
`programs/anl_staking/src/` lub `crates/anl-math/src/` = nowy `src_tree` = **mini-runda audytu przed deployem**
(branch `fix/<temat>`, raport dla Recenzenta, merge po OK, evidence → build → deploy → provenance).

## 1. Provenance (do samodzielnej weryfikacji z bundle'a)

| Element | Wartość | Jak sprawdzić |
|---|---|---|
| HEAD (paczka audytowa) | `272750da8b748e3faa5a26029e516500f374c883` | `git bundle verify …history-272750d.bundle`; `git rev-parse v1.0-testnet-freeze^{}` |
| `src_tree` (`HEAD:programs/anl_staking/src`) | `4c2256398137bb417a1b769316137852d14ec4d5` | `git rev-parse 272750d:programs/anl_staking/src` |
| `code_tree` (`HEAD:programs`) | `7dbf3de415685767654ad8e068f6034e27e51e53` | `git rev-parse 272750d:programs` |
| `math_tree` (`HEAD:crates/anl-math`) | `6fb61151f3e10b0a5d68249a941721500b34a5b3` (bez zmian od R6) | `git rev-parse 272750d:crates/anl-math` |
| Evidence | `docs/TEST-LOG.txt` na `544b12a` (ten sam `src_tree`/`code_tree`): integracja 52/52 × 2 reżimy, lib 13/13, core 34+2 / 30+2, clippy ×2, fmt, `cargo audit` 0 podatności | `sha256sum docs/TEST-LOG.txt` = `docs/TEST-LOG.sha256` |
| Binarka `.so` | sha256 `87b431d43280e4eccfca71725bbfafda2f1fbd2fb2d95a94cd2715fd4ae530a3`, 676 536 B, `network-testnet,test-periods`, platform-tools v1.41 (rustc 1.75.0) | `release-manifest-testnet.txt`; rebuild: `cargo build-sbf` na `git archive 272750d` |
| Deploy testnet | program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`, slot **185899744**, upgrade authority `Dx2vEpVdMh2qScz4vEHAXquTm6QYocKbmPRdcXHLzvEm` | `solana program show 4Cpx… --url https://rpc.testnet.x1.xyz` |
| Dump on-chain | pierwsze 676 536 B dumpu == binarka (0 bajtów różnicy), reszta zera | `solana program dump 4Cpx… x.so && head -c 676536 x.so \| sha256sum` |
| Paczka audytowa | `ANL-Protocol-audyt-r7.2-272750d.zip` sha256 `495de0af0b4b9f29310c70d52c4f3af93628d684d9d3d9d1656ce72eaa250888` (manifest OBOK zipa) | `pack-audit.sh` fail-closed na `src_tree`/`code_tree`/`math_tree`/`sha256` |
| Snapshot | `ANL-Protocol-FULL-272750d.zip` `b10913167a5177d74025fcefd5fdeaa3e5f2d7a13a14de76cf14121016f5683b`; `ANL-Protocol-history-272750d.bundle` `53b6809452fb1b1fb81b7f50b76bd096c3a9cc5a90230711d899887e63d22583` | `git bundle verify` |

Buildy bez feature sieciowego mają osobny Program ID `ChG81WApHgpbWjt4r8wmJ57WS3MkXyMGzJ2tBbC8pcov` (R7.2 A) — binarka
bez pinu `initialize` nie uruchomi się pod `4Cpx…`.

## 2. Werdykty rundy 7.2 (release-path; logika ekonomiczna bez zmian od 7.1)

| Audytor | A (Program ID / pin) | B (provenance) | C (CAPY 20M) | Regresja 7.1→7.2 | FREEZE | Ocena | Plik |
|---|---|---|---|---|---|---|---|
| Kimi | Czyste | Czyste (weryfikacja wprost na bundle) | Czyste | Czyste | **TAK** | **9 / 10** | `docs/audits/2026-09-05_audyt-r7.2_kimi.md` |
| B | Czyste | Czyste (Uwaga: osobny manifest mainnet) | Czyste | Czyste | **TAK** | **9 / 10** | `docs/audits/2026-09-05_audyt-r7.2_B.md` |
| C | CLOSED | CLOSED (+ `cargo_lock_sha`/`root_cargo_toml_sha` w manifeście mainnet) | CLOSED | Czyste | **TAK** | **9.3 / 10** | `docs/audits/2026-09-05_audyt-r7.2_C.md` |

Findingów Critical/High/Medium/Low w delcie 7.2: **0** (wszyscy trzej). Punkty odjęte wyłącznie za F-02 (jeden hot key)
i za brak ceremonii mainnet — nie za luki w instrukcjach. Historia rund: `docs/CHANGES-AFTER-ROUND{4,5,6,7}.md`,
`docs/AUDIT-NOTE-round7.2.md`, `docs/AUDIT-BRIEF-round7.md`.

## 3. Poza kodem — do mainnetu (checklista, szczegóły w `docs/MAINNET-RUNBOOK.md`)

1. **Klucze:** multisig ≥ 2/3 z timelockiem jako upgrade authority; rozdzielenie ról authority / operator / upgrade;
   operator na serwerze z prawem wyłącznie do `fund_xnt` (F-02).
2. **Ceremonia w jednym commicie:** `EXPECTED_INIT_AUTHORITY` → pubkey multisiga **razem** ze strażnikiem
   `init_authority_pinned_mainnet`; ewentualnie nowy Program ID (**razem** ze strażnikiem `program_id_pinned_mainnet`).
   Nowy `src_tree` ⇒ własny provenance bundle i mini-runda.
3. **CAPY:** mint dokładnie 20 000 000 → wypalenie mint authority (i freeze) → `init_capy_vault` (`InvalidCapySupply`
   pilnuje kolejności i kwoty).
4. **Build:** `network-mainnet` bez `test-periods` (`compile_error!` pilnuje), platform-tools jak w manifeście;
   `build-mainnet.sh` + `release-manifest-mainnet.txt` w tej samej pętli fail-closed co testnet.
5. **Skarbiec nagród:** 200 000 000 ANL do `reward_vault` (`fund_rewards`); stała `ANL_REWARD_POOL` = twardy limit emisji.
6. **Obserwacja:** ≥ 45 dni pracy na mainnecie z upgrade authority w multisigu; potem `solana program set-upgrade-authority --final`
   (immutable) — dopiero po upgrade'zie stosu Solana/Anchor (listy `deny.toml`/`audit.toml` puste).
7. **WP:** cooldown 3 dni i ≤ 365 dni Flexible, limit emisji 200M, polityka orphanów (żywi w chwili settle), `initialize` przypięty,
   SLA bota `close_day → settle_expired` w tej samej dobie.
8. **Proces:** wyłączyć bypass status-checków na `main`; zmiany `programs/`/`crates/` wyłącznie przez branch + raport dla Recenzenta.

## 4. Aktualizacja po R8 (2026-09-05, wieczór) — testnet `src_tree 7ab2a745…`

Drain-challenge trzech niezależnych audytorów (GPT, Grok, Kimi): **brak drenażu, blokady i double-spend**; jedyny wskazany
punkt (`checked_sub` w `claim` przy `xnt_window_claimed > xnt_accrued`, residual I-01) naprawiony w `fix/r8-principal-fail-open`
(`saturating_sub` + `InvariantAlarm kind 2`, test `regresja_r8_window_claimed_ponad_accrued_nie_blokuje_principalu`),
OK Recenzenta, merge `2582b9d`.

| Element | Wartość |
|---|---|
| `src_tree` | `7ab2a7455eca9f9efc7a83699f9aee4616017efa` (zmiana wyłącznie w `lifecycle.rs::claim`) |
| `code_tree` / `math_tree` | `2842efaf75bb255ea0d7567f3280e83f3f33e0dd` / `6fb61151f3e10b0a5d68249a941721500b34a5b3` |
| Evidence | `docs/TEST-LOG.txt` na `2582b9d`: integracja 53/53 × 2 reżimy, lib 13/13, core 34+2 / 30+2, clippy ×2, fmt, `cargo audit` 0 |
| Binarka `.so` | sha256 `1d44959176da39a83a42cadd835b108795d7fee4a0aed706c00e9f24b9a6ba0a`, 676 840 B, platform-tools v1.41 |
| Deploy testnet | slot **185933070**, program `4Cpx…`; dump on-chain == binarka (0 bajtów różnicy) |

Tag `v1.0-testnet-freeze` (`272750d`, `src_tree 4c225639…`) pozostaje punktem freeze logiki ekonomicznej; R8 to obrona w głąb
bez zmiany ekonomiki. Baza mainnetu = `src_tree 7ab2a745…` (decyzja o tagu `v1.0.1-testnet` po stronie Dawida).

## 5. R8 mini-runda: 2/2 potwierdzenie freeze na `src_tree 7ab2a745…`

Recenzja jednej zmiany (`claim`: `saturating_sub` + `InvariantAlarm kind 2`, prompt `R8-CLAIM-SATURATING-SUB`), po drain-challenge
GPT/Grok/Kimi bez findingów:

| Audytor | Fix zweryfikowany | DRAINABLE | FREEZE na `7ab2a745…` | Plik |
|---|---|---|---|---|
| Kimi | TAK | **NIEUDOWODNIONE** (NOT PROVEN SAFE) | **TAK** | `docs/audits/2026-09-05_r8-weryfikacja_kimi.md` |
| Grok | TAK | **NIE** | **TAK** | `docs/audits/2026-09-05_r8-weryfikacja_grok.md` |

Tag `v1.1-testnet-freeze` → `6113493` (komplet: kod `7ab2a745…`, manifest sha `1d449591…`, TEST-LOG 53/53 × 2, FREEZE.md).
Provenance zweryfikowane 2026-09-05: `src_tree` kodu = manifest = TEST-LOG; sha manifestu = dump z łańcucha; slot 185933070.
