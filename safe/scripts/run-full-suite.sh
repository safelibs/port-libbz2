#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOG_ROOT="$ROOT/target/security"
COMPAT="$ROOT/target/compat"
BASELINE="$ROOT/target/original-baseline"
CC_BIN="${CC:-gcc}"

rm -rf "$LOG_ROOT"
mkdir -p "$LOG_ROOT"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || die "missing required file: $1"
}

repo_relative() {
  local path="$1"

  if [[ "$path" == "$ROOT/"* ]]; then
    printf '%s\n' "${path#$ROOT/}"
  else
    printf '%s\n' "$path"
  fi
}

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

run_direct_dlltest_object_gate() {
  local shared_object="$COMPAT/libbz2.so.1.0.4"
  local dlltest_exe="$COMPAT/dlltest-object-release-gate"
  local path_bz2="$BASELINE/dlltest-path.bz2"
  local path_out="$BASELINE/dlltest-path.out"
  local stdio_bz2="$BASELINE/dlltest-stdio.bz2"
  local stdio_out="$BASELINE/dlltest-stdio.out"
  local tmpdir path_bz2_rel path_out_rel stdio_bz2_rel stdio_out_rel tmpdir_rel
  local status=0

  require_file "$shared_object"
  require_file "$ROOT/target/original-baseline/dlltest.o"
  require_file "$path_bz2"
  require_file "$path_out"
  require_file "$stdio_bz2"
  require_file "$stdio_out"

  mkdir -p "$ROOT/target"
  tmpdir="$(mktemp -d "$ROOT/target/release-dlltest-object.XXXXXX")"
  path_bz2_rel="$(repo_relative "$path_bz2")"
  path_out_rel="$(repo_relative "$path_out")"
  stdio_bz2_rel="$(repo_relative "$stdio_bz2")"
  stdio_out_rel="$(repo_relative "$stdio_out")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  "$CC_BIN" \
    -o "$dlltest_exe" \
    "$ROOT/target/original-baseline/dlltest.o" \
    -Wl,-rpath,'$ORIGIN' \
    "$ROOT/target/compat/libbz2.so.1.0.4"

  if (
    cd "$ROOT"
    env LD_LIBRARY_PATH="$COMPAT${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      "$dlltest_exe" -d "$path_bz2_rel" "$tmpdir_rel/path.out"
    cmp "$tmpdir_rel/path.out" "$path_out_rel"

    env LD_LIBRARY_PATH="$COMPAT${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      "$dlltest_exe" -d < "$stdio_bz2_rel" > "$tmpdir_rel/stdio.out"
    cmp "$tmpdir_rel/stdio.out" "$stdio_out_rel"

    env LD_LIBRARY_PATH="$COMPAT${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      "$dlltest_exe" "$path_out_rel" "$tmpdir_rel/path.bz2"
    cmp "$tmpdir_rel/path.bz2" "$path_bz2_rel"

    env LD_LIBRARY_PATH="$COMPAT${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
      "$dlltest_exe" -1 < "$stdio_out_rel" > "$tmpdir_rel/stdio.bz2"
    cmp "$tmpdir_rel/stdio.bz2" "$stdio_bz2_rel"
  ); then
    status=0
  else
    status=$?
  fi

  rm -rf "$tmpdir"
  return "$status"
}

run_step 01-cargo-test cargo test --manifest-path "$ROOT/safe/Cargo.toml" --release
run_step 02-build-safe bash "$ROOT/safe/scripts/build-safe.sh" --release
run_step 03-check-abi bash "$ROOT/safe/scripts/check-abi.sh" --strict
run_step 04-link-original bash "$ROOT/safe/scripts/link-original-tests.sh" --all
run_step 05-dlltest-object-release-gate run_direct_dlltest_object_gate
run_step 06-build-original-cli bash "$ROOT/safe/scripts/build-original-cli-against-safe.sh" --run-samples
run_step 07-build-debs bash "$ROOT/safe/scripts/build-debs.sh"
run_step 08-check-package-layout bash "$ROOT/safe/scripts/check-package-layout.sh"
run_step 09-run-debian-tests bash "$ROOT/safe/scripts/run-debian-tests.sh" --tests link-with-shared bigfile bzexe-test compare compress grep
run_step 10-test-original-all "$ROOT/test-original.sh"
run_step 11-test-original-libapt "$ROOT/test-original.sh" --only libapt-pkg6.0t64
run_step 12-test-original-bzip2 "$ROOT/test-original.sh" --only bzip2
run_step 13-test-original-python "$ROOT/test-original.sh" --only libpython3.12-stdlib
run_step 14-test-original-php "$ROOT/test-original.sh" --only php8.3-bz2
run_step 15-benchmark env LIBBZ2_BENCH_CAPTURE_SECURITY_LOG=0 bash "$ROOT/safe/scripts/benchmark-compare.sh"

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
