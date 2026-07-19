#!/usr/bin/env bash
# Produkcyjny build MUSI byc jawny: network-mainnet, bez test-periods.
# Zapisuje manifest release (features + HEAD + hash binarki).
# V-03: czystosc drzewa przez git status --porcelain (lapie staged i untracked).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
if [ -n "$(git status --porcelain)" ]; then
  echo "BLAD: drzewo robocze nie jest czyste (zmiany staged/untracked tez sie licza)." >&2
  git status --porcelain >&2
  exit 1
fi
FEATURES="network-mainnet"
anchor build -- --features "$FEATURES"
BIN=target/deploy/anl_staking.so
{
  echo "release: mainnet"
  echo "date: $(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  echo "head: $(git rev-parse HEAD)"
  echo "features: $FEATURES"
  echo "sha256: $(sha256sum "$BIN" | cut -d' ' -f1)"
  echo "rustc: $(rustc --version)"
} | tee release-manifest-mainnet.txt
echo "Przed deployem: porownaj sha256 wdrazanej binarki z manifestem."
