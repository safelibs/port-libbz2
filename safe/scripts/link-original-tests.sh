#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPAT="$ROOT/target/compat"
BASELINE="$ROOT/target/original-baseline"
cc_bin="${CC:-gcc}"
shared_object="$COMPAT/libbz2.so.1.0.4"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || die "missing required file: $1"
}

run_with_compat_lib() {
  env LD_LIBRARY_PATH="$COMPAT${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$@"
}

mode=""
while (($# > 0)); do
  case "$1" in
    --read-side)
      mode="read-side"
      ;;
    --public-api)
      mode="public-api"
      ;;
    --dlltest-object)
      mode="dlltest-object"
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
  echo "expected --read-side, --public-api, --dlltest-object, or --all" >&2
  exit 1
fi

require_file "$shared_object"
require_file "$COMPAT/include/bzlib.h"
require_file "$BASELINE/dlltest.o"
require_file "$BASELINE/public_api_test.o"
require_file "$BASELINE/bzip2.o"
require_file "$BASELINE/dlltest-path.bz2"
require_file "$BASELINE/dlltest-path.out"
require_file "$BASELINE/dlltest-stdio.bz2"
require_file "$BASELINE/dlltest-stdio.out"

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
  "$cc_bin" \
    -D_FILE_OFFSET_BITS=64 \
    -Wall -Winline -O2 -g \
    -o "$output" \
    "$source" \
    -I"$COMPAT/include" \
    -Wl,-rpath,'$ORIGIN' \
    "$shared_object"
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

DLLTEST_PATH_BZ2="$BASELINE/dlltest-path.bz2"
DLLTEST_PATH_OUT="$BASELINE/dlltest-path.out"
DLLTEST_STDIO_BZ2="$BASELINE/dlltest-stdio.bz2"
DLLTEST_STDIO_OUT="$BASELINE/dlltest-stdio.out"
PUBLIC_API_OBJECT="$BASELINE/public_api_test.o"
CLI_OBJECT="$BASELINE/bzip2.o"

run_public_api_source() {
  compile_c_fixture "$COMPAT/public_api_test-source" "$ROOT/original/public_api_test.c"
  run_with_compat_lib "$COMPAT/public_api_test-source"
}

run_public_api_object() {
  link_object_fixture "$COMPAT/public_api_test-object" "$PUBLIC_API_OBJECT"
  run_with_compat_lib "$COMPAT/public_api_test-object"
}

run_bzip2_object() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-bzip2-object.XXXXXX")"

  link_object_fixture "$COMPAT/bzip2-object" "$CLI_OBJECT"

  run_with_compat_lib "$COMPAT/bzip2-object" -1c "$ROOT/original/sample1.ref" > "$tmpdir/sample1.bz2"
  cmp "$tmpdir/sample1.bz2" "$ROOT/original/sample1.bz2"

  run_with_compat_lib "$COMPAT/bzip2-object" -2c "$ROOT/original/sample2.ref" > "$tmpdir/sample2.bz2"
  cmp "$tmpdir/sample2.bz2" "$ROOT/original/sample2.bz2"

  run_with_compat_lib "$COMPAT/bzip2-object" -3c "$ROOT/original/sample3.ref" > "$tmpdir/sample3.bz2"
  cmp "$tmpdir/sample3.bz2" "$ROOT/original/sample3.bz2"
  rm -rf "$tmpdir"
}

run_dlltest_read_modes() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-dlltest-read.XXXXXX")"
  local path_bz2_rel path_out_rel stdio_bz2_rel stdio_out_rel tmpdir_rel

  compile_c_fixture "$COMPAT/dlltest-source" "$ROOT/original/dlltest.c"
  link_object_fixture "$COMPAT/dlltest-object" "$BASELINE/dlltest.o"
  path_bz2_rel="$(repo_relative "$DLLTEST_PATH_BZ2")"
  path_out_rel="$(repo_relative "$DLLTEST_PATH_OUT")"
  stdio_bz2_rel="$(repo_relative "$DLLTEST_STDIO_BZ2")"
  stdio_out_rel="$(repo_relative "$DLLTEST_STDIO_OUT")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  (
    cd "$ROOT"
    run_with_compat_lib "$COMPAT/dlltest-source" -d "$path_bz2_rel" "$tmpdir_rel/path.out"
    cmp "$tmpdir_rel/path.out" "$path_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-source" -d < "$stdio_bz2_rel" > "$tmpdir_rel/stdio.out"
    cmp "$tmpdir_rel/stdio.out" "$stdio_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-object" -d "$path_bz2_rel" "$tmpdir_rel/object-path.out"
    cmp "$tmpdir_rel/object-path.out" "$path_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-object" -d < "$stdio_bz2_rel" > "$tmpdir_rel/object-stdio.out"
    cmp "$tmpdir_rel/object-stdio.out" "$stdio_out_rel"
  )
  rm -rf "$tmpdir"
}

