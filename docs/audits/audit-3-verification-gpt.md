# Weryfikacja finalnej paczki po poprawkach (źródło, nie deklaracje)
*Raport weryfikacyjny GPT · 19.07.2026 · paczka: `anl-protocol (2).zip` · zarchiwizowany w repo bez zmian treści (formatowanie: markdown)*

Przeanalizowałem dokładnie pliki z `anl-protocol (2).zip`. Oto wyniki punkt po punkcie.

## 1. M-01 — version check na PoolConfig w FundXnt

Potwierdzone w źródle.

```
:129  constraint = genesis_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
:138  constraint = flexible_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
```

Oba konta PoolConfig w FundXnt mają teraz pełny constraint wersji. **Status: zamknięte.**

## 2. H-01 — twarda blokada test-periods na mainnet

Potwierdzone w źródle i w dowodzie negatywnym.

`Cargo.toml`:
```toml
network-mainnet = []
network-testnet = []
test-periods = ["anl-math/test-periods"]
```

`lib.rs`:
```rust
#[cfg(all(feature = "network-mainnet", feature = "test-periods"))]
compile_error!("test-periods cannot be enabled together with network-mainnet");
#[cfg(all(feature = "network-mainnet", feature = "network-testnet"))]
compile_error!("select exactly one network feature");
```

`docs/TEST-LOG.txt` zawiera surowy wynik:
```
$ cargo check -p anl_staking --features network-mainnet,test-periods
error: test-periods cannot be enabled together with network-mainnet
```

CI job release-guards trwale pilnuje obu negatywnych przypadków. **Status: zamknięte.**

## 3. L-01 — owner check przed deserializacją checkpointu

Potwierdzone w obu miejscach.

`lifecycle.rs` (`settlement_cap_index`), linia ~71 (przed `try_deserialize`):
```rust
require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
```

`fund.rs` (`roll_checkpoint`), linia ~207 (przed `try_deserialize`):
```rust
require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
```

**Status: zamknięte.**

## 4. M-02 — dokumentacja end_ts / end_epoch + liczby testów

Czysto.

- `README.md:19` i `README.pl.md:19` jednoznacznie mówią: ANL kończy się na dokładnym `end_ts`, XNT rozlicza się epokowo do pełnej `end_epoch`.
- Blockquote w obu językach powtarza regułę `end_epoch = epoch_of(end_ts − 1)`.
- Liczby testów zsynchronizowane: 24/24 (anl-math) i 4/4 (integration).
- Brak starych sformułowań typu „naliczanie obu strumieni staje na end_ts" w kontekście XNT.

**Status: zamknięte.**

## 5. Zmiany wymuszone przez clippy — czy wprowadziły regresję?

Nie.

- Zmiana `epoch_of → Option<u64>` jest poprawnie obsłużona w obu call-site'ach (`stake.rs:195-196` i `fund.rs:233-234`) przez `.ok_or(AnlError::BeforeGenesis)`.
- Zmiana nazw wewnętrznych handlerów (`initialize_handler`, `stake_handler`, `create_pool_handler`) nie zmienia interfejsu on-chain. W `#[program]` nadal eksponowane są oryginalne nazwy instrukcji: `initialize`, `create_pool`, `stake`, `fund_xnt`, `settle_expired`, `claim`, `unstake_early` itd. ABI pozostaje identyczne.
- `cargo fmt --check` i clippy `-D warnings` (oba warianty) przechodzą według `TEST-LOG.txt`.

**Brak regresji.**

## Odpowiedzi na pytania

**(a)** Tak — po weryfikacji w źródle podtrzymuję status **zamknięte** dla M-01, H-01 i L-01.

**(b)** Zmiany clippy nie wprowadziły regresji. Interfejs instrukcji on-chain jest nienaruszony.

**(c)** Zaktualizowany, konkretny definition-of-done przed immutable mainnetem:

### Definition of Done — Immutable Mainnet

1. Finalny Program ID (`anchor keys sync` + aktualizacja `declare_id!` + `Anchor.toml` + rebuild + weryfikacja wszystkich PDA).
2. `anchor build --verifiable` + `anchor verify` na X1 (potwierdzenie semantyki loadera).
3. Pełny bieg `scripts/audit-evidence.sh` na czystym drzewie git po powyższych zmianach — wynik zapisany w `docs/TEST-LOG.txt` powiązanym z `git rev-parse HEAD`.
4. Wszystkie testy (anl-math 24, core 34, integration 4 w obu wariantach) zielone na toolchainie 1.89.
5. Negatywne testy blokad (`network-mainnet`+`test-periods` oraz `network-mainnet`+`network-testnet`) nadal nie kompilują się.
6. `cargo clippy --workspace --all-targets -- -D warnings` czysty w obu wariantach.
7. `cargo audit` i `cargo deny check` bez krytycznych/high findings (job supply-chain w CI).
8. Manifest release wygenerowany przez `scripts/build-mainnet.sh` (HEAD + features + sha256 binarki + rustc version) dołączony do release notes.
9. Upgrade authority pozostaje aktywna do czasu pozytywnego wyniku punktów 1–8; dopiero potem `--final`.

## Werdykt po tej weryfikacji

- **Testnet / zamknięty pilot: gotowy** (przy zachowaniu osobnego Program ID, limitowanej wartości aktywów i monitoringu).
- **Immutable mainnet: gotowy po spełnieniu powyższej zamkniętej listy 9 punktów.** Nie ma już otwartych ustaleń kodowych z rundy #3, które blokowałyby immutable.

Rdzeń bezpieczeństwa (model checkpointów + ochrona przed fundingiem epoki > `end_epoch`) pozostaje nienaruszony i jest teraz wzmocniony dodatkowymi checkami owner/version oraz twardą blokadą feature'ów.
