#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ABI_DIR="$ROOT/safe/abi"
BASELINE="$ROOT/target/original-baseline"
COMPAT="$ROOT/target/compat"
tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

strict=0

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_file() {
  [[ -f "$1" ]] || die "missing required file: $1"
}

require_symlink_target() {
  local path="$1"
  local expected="$2"
  local actual

  [[ -L "$path" ]] || die "expected symlink: $path"
  actual="$(readlink "$path")"
  [[ "$actual" == "$expected" ]] || die "unexpected symlink target for $path: expected $expected, found $actual"
}

while (($# > 0)); do
  case "$1" in
    --strict)
      strict=1
      ;;
    --strict-exports)
      strict=1
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

normalize_bz2_undefined() {
  local object_path="$1"
  readelf -Ws "$object_path" | awk '$7 == "UND" && $8 ~ /^BZ2_/ { print $8 }' | sed '/^$/d' | sort -u
}

normalize_def_exports() {
  local def_path="$1"
  awk '
    BEGIN { exports = 0 }
    {
      line = $0
      gsub(/\r/, "", line)
    }
    line == "EXPORTS" { exports = 1; next }
    exports {
      if (line != "") {
        split(line, fields, /[[:space:]]+/)
        print fields[1]
      }
    }
  ' "$def_path" | sort
}

normalize_map_exports() {
  local map_path="$1"
  awk '
    BEGIN { in_global = 0 }
    /^[[:space:]]*global:[[:space:]]*$/ { in_global = 1; next }
    /^[[:space:]]*local:[[:space:]]*$/ { in_global = 0; next }
    in_global {
      line = $0
      gsub(/^[[:space:]]+/, "", line)
      gsub(/;[[:space:]]*$/, "", line)
      if (line != "") {
        print line
      }
    }
  ' "$map_path" | sort
}

normalize_expected_export_names() {
  local exports_path="$1"
  awk '{ print $2 }' "$exports_path" | sort
}

count_bz2_undefineds() {
  local object_path="$1"
  readelf -Ws "$object_path" | awk '$7 == "UND" && $8 ~ /^BZ2_/ { print $8 }' | sort -u | wc -l
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

require_line() {
  local pattern="$1"
  local file="$2"
  local label="$3"
  if ! grep -Fxq "$pattern" "$file"; then
    die "missing $label: $pattern"
  fi
}

for required in \
  "$ABI_DIR/original.exports.txt" \
  "$ABI_DIR/original.soname.txt" \
  "$ABI_DIR/original.public_api_undefined.txt" \
  "$ABI_DIR/original.cli_undefined.txt" \
  "$ABI_DIR/libbz2.map" \
  "$ABI_DIR/libbz2.def" \
  "$ROOT/original/libbz2.def" \
  "$BASELINE/libbz2.so.1.0.4" \
  "$BASELINE/public_api_test.o" \
  "$BASELINE/bzip2.o" \
  "$BASELINE/dlltest.o"
do
  require_file "$required"
done

require_symlink_target "$BASELINE/libbz2.so.1.0" "libbz2.so.1.0.4"
require_symlink_target "$BASELINE/libbz2.so" "libbz2.so.1.0"

PUBLIC_API_OBJECT="$BASELINE/public_api_test.o"
CLI_OBJECT="$BASELINE/bzip2.o"
DLLTEST_OBJECT="$BASELINE/dlltest.o"

normalize_exports "$BASELINE/libbz2.so.1.0.4" > "$tmpdir/original.exports.txt"
normalize_soname "$BASELINE/libbz2.so.1.0.4" > "$tmpdir/original.soname.txt"
normalize_undefined "$PUBLIC_API_OBJECT" > "$tmpdir/original.public_api_undefined.txt"
normalize_undefined "$CLI_OBJECT" > "$tmpdir/original.cli_undefined.txt"
normalize_def_exports "$ROOT/original/libbz2.def" > "$tmpdir/original.def.exports.txt"
normalize_expected_export_names "$ABI_DIR/original.exports.txt" > "$tmpdir/original.export_names.txt"
normalize_map_exports "$ABI_DIR/libbz2.map" > "$tmpdir/safe.map.exports.txt"
normalize_def_exports "$ABI_DIR/libbz2.def" > "$tmpdir/safe.def.exports.txt"

compare_file "$ABI_DIR/original.exports.txt" "$tmpdir/original.exports.txt" "baseline exports"
compare_file "$ABI_DIR/original.soname.txt" "$tmpdir/original.soname.txt" "baseline soname"
compare_file "$ABI_DIR/original.public_api_undefined.txt" "$tmpdir/original.public_api_undefined.txt" "public_api_test undefineds"
compare_file "$ABI_DIR/original.cli_undefined.txt" "$tmpdir/original.cli_undefined.txt" "bzip2 undefineds"
compare_file "$tmpdir/original.export_names.txt" "$tmpdir/safe.map.exports.txt" "version script exports"
compare_file "$tmpdir/original.def.exports.txt" "$tmpdir/safe.def.exports.txt" "Windows .def exports"

require_file "$COMPAT/libbz2.so.1.0.4"
require_symlink_target "$COMPAT/libbz2.so.1.0" "libbz2.so.1.0.4"
require_symlink_target "$COMPAT/libbz2.so" "libbz2.so.1.0"

normalize_exports "$COMPAT/libbz2.so.1.0.4" > "$tmpdir/safe.exports.txt"
normalize_soname "$COMPAT/libbz2.so.1.0.4" > "$tmpdir/safe.soname.txt"

compare_file "$ABI_DIR/original.exports.txt" "$tmpdir/safe.exports.txt" "safe exports"
compare_file "$ABI_DIR/original.soname.txt" "$tmpdir/safe.soname.txt" "safe soname"

if (( strict )); then
  cat > "$tmpdir/original.dlltest_bz2_undefined.txt.expected" <<'EOF'
BZ2_bzRead
BZ2_bzReadClose
BZ2_bzReadOpen
BZ2_bzWrite
BZ2_bzWriteClose
BZ2_bzWriteOpen
EOF
  normalize_bz2_undefined "$DLLTEST_OBJECT" > "$tmpdir/original.dlltest_bz2_undefined.txt"

  [[ "$(wc -l < "$tmpdir/safe.exports.txt")" -eq 35 ]] || {
    echo "safe export count mismatch" >&2
    exit 1
  }
  require_line "OBJECT BZ2_crc32Table 1024" "$tmpdir/safe.exports.txt" "ABI data export"
  require_line "OBJECT BZ2_rNums 2048" "$tmpdir/safe.exports.txt" "ABI data export"
  compare_file \
    "$tmpdir/original.dlltest_bz2_undefined.txt.expected" \
    "$tmpdir/original.dlltest_bz2_undefined.txt" \
    "dlltest.o BZ2 undefineds"
  [[ "$(count_bz2_undefineds "$PUBLIC_API_OBJECT")" -eq 23 ]] || {
    echo "public_api_test.o BZ2 symbol count mismatch" >&2
    exit 1
  }
  [[ "$(count_bz2_undefineds "$CLI_OBJECT")" -eq 8 ]] || {
    echo "bzip2.o BZ2 symbol count mismatch" >&2
    exit 1
  }
  [[ "$(count_bz2_undefineds "$DLLTEST_OBJECT")" -eq 6 ]] || {
    echo "dlltest.o BZ2 symbol count mismatch" >&2
    exit 1
  }
fi
