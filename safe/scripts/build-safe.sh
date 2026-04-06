#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MANIFEST="$ROOT/safe/Cargo.toml"
LOCKFILE="$ROOT/safe/Cargo.lock"
SAFE_HEADER="$ROOT/safe/include/bzlib.h"
ORIGINAL_HEADER="$ROOT/original/bzlib.h"
COMPAT="$ROOT/target/compat"
BASELINE="$ROOT/target/original-baseline"

profile="debug"
cargo_args=()
cc_bin="${CC:-gcc}"
shared_object=""

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_tool() {
  command -v "$1" >/dev/null 2>&1 || die "missing required host tool: $1"
}

require_file() {
  [[ -f "$1" ]] || die "missing required file: $1"
}

link_object_fixture() {
  local output="$1"
  local object="$2"
  "$cc_bin" \
    -o "$output" \
    "$object" \
    -Wl,-rpath,'$ORIGIN' \
    "$shared_object"
}

while (($# > 0)); do
  case "$1" in
    --release)
      profile="release"
      cargo_args+=(--release)
      ;;
    --debug)
      profile="debug"
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

require_file "$MANIFEST"
require_file "$LOCKFILE"
require_file "$SAFE_HEADER"
require_file "$ORIGINAL_HEADER"
require_tool "$cc_bin"
require_file "$BASELINE/public_api_test.o"
require_file "$BASELINE/bzip2.o"
require_file "$BASELINE/dlltest.o"

cmp -s "$SAFE_HEADER" "$ORIGINAL_HEADER" || die "safe/include/bzlib.h must match original/bzlib.h"

mkdir -p \
  "$ROOT/target/original-baseline" \
  "$ROOT/target/compat" \
  "$ROOT/target/install" \
  "$ROOT/target/package" \
  "$ROOT/target/bench" \
  "$ROOT/target/security" \
  "$COMPAT/include"

rm -rf "$COMPAT"
mkdir -p "$COMPAT/include"

export CARGO_TARGET_DIR="$COMPAT/cargo"

cargo build --locked --manifest-path "$MANIFEST" "${cargo_args[@]}"

artifact_dir="$CARGO_TARGET_DIR/$profile"
require_file "$artifact_dir/libbz2.so"
require_file "$artifact_dir/libbz2.a"

install -m 0755 "$artifact_dir/libbz2.so" "$COMPAT/libbz2.so.1.0.4"
ln -sfn libbz2.so.1.0.4 "$COMPAT/libbz2.so.1.0"
ln -sfn libbz2.so.1.0 "$COMPAT/libbz2.so"
install -m 0644 "$artifact_dir/libbz2.a" "$COMPAT/libbz2.a"
install -m 0644 "$SAFE_HEADER" "$COMPAT/include/bzlib.h"
shared_object="$COMPAT/libbz2.so.1.0.4"
require_file "$shared_object"

# Relink the staged shared object against captured upstream objects so wrapper
# coverage keeps consuming target/original-baseline instead of source rebuilds.
link_object_fixture "$COMPAT/public_api_test" "$BASELINE/public_api_test.o"
link_object_fixture "$COMPAT/bzip2" "$BASELINE/bzip2.o"
link_object_fixture "$COMPAT/dlltest" "$BASELINE/dlltest.o"
