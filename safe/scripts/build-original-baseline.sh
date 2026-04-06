#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ORIGINAL="$ROOT/original"
BASELINE="$ROOT/target/original-baseline"
DLLTEST_FLAGS=(
  -D_FILE_OFFSET_BITS=64
  -Wall
  -Winline
  -O2
  -g
)

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

require_dir() {
  [[ -d "$1" ]] || die "missing required directory: $1"
}

require_dir "$ORIGINAL"
for required in \
  "$ORIGINAL/Makefile" \
  "$ORIGINAL/bzlib.h" \
  "$ORIGINAL/libbz2.def" \
  "$ORIGINAL/public_api_test.c" \
  "$ORIGINAL/dlltest.c" \
  "$ORIGINAL/bzip2.c"
do
  require_file "$required"
done

cc_bin="${CC:-gcc}"
require_tool "$cc_bin"
require_tool make

mkdir -p \
  "$ROOT/target/original-baseline" \
  "$ROOT/target/compat" \
  "$ROOT/target/install" \
  "$ROOT/target/package" \
  "$ROOT/target/bench" \
  "$ROOT/target/security"

rm -rf "$BASELINE"
mkdir -p "$BASELINE"

make -C "$ORIGINAL" CC="$cc_bin" libbz2.so public_api_test bzip2

"$cc_bin" \
  "${DLLTEST_FLAGS[@]}" \
  -I"$ORIGINAL" \
  -o "$BASELINE/dlltest.o" \
  -c "$ORIGINAL/dlltest.c"

"$cc_bin" \
  "${DLLTEST_FLAGS[@]}" \
  -I"$ORIGINAL" \
  -o "$ORIGINAL/dlltest" \
  "$ORIGINAL/dlltest.c" \
  -L"$ORIGINAL" \
  -lbz2

run_original_dlltest() {
  (
    cd "$ORIGINAL"
    env LD_LIBRARY_PATH="$ORIGINAL${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}" ./dlltest "$@"
  )
}

if [[ ! -e "$ORIGINAL/dlltest-path.bz2" ]]; then
  run_original_dlltest sample1.ref dlltest-path.bz2
fi

if [[ ! -e "$ORIGINAL/dlltest-path.out" ]]; then
  run_original_dlltest -d dlltest-path.bz2 dlltest-path.out
fi

if [[ ! -e "$ORIGINAL/dlltest-stdio.bz2" ]]; then
  (
    cd "$ORIGINAL"
    env LD_LIBRARY_PATH="$ORIGINAL${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}" ./dlltest -1 < sample1.ref > dlltest-stdio.bz2
  )
fi

if [[ ! -e "$ORIGINAL/dlltest-stdio.out" ]]; then
  (
    cd "$ORIGINAL"
    env LD_LIBRARY_PATH="$ORIGINAL${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}" ./dlltest -d < dlltest-stdio.bz2 > dlltest-stdio.out
  )
fi

# Preserve the exact upstream-built objects in target/original-baseline so
# staged relink checks do not quietly fall back to recompiling wrappers.
for artifact in \
  libbz2.so \
  libbz2.so.1.0 \
  libbz2.so.1.0.4 \
  public_api_test \
  public_api_test.o \
  bzip2 \
  bzip2.o \
  dlltest \
  dlltest-path.bz2 \
  dlltest-path.out \
  dlltest-stdio.bz2 \
  dlltest-stdio.out
do
  require_file "$ORIGINAL/$artifact"
  cp -a "$ORIGINAL/$artifact" "$BASELINE/$artifact"
done
