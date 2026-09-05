# MAINNET RUNBOOK — szkic (po freeze v1.0-testnet, 2026-09-05)

Baza: `src_tree 4c2256398137bb417a1b769316137852d14ec4d5` (tag `v1.0-testnet-freeze`). Każdy krok zmieniający kod
tworzy nowy `src_tree` i wymaga mini-rundy audytu (branch + raport dla Recenzenta) **przed** buildem ceremonii.
Źródła: raporty R7.2 (Kimi, B, C), R7.1 (handoff C), R6 (Kimi Info).

## 0. Backlog z raportów R7.2 (co trzeba dobudować / zdecydować)

| # | Zadanie | Źródło | Typ |
|---|---|---|---|
| M-1 | `scripts/build-mainnet.sh` + `release-manifest-mainnet.txt` w **tej samej pętli fail-closed** co testnet (`pack-audit.sh` czyta manifest mainnet, gdy pakuje release mainnet) | Kimi (Uwaga B), C | skrypty |
| M-2 | Krok **„dump on-chain == sha manifestu” w skrypcie** (`scripts/verify-deploy.sh`: `solana program dump` → `head -c <rozmiar>` → sha256 == manifest; wynik dopisywany do manifestu automatycznie, nie ręczna notatka) | Kimi, C (nie mogli zweryfikować dumpu) | skrypty |
| M-3 | `cargo_lock_sha` i `root_cargo_toml_sha` (+ sha `programs/anl_staking/Cargo.toml`) w manifeście release | C | skrypty |
| M-4 | **Rebuild na drugiej maszynie** przed ceremonią (inny host, ten sam platform-tools) — sha `.so` musi się zgadzać; wynik do manifestu (`rebuild_host_sha`) | Kimi | procedura |
| M-5 | `sweep_excess` dla multisiga (wypłata nadwyżki `reward_vault` ponad `anl_reward_reserved` + `ANL_REWARD_POOL − total_anl_paid`) — **decyzja produktowa**; jeśli TAK: nowa instrukcja = nowy `src_tree` = runda audytu | Kimi (Info) | decyzja / kod |
| M-6 | Upgrade stosu Solana/Anchor (opróżnia `deny.toml`/`audit.toml`; `Cargo.lock` v3 → zgodnie z nowym platform-tools) — **przed** immutable | wszyscy | kod / supply-chain |
| M-7 | Multisig ≥ 2/3 + timelock jako upgrade authority; rozdział ról; operator tylko `fund_xnt`; alerty TG (pause/set_operator/upgrade/fund_rewards) | F-02 | klucze / ops |
| M-8 | WP: cooldown 3 d, Flexible ≤ 365 d, limit 200M, orphan-policy, pin `initialize`, SLA bota | Recenzent | produkt |
| M-9 | Bug bounty przed otwarciem; wyłączenie bypassu status-checków na `main` | Recenzent | proces |

## 1. Ceremonia — kolejność (jeden commit kodu, jeden build, jeden deploy)

1. **Branch `release/mainnet`**: `EXPECTED_INIT_AUTHORITY` = pubkey multisiga **i** strażnik `init_authority_pinned_mainnet`
   w tym samym commicie; jeśli nowy Program ID — `declare_id` pod `network-mainnet` **i** strażnik `program_id_pinned_mainnet`.
   Gates: lib (`--features network-mainnet`), integracja ×2, core ×2, clippy ×3, fmt, `cargo audit`. Raport dla Recenzenta → OK → merge.
2. **Evidence** (`audit-evidence.sh`): TEST-LOG z `HEAD`, `src_tree`, `code_tree`, `math_tree`; commit.
3. **Build** (`build-mainnet.sh`, M-1): `cargo build-sbf --features network-mainnet` (bez `test-periods`; `compile_error!` pilnuje),
   manifest mainnet: `head`, `src_tree`, `code_tree`, `math_tree`, `sha256`, `build_sbf`, `cargo_lock_sha`, `root_cargo_toml_sha` (M-3).
4. **Rebuild na drugiej maszynie** (M-4) → identyczny sha albo STOP.
5. **Tokeny (przed `initialize`):** ANL — fixed supply (mint authority None, decimals 9); CAPY — mint **dokładnie** 20 000 000 × 10⁹
   → wypalenie mint authority (i brak freeze authority). `init_capy_vault` odrzuci inną podaż (`InvalidCapySupply`).
6. **Deploy (Dawid):** `solana program deploy` z upgrade authority = klucz ceremonii → natychmiast `set-upgrade-authority` na multisig.
7. **Weryfikacja deployu w skrypcie (M-2):** dump == sha manifestu; slot → manifest; commit `build: deploy mainnet sha … slot …`.
8. **Inicjalizacja (multisig, jedna sesja):** `initialize(genesis_start_ts, start_paused=true)` przez `EXPECTED_INIT_AUTHORITY` →
   `init_principal_vault`, `init_reward_vault`, `init_xnt_vault`, `init_capy_vault` → `create_pool` ×2 → `set_operator` (hot key bota) →
   `fund_rewards` 200 000 000 ANL → weryfikacja `GlobalConfig` (authority, minty, `capy_mint`) → `resume`.
9. **Provenance:** `pack-audit.sh mainnet` (fail-closed na manifest mainnet), `git archive`, `git bundle`, tag `v1.0-mainnet`.
10. **Obserwacja ≥ 45 dni** (bot `close_day → settle_expired` codziennie, `audyt-naliczen.js` codziennie, alerty) →
    upgrade stosu (M-6) → mini-runda → `set-upgrade-authority --final` (immutable).

## 2. Kryteria STOP
- rozjazd sha między buildem a rebuildem na drugiej maszynie;
- dump on-chain ≠ sha manifestu;
- `GlobalConfig.authority` ≠ multisig po `initialize`;
- podaż CAPY ≠ 20M lub aktywna mint/freeze authority;
- jakikolwiek czerwony krok evidence lub `cargo audit` z nową podatnością w grafie SBF (`cargo +solana tree --target sbf-solana-solana`).
