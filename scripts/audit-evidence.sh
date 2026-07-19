#!/usr/bin/env bash
# Dowody dla audytu (GPT pkt 1-3): uruchom W REPO GIT na czystym drzewie.
# Wynik: docs/TEST-LOG.txt kryptograficznie powiazany z HEAD.
set -uo pipefail
LOG=docs/TEST-LOG.txt
run(){ echo "\$ $*" >>"$LOG"; "$@" >>"$LOG" 2>&1; echo "exit: $?" >>"$LOG"; echo >>"$LOG"; }

{
  echo "ANL Protocol — dowody audytowe"
  echo "Wygenerowano: $(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  echo "rustc: $(rustc --version)"
  echo "cargo: $(cargo --version)"
  echo "========================================================================"
} > "$LOG"

echo "## Tozsamosc zrodla" >>"$LOG"
run git rev-parse HEAD
run git status --porcelain
run git diff --check

echo "## Format i lint" >>"$LOG"
run cargo fmt --all --check
run cargo clippy --workspace --all-targets -- -D warnings
run cargo clippy -p anl_staking --all-targets --features test-periods -- -D warnings

echo "## Testy" >>"$LOG"
run cargo test -p anl-math
run cargo test -p anl-math --features test-periods
( cd core && run cargo test )
run cargo test -p anl_staking --features test-periods --test integration
run cargo test -p anl_staking --test integration

echo "## NEGATYWNY dowod H-01 (oczekiwany BLAD kompilacji)" >>"$LOG"
echo "\$ cargo check -p anl_staking --features network-mainnet,test-periods" >>"$LOG"
if cargo check -p anl_staking --features "network-mainnet,test-periods" >>"$LOG" 2>&1; then
  echo "!!! BLAD: build sie skompilowal — blokada H-01 NIE dziala" >>"$LOG"
else
  echo "OK: oczekiwany sukces testu negatywnego (compile_error!)" >>"$LOG"
fi
echo >>"$LOG"
echo "\$ cargo check -p anl_staking --features network-mainnet,network-testnet" >>"$LOG"
if cargo check -p anl_staking --features "network-mainnet,network-testnet" >>"$LOG" 2>&1; then
  echo "!!! BLAD: obie sieci naraz sie kompiluja" >>"$LOG"
else
  echo "OK: sieci wzajemnie wykluczone" >>"$LOG"
fi
echo >>"$LOG"

echo "## Supply chain (jesli narzedzia zainstalowane)" >>"$LOG"
command -v cargo-audit >/dev/null && run cargo audit || echo "cargo-audit: niezainstalowane (patrz job supply-chain w CI)" >>"$LOG"
command -v cargo-deny  >/dev/null && run cargo deny check || echo "cargo-deny: niezainstalowane (patrz job supply-chain w CI)" >>"$LOG"

echo "GOTOWE -> $LOG"
