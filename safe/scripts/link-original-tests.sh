#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPAT="$ROOT/target/compat"
BASELINE="$ROOT/target/original-baseline"

mode=""
while (($# > 0)); do
  case "$1" in
    --read-side)
      mode="read-side"
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

if [[ "$mode" != "read-side" ]]; then
  echo "expected --read-side" >&2
  exit 1
fi

test -f "$COMPAT/libbz2.so.1.0.4"
test -f "$BASELINE/dlltest-path.bz2"
test -f "$BASELINE/dlltest-path.out"
test -f "$BASELINE/dlltest-stdio.bz2"
test -f "$BASELINE/dlltest-stdio.out"

gcc \
  -o "$COMPAT/dlltest-read-side" \
  "$ROOT/original/dlltest.c" \
  -I"$COMPAT/include" \
  -L"$COMPAT" \
  -Wl,-rpath,'$ORIGIN' \
  -lbz2

mkdir -p "$ROOT/target"
tmpdir="$(mktemp -d "$ROOT/target/link-original-tests.XXXXXX")"
trap 'rm -rf "$tmpdir"' EXIT
tmpdir_rel="${tmpdir#$ROOT/}"

cd "$ROOT"

"$COMPAT/dlltest-read-side" \
  -d \
  "target/original-baseline/dlltest-path.bz2" \
  "$tmpdir_rel/path.out"
cmp "$tmpdir_rel/path.out" "target/original-baseline/dlltest-path.out"

"$COMPAT/dlltest-read-side" \
  -d \
  < "target/original-baseline/dlltest-stdio.bz2" \
  > "$tmpdir_rel/stdio.out"
cmp "$tmpdir_rel/stdio.out" "target/original-baseline/dlltest-stdio.out"
