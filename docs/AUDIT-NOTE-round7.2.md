# AUDIT NOTE — R7.2 (release path), 2026-09-05

Trzy poprawki domykające ścieżkę wydania. **Logika ekonomiczna (XNT, ANL, CAPY entitlement) bez zmian**
względem R7.1; ścieżka wykonania na testnecie różni się wyłącznie obecnością nowego kodu błędu.

| # | Zmiana | Plik | Cel |
|---|---|---|---|
| A | Build **bez** feature sieciowego ma osobny Program ID `ChG81WApHgpbWjt4r8wmJ57WS3MkXyMGzJ2tBbC8pcov` (keypair poza repo). Strażniki `program_id_pinned_{testnet,mainnet}` (= `4Cpx…`, ≠ testowe) i `program_id_is_test_id`. | `lib.rs` | binarka bez pinu `initialize` i bez bramek mainnetu nie uruchomi się pod adresem produkcyjnym (`DeclaredProgramIdMismatch`) |
| B | `pack-audit.sh` **fail-closed**: `src_tree`/`code_tree`/`math_tree`/`sha256` z manifestu muszą równać się HEAD i binarce, inaczej `exit 1` z nazwą hasza. `build-testnet.sh` wypisuje rozjazd jako ostrzeżenie. | `scripts/` | paczka audytowa nie może opisywać innego kodu niż ten, który zawiera |
| C | `init_capy_vault` pod `network-mainnet` wymaga `supply == CAPY_TOTAL_SUPPLY` (20 000 000 × 10⁹); nowy błąd `InvalidCapySupply` (kod **6046**, koniec enumu). Strażnik `capy_supply_constant`. Testnet (1B CAPY) bez bramki. | `constants.rs`, `initialize.rs`, `errors.rs` | tokenomika WP jako inwariant on-chain (authority wypalona ⇒ kontrola jednorazowa wystarcza) |

`Anchor.toml`: `localnet` = testowe ID, `testnet` = `4Cpx…` (usunięty efemeryczny `49vh…`). Układ kont instrukcji bez zmian;
istniejące kody błędów bez zmian.

## Provenance R7.2

| Element | Wartość |
|---|---|
| HEAD (merge) | `544b12a` — evidence `docs/TEST-LOG.txt` (52/52 oba reżimy, lib 13/13, core, clippy ×2, fmt, `cargo audit` 0 podatności) |
| `src_tree` (`HEAD:programs/anl_staking/src`) | `4c2256398137bb417a1b769316137852d14ec4d5` |
| `code_tree` (`HEAD:programs`) | `7dbf3de415685767654ad8e068f6034e27e51e53` |
| `math_tree` (`HEAD:crates/anl-math`) | `6fb61151f3e10b0a5d68249a941721500b34a5b3` (bez zmian od R6) |
| Binarka `.so` | sha256 `87b431d43280e4eccfca71725bbfafda2f1fbd2fb2d95a94cd2715fd4ae530a3`, 676 536 B, platform-tools v1.41 |
| Deploy testnet | slot **185899744**, program `4Cpxg8U3pQWzjMYmoyQgjep9UcMw4DtK7V5tYhmHTVRM`; dump on-chain == binarka (0 bajtów różnicy, weryfikacja Claude Code i Recenzenta) |
| Po deployu | `audyt-naliczen.js`: 7 portfeli, 139 pozycji, 1 znana flaga (`3sva…#11` APY, artefakt) |

Dodatkowe strażniki lokalne: lib 15/15 pod `network-mainnet`, 14/14 pod `network-testnet,test-periods`, clippy `-D warnings`
czysty w trzech wariantach feature. Pytanie (h) w `docs/AUDIT-BRIEF-round7.md`.
