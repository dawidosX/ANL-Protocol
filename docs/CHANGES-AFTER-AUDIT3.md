# Zmiany po audycie #3 — dowody diff

Zakres: trzy realne ustalenia kodowe z raportu 6,8/10 + hardening jakości
(fmt/clippy) wg checklisty weryfikacyjnej. Zero zmian w logice protokołu.

## M-01 — `ACCOUNT_VERSION` dla obu `PoolConfig` w `FundXnt` ✅

`programs/anl_staking/src/instructions/fund.rs`:

```diff
         bump = genesis_pool.bump,
-        constraint = genesis_pool.pool_type == PoolType::Genesis @ AnlError::InvalidVault
+        constraint = genesis_pool.pool_type == PoolType::Genesis @ AnlError::InvalidVault,
+        constraint = genesis_pool.version == ACCOUNT_VERSION @ AnlError::InvalidAccountVersion
     )]
     pub genesis_pool: Account<'info, PoolConfig>,
```
(analogicznie `flexible_pool`)

## L-01 — jawny owner check dla checkpointów ✅

`lifecycle.rs::settlement_cap_index`:
```diff
     let info = ai.to_account_info();
+    require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
     let ck = XntCheckpoint::try_deserialize(&mut &info.data.borrow()[..])?;
```
`fund.rs::roll_checkpoint`:
```diff
         require_keys_eq!(prev_ai.key(), pda, AnlError::CheckpointMismatch);
         let info = prev_ai.to_account_info();
+        require_keys_eq!(*info.owner, *program_id, AnlError::CheckpointMismatch);
```

## H-01 — twarda blokada `test-periods` na mainnet ✅

`programs/anl_staking/Cargo.toml`:
```diff
 [features]
+network-mainnet = []
+network-testnet = []
 test-periods = ["anl-math/test-periods"]
```
`lib.rs`:
```diff
+#[cfg(all(feature = "network-mainnet", feature = "test-periods"))]
+compile_error!("test-periods cannot be enabled together with network-mainnet");
+#[cfg(all(feature = "network-mainnet", feature = "network-testnet"))]
+compile_error!("select exactly one network feature");
```
Dowód negatywny (oczekiwany sukces testu negatywnego, NIE awaria pipeline'u):
`cargo check -p anl_staking --features "network-mainnet,test-periods"` →
`error: test-periods cannot be enabled together with network-mainnet`.

## Polityka feature'ów sieci (świadoma decyzja)

Trzeciej blokady (`compile_error!` przy braku JAKIEJKOLWIEK sieci) **nie
dodajemy**: wymusiłaby feature w każdym `cargo test`/`clippy`/IDE i zepsuła
domyślne pipeline'y, nie podnosząc bezpieczeństwa. Gwarancje bez niej:
- build domyślny (bez feature) = deweloperski; release wyłącznie przez
  `scripts/build-mainnet.sh` / `scripts/build-testnet.sh` z jawnym zestawem
  feature'ów + manifestem (HEAD, features, sha256 binarki);
- kontrola `EXPECTED_XNT_MINT` jest pod `#[cfg(not(feature="test-periods"))]`,
  a `network-mainnet` ⟹ ¬`test-periods` (compile_error) ⟹ **na mainnecie
  kontrola minta jest zawsze aktywna** — niezależnie od innych feature'ów;
- CI (`release-guards`) trwale weryfikuje oba dowody negatywne.

## Hardening jakości (checklist weryfikacji)

- `cargo fmt --all` zastosowany; `--check` czysty.
- `cargo clippy --workspace --all-targets -- -D warnings` czysty w OBU
  wariantach. Poprawki: `epoch_of` → `Option<u64>` (result_unit_err);
  usunięta kolizja glob-reeksportów przez unikalne nazwy handlerów
  (`initialize_handler`/`create_pool_handler`/`stake_handler` — nazwy
  instrukcji on-chain BEZ zmian, definiuje je moduł `#[program]`);
  `#![allow(unexpected_cfgs)]` dla cfg emitowanych przez makra anchor 0.29;
  testy: dead_code + bool_assert_comparison.
- CI rozbudowany: fmt, clippy -D warnings (oba warianty), integracja
  PRODUKCYJNA, testy negatywne blokad, cargo audit + cargo deny.
- `scripts/audit-evidence.sh` — pełny bieg dowodowy na czystym drzewie git,
  zapisuje `docs/TEST-LOG.txt` powiązany z `git rev-parse HEAD`.

## Statusy (uczciwe, wg kryteriów GPT)

| Ustalenie | Status |
|---|---|
| M-01 wersje pul w FundXnt | naprawione, testy zaliczone |
| H-01 mainnet+test-periods | naprawione, negatywny build NIE kompiluje się |
| L-01 owner check | naprawione |
| M-02 dokumentacja | naprawione; repo-grep bez starych sformułowań |
| Program ID / verifiable build | otwarte zadania WDROŻENIOWE |
| Model checkpointów | statycznie obroniony; dynamiczne potwierdzenie = `scripts/audit-evidence.sh` na finalnym commicie |