run_dlltest_source_all_modes() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-dlltest-source.XXXXXX")"
  local path_bz2_rel path_out_rel stdio_bz2_rel stdio_out_rel tmpdir_rel

  compile_c_fixture "$COMPAT/dlltest-source" "$ROOT/original/dlltest.c"
  path_bz2_rel="$(repo_relative "$DLLTEST_PATH_BZ2")"
  path_out_rel="$(repo_relative "$DLLTEST_PATH_OUT")"
  stdio_bz2_rel="$(repo_relative "$DLLTEST_STDIO_BZ2")"
  stdio_out_rel="$(repo_relative "$DLLTEST_STDIO_OUT")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  (
    cd "$ROOT"
    run_with_compat_lib "$COMPAT/dlltest-source" -d "$path_bz2_rel" "$tmpdir_rel/path.out"
    cmp "$tmpdir_rel/path.out" "$path_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-source" -d < "$stdio_bz2_rel" > "$tmpdir_rel/stdio.out"
    cmp "$tmpdir_rel/stdio.out" "$stdio_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-source" "$path_out_rel" "$tmpdir_rel/path.bz2"
    cmp "$tmpdir_rel/path.bz2" "$path_bz2_rel"

    run_with_compat_lib "$COMPAT/dlltest-source" -1 < "$stdio_out_rel" > "$tmpdir_rel/stdio.bz2"
    cmp "$tmpdir_rel/stdio.bz2" "$stdio_bz2_rel"
  )
  rm -rf "$tmpdir"
}

run_dlltest_object_all_modes() {
  mkdir -p "$ROOT/target"
  local tmpdir
  tmpdir="$(mktemp -d "$ROOT/target/link-dlltest-object.XXXXXX")"
  local path_bz2_rel path_out_rel stdio_bz2_rel stdio_out_rel tmpdir_rel

  link_object_fixture "$COMPAT/dlltest-object" "$BASELINE/dlltest.o"
  path_bz2_rel="$(repo_relative "$DLLTEST_PATH_BZ2")"
  path_out_rel="$(repo_relative "$DLLTEST_PATH_OUT")"
  stdio_bz2_rel="$(repo_relative "$DLLTEST_STDIO_BZ2")"
  stdio_out_rel="$(repo_relative "$DLLTEST_STDIO_OUT")"
  tmpdir_rel="$(repo_relative "$tmpdir")"

  (
    cd "$ROOT"
    run_with_compat_lib "$COMPAT/dlltest-object" -d "$path_bz2_rel" "$tmpdir_rel/path.out"
    cmp "$tmpdir_rel/path.out" "$path_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-object" -d < "$stdio_bz2_rel" > "$tmpdir_rel/stdio.out"
    cmp "$tmpdir_rel/stdio.out" "$stdio_out_rel"

    run_with_compat_lib "$COMPAT/dlltest-object" "$path_out_rel" "$tmpdir_rel/path.bz2"
    cmp "$tmpdir_rel/path.bz2" "$path_bz2_rel"

    run_with_compat_lib "$COMPAT/dlltest-object" -1 < "$stdio_out_rel" > "$tmpdir_rel/stdio.bz2"
    cmp "$tmpdir_rel/stdio.bz2" "$stdio_bz2_rel"
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
  dlltest-object)
    run_dlltest_object_all_modes
    ;;
  all)
    run_public_api_source
    run_public_api_object
    run_dlltest_source_all_modes
    run_dlltest_object_all_modes
    run_bzip2_object
    ;;
esac
