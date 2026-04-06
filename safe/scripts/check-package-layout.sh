#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_ROOT="$ROOT/target/package"
SRC="$PACKAGE_ROOT/src"
OUT="$PACKAGE_ROOT/out"
UNPACKED="$PACKAGE_ROOT/unpacked"
MANIFEST="$OUT/package-manifest.txt"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

lookup_manifest_value() {
  local key="$1"
  local value

  value="$(grep -E "^${key}=" "$MANIFEST" | tail -n1 | cut -d= -f2-)"
  [[ -n "$value" ]] || die "manifest entry missing: $key"
  printf '%s\n' "$value"
}

require_file() {
  [[ -e "$1" ]] || die "expected path is missing: $1"
}

require_symlink_target() {
  local path="$1"
  local expected="$2"
  local actual

  [[ -L "$path" ]] || die "expected symlink: $path"
  actual="$(readlink "$path")"
  [[ "$actual" == "$expected" ]] || die "unexpected symlink target for $path: expected $expected, found $actual"
}

[[ -f "$MANIFEST" ]] || die "missing package manifest: $MANIFEST; run bash safe/scripts/build-debs.sh first"
[[ "$(lookup_manifest_value "source_dir")" == "target/package/src" ]] || {
  die "package manifest points at an unexpected source_dir; expected target/package/src"
}
[[ -n "$(lookup_manifest_value "version")" ]] || die "package manifest is missing a version entry"
shopt -s nullglob

[[ ! -e "$SRC/target" ]] || {
  die "unexpected staged Cargo target tree copied into $SRC; safe/scripts/build-debs.sh should stage source files only"
}

for pkg in libbz2-1.0 libbz2-dev bzip2 bzip2-doc; do
  deb_name="$(lookup_manifest_value "package:$pkg")"
  [[ -f "$OUT/$deb_name" ]] || die "required package artifact missing from $OUT: $deb_name"
  [[ -d "$UNPACKED/$pkg" ]] || die "missing unpacked inspection tree: $UNPACKED/$pkg"
done

require_file "$SRC/debian/control"
require_file "$SRC/debian/bzip2-doc.doc-base"
require_file "$SRC/manual.texi"
require_file "$SRC/bzip2.info"

multiarch_dir="$(find "$UNPACKED/libbz2-1.0/usr/lib" -mindepth 1 -maxdepth 1 -type d | head -n1)"
[[ -n "$multiarch_dir" ]] || die "unable to locate the runtime multiarch library directory"

require_file "$multiarch_dir/libbz2.so.1.0.4"
require_symlink_target "$multiarch_dir/libbz2.so.1.0" "libbz2.so.1.0.4"
require_symlink_target "$multiarch_dir/libbz2.so.1" "libbz2.so.1.0.4"

require_file "$UNPACKED/libbz2-dev/usr/include/bzlib.h"
require_file "$UNPACKED/libbz2-dev/usr/lib/$(basename "$multiarch_dir")/libbz2.a"
require_symlink_target "$UNPACKED/libbz2-dev/usr/lib/$(basename "$multiarch_dir")/libbz2.so" "libbz2.so.1.0"

for path in \
  /usr/bin/bzip2 \
  /usr/bin/bzip2recover \
  /usr/bin/bzdiff \
  /usr/bin/bzgrep \
  /usr/bin/bzmore \
  /usr/bin/bzexe
do
  require_file "$UNPACKED/bzip2$path"
done

for path in \
  /usr/bin/bunzip2 \
  /usr/bin/bzcat \
  /usr/bin/bzcmp \
  /usr/bin/bzegrep \
  /usr/bin/bzfgrep \
  /usr/bin/bzless
do
  require_file "$UNPACKED/bzip2$path"
done

for path in \
  /usr/share/doc/bzip2/manual.html \
  /usr/share/doc/bzip2/manual.pdf.gz \
  /usr/share/doc/bzip2/manual.ps.gz \
  /usr/share/doc/bzip2/manual.texi.gz
do
  require_file "$UNPACKED/bzip2-doc$path"
done

info_matches=( "$UNPACKED/bzip2-doc/usr/share/info"/bzip2.info* )
(( ${#info_matches[@]} > 0 )) || die "missing /usr/share/info/bzip2.info* in bzip2-doc"

docbase_dir="$UNPACKED/bzip2-doc/usr/share/doc-base"
[[ -d "$docbase_dir" ]] || die "missing installed doc-base directory: $docbase_dir"
docbase_file="$(find "$docbase_dir" -mindepth 1 -maxdepth 1 -type f | head -n1)"
[[ -n "$docbase_file" ]] || die "missing installed doc-base metadata under $docbase_dir"
grep -F "Document: bzip2" "$docbase_file" >/dev/null || die "installed doc-base metadata does not describe bzip2"

while read -r field path; do
  case "$field" in
    Index:|Files:)
      require_file "$UNPACKED/bzip2-doc$path"
      ;;
  esac
done < <(awk '/^(Index|Files): / { print $1, $2 }' "$SRC/debian/bzip2-doc.doc-base")
