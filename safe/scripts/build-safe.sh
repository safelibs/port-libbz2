#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MANIFEST="$ROOT/safe/Cargo.toml"
COMPAT="$ROOT/target/compat"

profile="debug"
cargo_args=()

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

mkdir -p \
  "$ROOT/target/original-baseline" \
  "$ROOT/target/compat" \
  "$ROOT/target/install" \
  "$ROOT/target/package" \
  "$ROOT/target/bench" \
  "$ROOT/target/security" \
  "$COMPAT/include"

export CARGO_TARGET_DIR="$COMPAT/cargo"

cargo build --manifest-path "$MANIFEST" "${cargo_args[@]}"

artifact_dir="$CARGO_TARGET_DIR/$profile"
install -m 0755 "$artifact_dir/libbz2.so" "$COMPAT/libbz2.so.1.0.4"
ln -sfn libbz2.so.1.0.4 "$COMPAT/libbz2.so.1.0"
ln -sfn libbz2.so.1.0 "$COMPAT/libbz2.so"
install -m 0644 "$artifact_dir/libbz2.a" "$COMPAT/libbz2.a"
install -m 0644 "$ROOT/safe/include/bzlib.h" "$COMPAT/include/bzlib.h"

if [[ -e "$ROOT/target/original-baseline/public_api_test.o" ]]; then
  gcc \
    -o "$COMPAT/public_api_test" \
    "$ROOT/target/original-baseline/public_api_test.o" \
    -L"$COMPAT" \
    -Wl,-rpath,'$ORIGIN' \
    -lbz2
fi

if [[ -e "$ROOT/target/original-baseline/bzip2.o" ]]; then
  gcc \
    -o "$COMPAT/bzip2" \
    "$ROOT/target/original-baseline/bzip2.o" \
    -L"$COMPAT" \
    -Wl,-rpath,'$ORIGIN' \
    -lbz2
fi
