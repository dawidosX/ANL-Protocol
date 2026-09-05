#!/usr/bin/env bash
# Paczka audytowa z poprawnym provenance: manifest OBOK zipa (sha zipa liczone po zapisie).
# Uzycie: ./scripts/pack-audit.sh r6  -> ~/Downloads/ANL-Protocol-audyt-r6-<head>.zip + .manifest.txt
set -euo pipefail
TAG="${1:-audit}"
cd "$(git rev-parse --show-toplevel)"
if [ -n "$(git status --porcelain)" ]; then echo "BLAD: drzewo nie jest czyste" >&2; git status --short >&2; exit 1; fi
HEAD_FULL=$(git rev-parse HEAD); HEAD=$(git rev-parse --short HEAD)
# R7.2 (B): provenance FAIL-CLOSED - manifest musi opisywac DOKLADNIE ten kod i te binarke.
MAN=release-manifest-testnet.txt; BIN=target/deploy/anl_staking.so
[ -f "$MAN" ] || { echo "BLAD provenance: brak $MAN (uruchom build-testnet.sh)" >&2; exit 1; }
[ -f "$BIN" ] || { echo "BLAD provenance: brak $BIN (uruchom build-testnet.sh)" >&2; exit 1; }
prov_check() { # nazwa, wartosc z manifestu, wartosc rzeczywista
  if [ -z "$2" ]; then echo "BLAD provenance: manifest nie zawiera pola $1" >&2; exit 1; fi
  if [ "$2" != "$3" ]; then echo "BLAD provenance: $1 nie zgadza sie: manifest=$2 aktualne=$3" >&2; exit 1; fi
}
mf() { sed -n "s/^$1: //p" "$MAN" | head -1; }
prov_check src_tree  "$(mf src_tree)"  "$(git rev-parse HEAD:programs/anl_staking/src)"
prov_check code_tree "$(mf code_tree)" "$(git rev-parse HEAD:programs)"
prov_check math_tree "$(mf math_tree)" "$(git rev-parse HEAD:crates/anl-math)"
prov_check sha256    "$(mf sha256)"    "$(shasum -a 256 "$BIN" | cut -d' ' -f1)"
echo "provenance OK: src_tree/code_tree/math_tree/sha256 zgodne z HEAD i binarka"
ZIP="$HOME/Downloads/ANL-Protocol-audyt-${TAG}-${HEAD}.zip"; MAN="${ZIP%.zip}.manifest.txt"
rm -f "$ZIP" "$MAN"
zip -r -q "$ZIP" programs/anl_staking/src programs/anl_staking/tests programs/anl_staking/Cargo.toml \
  crates/anl-math docs/AUDIT-BRIEF-round5.md docs/CHANGES-AFTER-ROUND4.md docs/CHANGES-AFTER-ROUND5.md \
  docs/CHANGES-AFTER-ROUND6.md docs/CHANGES-AFTER-ROUND7.md docs/AUDIT-BRIEF-round7.md docs/AUDIT-NOTE-round7.2.md \
  docs/audits docs/TEST-LOG.txt docs/TEST-LOG.sha256 docs/ops deny.toml .cargo/audit.toml \
  scripts/audyt-naliczen.js scripts/diagnoza-user2.js scripts/audit-evidence.sh scripts/build-testnet.sh scripts/pack-audit.sh \
  release-manifest-testnet.txt Cargo.toml Cargo.lock Anchor.toml README.md .github/workflows/ci.yml \
  -x "*/target/*" "*/node_modules/*" "*.DS_Store"
sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
{ echo "package:        $(basename "$ZIP")"; echo "commit:         $HEAD_FULL"; echo "commit_short:   $HEAD"
  echo "date_utc:       $(date -u +%Y-%m-%dT%H:%M:%SZ)"; echo "cargo_lock_sha: $(sha Cargo.lock)"
  echo "zip_sha256:     $(sha "$ZIP")"
  if [ -f target/deploy/anl_staking.so ]; then echo "so_sha256:      $(sha target/deploy/anl_staking.so)"; else echo "so_sha256:      (brak - uruchom build-testnet.sh)"; fi
  [ -f release-manifest-testnet.txt ] && { echo "--- release-manifest-testnet.txt ---"; cat release-manifest-testnet.txt; }
} > "$MAN"
echo OK; ls -la "$ZIP" "$MAN"; cat "$MAN"
