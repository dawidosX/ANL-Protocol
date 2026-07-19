#!/usr/bin/env bash
# Produkcyjny build MUSI byc jawny: network-mainnet, bez test-periods.
# Zapisuje manifest release (features + HEAD + hash binarki).
set -euo pipefail
git diff --quiet || { echo "Drzewo niezatwierdzone — commit przed release."; exit 1; }
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
