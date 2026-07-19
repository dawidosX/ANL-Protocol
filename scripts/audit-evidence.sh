#!/usr/bin/env bash
# Dowody dla audytu — FAIL-CLOSED (V-02 / M-EVIDENCE-02).
# Uruchamiaj W REPO GIT, na CZYSTYM drzewie. Kazdy nieudany krok przerywa
# caly bieg z niezerowym kodem wyjscia. Wynik: docs/TEST-LOG.txt powiazany
# z HEAD + docs/TEST-LOG.sha256 (hash logu). Jedyna zmiana drzewa po biegu
# to te dwa pliki — do zacommitowania jako dowod.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

# --- Bramka 1: czyste drzewo PRZED czymkolwiek (git status, nie git diff) ---
if [ -n "$(git status --porcelain)" ]; then
  echo "BLAD: drzewo robocze nie jest czyste — commit/stash przed biegiem dowodowym." >&2
  git status --porcelain >&2
  exit 1
fi
HEAD="$(git rev-parse HEAD)"

# --- Log do pliku TYMCZASOWEGO poza repo; do docs/ trafia dopiero na koncu ---
LOG="$(mktemp /tmp/anl-test-log.XXXXXX)"
trap 'echo "PRZERWANO — pelny log czesciowy: $LOG" >&2' ERR

run() {
  echo "\$ $*" | tee -a "$LOG"
  "$@" 2>&1 | tee -a "$LOG"   # set -e + pipefail: blad polecenia przerywa bieg
  echo | tee -a "$LOG"
}

# Test negatywny: wymagany NIEZEROWY kod wyjscia ORAZ konkretny komunikat.
expect_fail() {
  local msg="$1"; shift
  echo "\$ $* (oczekiwany BLAD: ${msg})" | tee -a "$LOG"
  local out code
  set +e; out="$("$@" 2>&1)"; code=$?; set -e
  echo "$out" | tail -5 | tee -a "$LOG"
  if [ "$code" -eq 0 ]; then
    echo "BLAD: polecenie sie powiodlo, a mialo sie nie skompilowac." | tee -a "$LOG" >&2
    exit 1
  fi
  if ! grep -qF "$msg" <<<"$out"; then
    echo "BLAD: kompilacja padla, ale bez oczekiwanego komunikatu: ${msg}" | tee -a "$LOG" >&2
    exit 1
  fi
  echo "OK: oczekiwany sukces testu negatywnego" | tee -a "$LOG"
  echo | tee -a "$LOG"
}

{
  echo "ANL Protocol — dowody audytowe (fail-closed)"
  echo "Wygenerowano: $(date -u +'%Y-%m-%dT%H:%M:%SZ')"
  echo "HEAD:  $HEAD"
  echo "rustc: $(rustc --version)"
  echo "cargo: $(cargo --version)"
  echo "========================================================================"
} | tee -a "$LOG"

echo "## Format i lint" | tee -a "$LOG"
run cargo fmt --all --check
run cargo clippy --workspace --all-targets -- -D warnings
run cargo clippy -p anl_staking --all-targets --features test-periods -- -D warnings

echo "## Testy" | tee -a "$LOG"
run cargo test -p anl-math
run cargo test -p anl-math --features test-periods
( cd core && run cargo test && run cargo test --features test-periods )
run cargo test -p anl_staking --features test-periods --test integration
run cargo test -p anl_staking --test integration

echo "## NEGATYWNE dowody blokad feature'ow (H-01)" | tee -a "$LOG"
expect_fail "test-periods cannot be enabled together with network-mainnet" \
  cargo check -p anl_staking --features "network-mainnet,test-periods"
expect_fail "select exactly one network feature" \
  cargo check -p anl_staking --features "network-mainnet,network-testnet"

echo "## Warianty pozytywne (bledny cfg nie moze blokowac wszystkiego)" | tee -a "$LOG"
run cargo check -p anl_staking --features network-mainnet
run cargo check -p anl_staking --features "network-testnet,test-periods"

echo "## Supply chain" | tee -a "$LOG"
if command -v cargo-audit >/dev/null; then run cargo audit
else echo "cargo-audit: niezainstalowane lokalnie — WIAZACY wynik daje job supply-chain w CI" | tee -a "$LOG"; fi
if command -v cargo-deny >/dev/null; then run cargo deny check advisories bans sources
else echo "cargo-deny: niezainstalowane lokalnie — WIAZACY wynik daje job supply-chain w CI" | tee -a "$LOG"; fi

# --- Stopka: powiazanie z HEAD + stan drzewa PO biegu ---
{
  echo "========================================================================"
  echo "## Podsumowanie biegu"
  echo "HEAD: $HEAD"
  echo "git status --porcelain po biegu (oczekiwane: puste):"
  git status --porcelain
  echo "WSZYSTKIE KROKI PRZESZLY."
} | tee -a "$LOG"

# --- Publikacja dowodu do repo + hash logu ---
mkdir -p docs
cp "$LOG" docs/TEST-LOG.txt
sha256sum docs/TEST-LOG.txt | tee docs/TEST-LOG.sha256
echo
echo "SUKCES. Dowod: docs/TEST-LOG.txt (+ docs/TEST-LOG.sha256), HEAD=$HEAD"
echo "Zacommituj oba pliki, aby dowod byl powiazany z historia:"
echo "  git add docs/TEST-LOG.txt docs/TEST-LOG.sha256 && git commit -m 'Audit evidence: run bound to '$HEAD"
