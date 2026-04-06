#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SOURCE_ROOT="$ROOT"
OUTPUT_DIR="$ROOT/target/compat"
LIB_DIR="$ROOT/target/compat"
INCLUDE_DIR="$ROOT/target/compat/include"
ORIGINAL=""

default_cppflags='-D_REENTRANT -D_FILE_OFFSET_BITS=64'
default_cflags='-Wall -Winline -O2 -g'

run_with_safe_lib() {
  env LD_LIBRARY_PATH="$LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" "$@"
}

run_samples=0
while (($# > 0)); do
  case "$1" in
    --source-root)
      SOURCE_ROOT="${2:?missing value for --source-root}"
      shift
      ;;
    --output-dir)
      OUTPUT_DIR="${2:?missing value for --output-dir}"
      shift
      ;;
    --lib-dir)
      LIB_DIR="${2:?missing value for --lib-dir}"
      shift
      ;;
    --include-dir)
      INCLUDE_DIR="${2:?missing value for --include-dir}"
      shift
      ;;
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

ORIGINAL="$SOURCE_ROOT/original"
if [[ ! -d "$ORIGINAL" ]]; then
  ORIGINAL="$SOURCE_ROOT"
fi

[[ -f "$ORIGINAL/bzip2.c" && -f "$ORIGINAL/bzip2recover.c" ]] || {
  echo "missing original CLI sources under $SOURCE_ROOT" >&2
  exit 1
}

[[ -f "$INCLUDE_DIR/bzlib.h" ]] || {
  echo "missing bzlib.h under $INCLUDE_DIR" >&2
  exit 1
}

[[ -f "$LIB_DIR/libbz2.so" || -f "$LIB_DIR/libbz2.so.1.0.4" ]] || {
  echo "missing libbz2 shared library under $LIB_DIR" >&2
  exit 1
}

mkdir -p "$OUTPUT_DIR"

read -r -a cppflags_array <<< "${CPPFLAGS:-$default_cppflags}"
read -r -a cflags_array <<< "${CFLAGS:-$default_cflags}"
read -r -a ldflags_array <<< "${LDFLAGS:-}"

cc_bin="${CC:-gcc}"
link_args=(-L"$LIB_DIR")
if [[ "$OUTPUT_DIR" == "$LIB_DIR" ]]; then
  link_args+=(-Wl,-rpath,'$ORIGIN')
fi

("$cc_bin" \
  "${cppflags_array[@]}" \
  "${cflags_array[@]}" \
  -o "$OUTPUT_DIR/bzip2" \
  "$ORIGINAL/bzip2.c" \
  -I"$INCLUDE_DIR" \
  "${link_args[@]}" \
  "${ldflags_array[@]}" \
  -lbz2
)

("$cc_bin" \
  "${cppflags_array[@]}" \
  "${cflags_array[@]}" \
  -o "$OUTPUT_DIR/bzip2recover" \
  "$ORIGINAL/bzip2recover.c" \
  "${ldflags_array[@]}"
)

install -m 0755 "$ORIGINAL/bzdiff" "$OUTPUT_DIR/bzdiff"
install -m 0755 "$ORIGINAL/bzgrep" "$OUTPUT_DIR/bzgrep"
install -m 0755 "$ORIGINAL/bzmore" "$OUTPUT_DIR/bzmore"

if (( run_samples )); then
  mkdir -p "$SOURCE_ROOT/target"
  tmpdir="$(mktemp -d "$SOURCE_ROOT/target/compat-bzip2.XXXXXX")"
  trap 'rm -rf "$tmpdir"' EXIT

  run_with_safe_lib "$OUTPUT_DIR/bzip2" -1c "$ORIGINAL/sample1.ref" > "$tmpdir/sample1.bz2"
  cmp "$tmpdir/sample1.bz2" "$ORIGINAL/sample1.bz2"

  run_with_safe_lib "$OUTPUT_DIR/bzip2" -2c "$ORIGINAL/sample2.ref" > "$tmpdir/sample2.bz2"
  cmp "$tmpdir/sample2.bz2" "$ORIGINAL/sample2.bz2"

  run_with_safe_lib "$OUTPUT_DIR/bzip2" -3c "$ORIGINAL/sample3.ref" > "$tmpdir/sample3.bz2"
  cmp "$tmpdir/sample3.bz2" "$ORIGINAL/sample3.bz2"
fi
