#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ORIGINAL="$ROOT/original"
BASELINE="$ROOT/target/original-baseline"

mkdir -p \
  "$ROOT/target/original-baseline" \
  "$ROOT/target/compat" \
  "$ROOT/target/install" \
  "$ROOT/target/package" \
  "$ROOT/target/bench" \
  "$ROOT/target/security"

need_original_build=0
for artifact in \
  libbz2.so \
  libbz2.so.1.0 \
  libbz2.so.1.0.4 \
  public_api_test \
  public_api_test.o \
  bzip2 \
  bzip2.o
do
  if [[ ! -e "$ORIGINAL/$artifact" ]]; then
    need_original_build=1
    break
  fi
done

if (( need_original_build )); then
  make -C "$ORIGINAL" libbz2.so public_api_test bzip2
fi

if [[ ! -e "$ORIGINAL/dlltest" ]]; then
  gcc \
    -D_FILE_OFFSET_BITS=64 \
    -Wall -Winline -O2 -g \
    -o "$ORIGINAL/dlltest" \
    "$ORIGINAL/dlltest.c" \
    -L"$ORIGINAL" \
    -lbz2
fi

run_original_dlltest() {
  (
    cd "$ORIGINAL"
    env LD_LIBRARY_PATH="$ORIGINAL:${LD_LIBRARY_PATH:-}" ./dlltest "$@"
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
    env LD_LIBRARY_PATH="$ORIGINAL:${LD_LIBRARY_PATH:-}" ./dlltest -1 < sample1.ref > dlltest-stdio.bz2
  )
fi

if [[ ! -e "$ORIGINAL/dlltest-stdio.out" ]]; then
  (
    cd "$ORIGINAL"
    env LD_LIBRARY_PATH="$ORIGINAL:${LD_LIBRARY_PATH:-}" ./dlltest -d < dlltest-stdio.bz2 > dlltest-stdio.out
  )
fi

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
  rm -rf "$BASELINE/$artifact"
  cp -a "$ORIGINAL/$artifact" "$BASELINE/$artifact"
done
