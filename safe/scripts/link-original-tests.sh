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
    --public-api)
      mode="public-api"
      ;;
    --all)
      mode="all"
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

if [[ -z "$mode" ]]; then
  echo "expected --read-side, --public-api, or --all" >&2
  exit 1
fi

test -f "$COMPAT/libbz2.so.1.0.4"
test -f "$COMPAT/include/bzlib.h"
test -f "$BASELINE/dlltest.o"
test -f "$BASELINE/public_api_test.o"
test -f "$BASELINE/bzip2.o"

resolve_file() {
  local preferred="$1"
  local fallback="$2"
  if [[ -f "$preferred" ]]; then
    printf '%s\n' "$preferred"
  else
    printf '%s\n' "$fallback"
  fi
}

repo_relative() {
  local path="$1"
  if [[ "$path" == "$ROOT/"* ]]; then
    printf '%s\n' "${path#$ROOT/}"
  else
    printf '%s\n' "$path"
  fi
}

compile_c_fixture() {
  local output="$1"
  local source="$2"
  gcc \
    -D_FILE_OFFSET_BITS=64 \
    -Wall -Winline -O2 -g \
    -o "$output" \
    "$source" \
    -I"$COMPAT/include" \
    -L"$COMPAT" \
    -Wl,-rpath,'$ORIGIN' \
    -lbz2
}

link_object_fixture() {
  local output="$1"
  local object="$2"
  gcc \
    -o "$output" \
    "$object" \
    -L"$COMPAT" \
    -Wl,-rpath,'$ORIGIN' \
    -lbz2
}

DLLTEST_PATH_BZ2="$(resolve_file "$ROOT/original/dlltest-path.bz2" "$BASELINE/dlltest-path.bz2")"
DLLTEST_PATH_OUT="$(resolve_file "$ROOT/original/dlltest-path.out" "$BASELINE/dlltest-path.out")"
DLLTEST_STDIO_BZ2="$(resolve_file "$ROOT/original/dlltest-stdio.bz2" "$BASELINE/dlltest-stdio.bz2")"
DLLTEST_STDIO_OUT="$(resolve_file "$ROOT/original/dlltest-stdio.out" "$BASELINE/dlltest-stdio.out")"
PUBLIC_API_OBJECT="$(resolve_file "$ROOT/original/public_api_test.o" "$BASELINE/public_api_test.o")"
CLI_OBJECT="$(resolve_file "$ROOT/original/bzip2.o" "$BASELINE/bzip2.o")"

run_public_api_source() {
  compile_c_fixture "$COMPAT/public_api_test-source" "$ROOT/original/public_api_test.c"
  "$COMPAT/public_api_test-source"
}

run_public_api_object() {
  link_object_fixture "$COMPAT/public_api_test-object" "$PUBLIC_API_OBJECT"
  "$COMPAT/public_api_test-object"
}

run_bzip2_object() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-bzip2-object.XXXXXX")"

  link_object_fixture "$COMPAT/bzip2-object" "$CLI_OBJECT"

  local sample1_ref sample2_ref sample3_ref sample1_bz2 sample2_bz2 sample3_bz2 tmpdir_rel
  sample1_ref="$(repo_relative "$ROOT/original/sample1.ref")"
  sample2_ref="$(repo_relative "$ROOT/original/sample2.ref")"
  sample3_ref="$(repo_relative "$ROOT/original/sample3.ref")"
  sample1_bz2="$(repo_relative "$ROOT/original/sample1.bz2")"
  sample2_bz2="$(repo_relative "$ROOT/original/sample2.bz2")"
  sample3_bz2="$(repo_relative "$ROOT/original/sample3.bz2")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  (
    cd "$ROOT"
    "$COMPAT/bzip2-object" -1c "$sample1_ref" > "$tmpdir_rel/sample1.bz2"
    cmp "$tmpdir_rel/sample1.bz2" "$sample1_bz2"

    "$COMPAT/bzip2-object" -2c "$sample2_ref" > "$tmpdir_rel/sample2.bz2"
    cmp "$tmpdir_rel/sample2.bz2" "$sample2_bz2"

    "$COMPAT/bzip2-object" -3c "$sample3_ref" > "$tmpdir_rel/sample3.bz2"
    cmp "$tmpdir_rel/sample3.bz2" "$sample3_bz2"
  )
  rm -rf "$tmpdir"
}

