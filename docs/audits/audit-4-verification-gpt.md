# Weryfikacja paczki `anl-protocol-audit4-v1.00` (commit ddf4b36)
*Raport weryfikacyjny GPT · 19.07.2026 · zarchiwizowany bez zmian treści (formatowanie: markdown)*

Przeanalizowałem źródła w załączonym zipie. Poniżej odpowiedzi punkt po punkcie z dowodami.

## (a) Weryfikacja punktów 1–4 i status ustaleń przedtestnetowych

**1. `scripts/audit-evidence.sh` — fail-closed** — **Potwierdzone.** `set -euo pipefail` (linia 8); bramka czystego drzewa `if [ -n "$(git status --porcelain)" ]` przed czymkolwiek (linie 11–16); log do pliku tymczasowego poza repo (`mktemp /tmp/...`); `expect_fail` wymaga **kod ≠ 0 ORAZ** dokładnego komunikatu `compile_error!` (linie 32–48); stopka wiąże bieg z HEAD + `docs/TEST-LOG.sha256`; niezerowy exit przy dowolnym błędzie. Status: **zamknięte**.

**2. `scripts/build-mainnet.sh` / `build-testnet.sh`** — **Potwierdzone.** Oba zaczynają od bramki `git status --porcelain`; manifest zawiera HEAD / features / sha256 / rustc. Status: **zamknięte**.

**3. CI (`.github/workflows/ci.yml`)** — **Potwierdzone.** Supply-chain: `cargo audit` i `cargo deny check advisories bans sources` **bez** `|| true` (blokujące); drugi test negatywny release-guards asertuje komunikat `"select exactly one network feature"`. Status: **zamknięte**.

**4. README EN + PL** — **Potwierdzone.** Goły `anchor build` zdegradowany do „lokalny development, nigdy deploy"; release wyłącznie przez `scripts/build-*.sh`; liczby testów 24/24 i 4/4; opisane feature'y sieci + compile-time blokady. Status: **zamknięte**.

**Wniosek (a):** Wszystkie ustalenia przedtestnetowe z poprzedniej rundy uznaję za **zamknięte** na podstawie weryfikacji w źródłach.

## (b) Triaż supply-chain — 8 wyjątków RUSTSEC

| ID | Crate | Charakter | Ocena dla zamkniętego testnetu |
|----|-------|-----------|--------------------------------|
| RUSTSEC-2022-0093 | ed25519-dalek 1.0.1 | signing oracle | **Akceptowalny** — tylko SDK/testy, poza SBF |
| RUSTSEC-2024-0344 | curve25519-dalek 3.2.1 | timing | **Akceptowalny** — operacje sekretne poza programem |
| RUSTSEC-2025-0009 | ring 0.16.20 | AES panic | **Akceptowalny** — TLS klienta RPC |
| RUSTSEC-2026-0037 | quinn-proto 0.10.6 | DoS QUIC (high) | **Akceptowalny** — klient sieciowy |
| RUSTSEC-2026-0185 | quinn-proto 0.10.6 | memory exhaustion | **Akceptowalny** — jw. |
| RUSTSEC-2026-0098/0099/0104 | rustls-webpki 0.101.7 | cert validation | **Akceptowalny** — TLS klienta |

**Werdykt (b):** Żaden z 8 wyjątków **nie jest nieakceptowalny** dla fazy zamkniętego testnetu. Uzasadnienie: artefakt SBF nie wykonuje operacji kluczem prywatnym ani połączeń TLS/QUIC; ryzyko ogranicza się do narzędzi deweloperskich i testów. Warunki akceptacji: wyjątki per-ID (nowe advisories nadal blokują CI); data przeglądu 2026-10-19 zapisana; upgrade stosu Solana/Anchor z pustą listą wyjątków w Definition of Done mainnetu.

## (c) Przegląd `docs/SECURITY-AUDITS.md`

Dokument jest **rzetelny i zgodny** z przebiegiem rund #1–#3 oraz weryfikacjami: chronologia, tabela, statusy i dowody plik:linia odpowiadają faktom; jasno rozróżnia ustalenia kodowe od procesowych; DoD jest konkretne. Drobna uwaga redakcyjna (nie blokująca): sformułowanie o warunkowym werdykcie Groka można po dzisiejszej weryfikacji zaktualizować.

## (d) `declare_id!` per sieć

**Warto przed testnetem, nie dopiero przed mainnetem.** Rekomendacja (niski koszt, wysoka wartość):

```rust
#[cfg(feature = "network-mainnet")]
declare_id!("MAINNET_PROGRAM_ID_HERE");
#[cfg(feature = "network-testnet")]
declare_id!("TESTNET_PROGRAM_ID_HERE");
#[cfg(not(any(feature = "network-mainnet", feature = "network-testnet")))]
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"); // dev only
```

Uniemożliwia przypadkowe wdrożenie artefaktu deweloperskiego lub testnetowego na mainnet (inne PDA i Program ID). Zalecane przed testnetem, nie twardy bloker.

## (e) Kontrola regresji skryptów

`audit-evidence.sh`: `run` + `pipefail` + `tee` — poprawne; subshell `( cd core && run ... )` — błąd propaguje się; `expect_fail` solidny. Skrypty build: bramka `git status --porcelain` silniejsza niż `git diff`. **Brak luk** pozwalających na ciche przejście błędnego biegu.

## (f) Zaktualizowany werdykt

**Zamknięty testnet:** **Gotowy bezwarunkowo** (przy osobnym Program ID testnetowym, monitoringu i jasnej komunikacji o modelu epokowym).

**Immutable mainnet — Definition of Done:** finalny Program ID + `anchor keys sync` + `declare_id!` per sieć; `anchor build --verifiable` + `anchor verify`; pełny bieg `audit-evidence.sh` → `TEST-LOG.txt` + `.sha256` z HEAD; wszystkie testy zielone na 1.89; negatywne blokady nadal nie kompilują się; clippy `-D warnings` czysty; audit+deny z **pustymi listami wyjątków** (po upgrade stosu) lub świadomą decyzją; manifest release; upgrade authority aktywna do końca listy — dopiero potem `--final`.

**Podsumowanie:** Kod i pipeline są w najlepszym stanie od początku audytów. Rdzeń bezpieczeństwa (checkpointy + ochrona przed fundingiem epoki > `end_epoch`) nienaruszony. Zamknięty testnet można rozpoczynać.
