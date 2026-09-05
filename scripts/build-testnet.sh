#!/usr/bin/env bash
# Build TESTNETOWY: network-testnet + test-periods. Osobny Program ID.
# V-03: czystosc drzewa przez git status --porcelain (lapie staged i untracked).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if [ -n "$(git status --porcelain)" ]; then
  echo "BLAD: drzewo robocze nie jest czyste (zmiany staged/untracked tez sie licza)." >&2
  git status --porcelain >&2
  exit 1
fi
FEATURES="network-testnet,test-periods"
BIN=target/deploy/anl_staking.so
rm -f "$BIN"   # runda #4: zero ryzyka starego artefaktu
anchor build --no-idl -- --features "$FEATURES"
test -s "$BIN" || { echo "BLAD: brak binarki po buildzie." >&2; exit 1; }
{
  echo "release: testnet"
  echo "date: $(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  echo "head: $(git rev-parse HEAD)"
  # R7: hash drzewa kodu (niezalezny od commitow docs) — to on odpowiada sha binarki
  echo "code_tree: $(git rev-parse HEAD:programs)"
  echo "math_tree: $(git rev-parse HEAD:crates/anl-math)"
  # R7.1: src_tree = wylacznie kod on-chain (bez testow) - identyfikuje binarke
  echo "src_tree: $(git rev-parse HEAD:programs/anl_staking/src)"
  echo "features: $FEATURES"
  echo "sha256: $(sha256sum "$BIN" | cut -d' ' -f1)"
  echo "rustc_host: $(rustc --version)"
  # R6 I-04: binarke SBF kompiluje rustc z platform-tools, nie rustc hosta
  echo "build_sbf: $(cargo build-sbf --version 2>&1 | tr '\n' ' ' | sed 's/  */ /g')"
} | tee release-manifest-testnet.txt
echo "Przed deployem: porownaj sha256 wdrazanej binarki z manifestem."