run_dlltest_read_modes() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-dlltest-read.XXXXXX")"
  local path_bz2 path_out stdio_bz2 stdio_out tmpdir_rel

  compile_c_fixture "$COMPAT/dlltest-source" "$ROOT/original/dlltest.c"
  link_object_fixture "$COMPAT/dlltest-object" "$BASELINE/dlltest.o"
  path_bz2="$(repo_relative "$DLLTEST_PATH_BZ2")"
  path_out="$(repo_relative "$DLLTEST_PATH_OUT")"
  stdio_bz2="$(repo_relative "$DLLTEST_STDIO_BZ2")"
  stdio_out="$(repo_relative "$DLLTEST_STDIO_OUT")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  (
    cd "$ROOT"
    "$COMPAT/dlltest-source" -d "$path_bz2" "$tmpdir_rel/path.out"
    cmp "$tmpdir_rel/path.out" "$path_out"

    "$COMPAT/dlltest-source" -d < "$stdio_bz2" > "$tmpdir_rel/stdio.out"
    cmp "$tmpdir_rel/stdio.out" "$stdio_out"

    "$COMPAT/dlltest-object" -d "$path_bz2" "$tmpdir_rel/object-path.out"
    cmp "$tmpdir_rel/object-path.out" "$path_out"

    "$COMPAT/dlltest-object" -d < "$stdio_bz2" > "$tmpdir_rel/object-stdio.out"
    cmp "$tmpdir_rel/object-stdio.out" "$stdio_out"
  )
  rm -rf "$tmpdir"
}

run_dlltest_all_modes() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-dlltest-all.XXXXXX")"
  local path_bz2 path_out stdio_bz2 stdio_out tmpdir_rel

  compile_c_fixture "$COMPAT/dlltest-source" "$ROOT/original/dlltest.c"
  link_object_fixture "$COMPAT/dlltest-object" "$BASELINE/dlltest.o"
  path_bz2="$(repo_relative "$DLLTEST_PATH_BZ2")"
  path_out="$(repo_relative "$DLLTEST_PATH_OUT")"
  stdio_bz2="$(repo_relative "$DLLTEST_STDIO_BZ2")"
  stdio_out="$(repo_relative "$DLLTEST_STDIO_OUT")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  (
    cd "$ROOT"
    "$COMPAT/dlltest-source" -d "$path_bz2" "$tmpdir_rel/path.out"
    cmp "$tmpdir_rel/path.out" "$path_out"

    "$COMPAT/dlltest-source" -d < "$stdio_bz2" > "$tmpdir_rel/stdio.out"
    cmp "$tmpdir_rel/stdio.out" "$stdio_out"

    "$COMPAT/dlltest-source" "$path_out" "$tmpdir_rel/path.bz2"
    cmp "$tmpdir_rel/path.bz2" "$path_bz2"

    "$COMPAT/dlltest-source" -1 < "$stdio_out" > "$tmpdir_rel/stdio.bz2"
    cmp "$tmpdir_rel/stdio.bz2" "$stdio_bz2"

    "$COMPAT/dlltest-object" -d "$path_bz2" "$tmpdir_rel/object-path.out"
    cmp "$tmpdir_rel/object-path.out" "$path_out"

    "$COMPAT/dlltest-object" -d < "$stdio_bz2" > "$tmpdir_rel/object-stdio.out"
    cmp "$tmpdir_rel/object-stdio.out" "$stdio_out"

    "$COMPAT/dlltest-object" "$path_out" "$tmpdir_rel/object-path.bz2"
    cmp "$tmpdir_rel/object-path.bz2" "$path_bz2"

    "$COMPAT/dlltest-object" -1 < "$stdio_out" > "$tmpdir_rel/object-stdio.bz2"
    cmp "$tmpdir_rel/object-stdio.bz2" "$stdio_bz2"
  )
  rm -rf "$tmpdir"
}

case "$mode" in
  public-api)
    run_public_api_source
    run_public_api_object
    ;;
  read-side)
    run_dlltest_read_modes
    ;;
  all)
    run_public_api_source
    run_public_api_object
    run_dlltest_all_modes
    run_bzip2_object
    ;;
esac
