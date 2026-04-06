#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPAT="$ROOT/target/compat"

run_samples=0
while (($# > 0)); do
  case "$1" in
    --run-samples)
      run_samples=1
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

test -f "$COMPAT/libbz2.so.1.0.4"
test -f "$COMPAT/include/bzlib.h"

gcc \
  -D_FILE_OFFSET_BITS=64 \
  -Wall -Winline -O2 -g \
  -o "$COMPAT/bzip2" \
  "$ROOT/original/bzip2.c" \
  -I"$COMPAT/include" \
  -L"$COMPAT" \
  -Wl,-rpath,'$ORIGIN' \
  -lbz2

if (( run_samples )); then
  mkdir -p "$ROOT/target"
  tmpdir="$(mktemp -d "$ROOT/target/compat-bzip2.XXXXXX")"
  trap 'rm -rf "$tmpdir"' EXIT

  "$COMPAT/bzip2" -1c "$ROOT/original/sample1.ref" > "$tmpdir/sample1.bz2"
  cmp "$tmpdir/sample1.bz2" "$ROOT/original/sample1.bz2"

  "$COMPAT/bzip2" -2c "$ROOT/original/sample2.ref" > "$tmpdir/sample2.bz2"
  cmp "$tmpdir/sample2.bz2" "$ROOT/original/sample2.bz2"

  "$COMPAT/bzip2" -3c "$ROOT/original/sample3.ref" > "$tmpdir/sample3.bz2"
  cmp "$tmpdir/sample3.bz2" "$ROOT/original/sample3.bz2"
fi
