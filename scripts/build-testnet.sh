#!/usr/bin/env bash
set -euo pipefail
FEATURES="network-testnet,test-periods"
anchor build -- --features "$FEATURES"
BIN=target/deploy/anl_staking.so
{
  echo "release: testnet"
  echo "date: $(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  echo "head: $(git rev-parse HEAD)"
  echo "features: $FEATURES"
  echo "sha256: $(sha256sum "$BIN" | cut -d' ' -f1)"
} | tee release-manifest-testnet.txt
