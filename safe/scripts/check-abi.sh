#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ABI_DIR="$ROOT/safe/abi"
BASELINE="$ROOT/target/original-baseline"
COMPAT="$ROOT/target/compat"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

while (($# > 0)); do
  case "$1" in
    --baseline-only)
      ;;
    *)
      echo "unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

normalize_exports() {
  local so_path="$1"
  readelf --dyn-syms --wide "$so_path" \
    | awk 'NR > 3 && $8 ~ /^BZ2_/ { if ($4 == "OBJECT") printf "%s %s %s\n", $4, $8, $3; else printf "%s %s\n", $4, $8 }' \
    | sort
}

normalize_soname() {
  local so_path="$1"
  printf 'linkname libbz2.so\n'
  printf 'soname %s\n' "$(readelf -d "$so_path" | awk -F'[][]' '/SONAME/ { print $2 }')"
  printf 'realname %s\n' "$(basename "$so_path")"
}

normalize_undefined() {
  local object_path="$1"
  readelf -Ws "$object_path" | awk '$7 == "UND" { print $8 }' | sed '/^$/d' | sort -u
}

compare_file() {
  local expected="$1"
  local actual="$2"
  local label="$3"
  if ! diff -u "$expected" "$actual"; then
    echo "$label mismatch" >&2
    exit 1
  fi
}

test -f "$BASELINE/libbz2.so.1.0.4"
test -f "$BASELINE/public_api_test.o"
test -f "$BASELINE/bzip2.o"
test -f "$COMPAT/libbz2.so.1.0.4"

normalize_exports "$BASELINE/libbz2.so.1.0.4" > "$tmpdir/original.exports.txt"
normalize_soname "$BASELINE/libbz2.so.1.0.4" > "$tmpdir/original.soname.txt"
normalize_undefined "$BASELINE/public_api_test.o" > "$tmpdir/original.public_api_undefined.txt"
normalize_undefined "$BASELINE/bzip2.o" > "$tmpdir/original.cli_undefined.txt"
normalize_exports "$COMPAT/libbz2.so.1.0.4" > "$tmpdir/safe.exports.txt"
normalize_soname "$COMPAT/libbz2.so.1.0.4" > "$tmpdir/safe.soname.txt"

compare_file "$ABI_DIR/original.exports.txt" "$tmpdir/original.exports.txt" "baseline exports"
compare_file "$ABI_DIR/original.soname.txt" "$tmpdir/original.soname.txt" "baseline soname"
compare_file "$ABI_DIR/original.public_api_undefined.txt" "$tmpdir/original.public_api_undefined.txt" "public_api_test undefineds"
compare_file "$ABI_DIR/original.cli_undefined.txt" "$tmpdir/original.cli_undefined.txt" "bzip2 undefineds"
compare_file "$ABI_DIR/original.exports.txt" "$tmpdir/safe.exports.txt" "safe exports"
compare_file "$ABI_DIR/original.soname.txt" "$tmpdir/safe.soname.txt" "safe soname"
