#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG_ROOT="$ROOT/target/security"

rm -rf "$LOG_ROOT"
mkdir -p "$LOG_ROOT"

run_step() {
  local name="$1"
  shift
  local log="$LOG_ROOT/${name}.log"

  printf '\n==> %s\n' "$name"
  (
    printf '+'
    printf ' %q' "$@"
    printf '\n'
    "$@"
  ) 2>&1 | tee "$log"
}

run_step 01-cargo-test cargo test --manifest-path "$ROOT/safe/Cargo.toml" --release
run_step 02-build-safe bash "$ROOT/safe/scripts/build-safe.sh" --release
run_step 03-check-abi bash "$ROOT/safe/scripts/check-abi.sh" --strict
run_step 04-link-original bash "$ROOT/safe/scripts/link-original-tests.sh" --all
run_step 05-build-original-cli bash "$ROOT/safe/scripts/build-original-cli-against-safe.sh" --run-samples
run_step 06-build-debs bash "$ROOT/safe/scripts/build-debs.sh"
run_step 07-check-package-layout bash "$ROOT/safe/scripts/check-package-layout.sh"
run_step 08-run-debian-tests bash "$ROOT/safe/scripts/run-debian-tests.sh" --tests link-with-shared bigfile bzexe-test compare compress grep
run_step 09-test-original "$ROOT/test-original.sh"
run_step 10-benchmark env LIBBZ2_BENCH_CAPTURE_SECURITY_LOG=0 bash "$ROOT/safe/scripts/benchmark-compare.sh"

{
  printf 'release_gate=impl_06_final_hardening_and_release_gate\n'
  printf 'git_head=%s\n' "$(git -C "$ROOT" rev-parse HEAD)"
  printf 'generated_at_utc=%s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
  printf 'log_root=target/security\n'
  printf 'benchmark_summary=target/bench/summary.txt\n'
  for log in "$LOG_ROOT"/*.log; do
    printf 'log=%s\n' "$(basename "$log")"
  done
} > "$LOG_ROOT/summary.txt"
