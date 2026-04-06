#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_ROOT="$ROOT/target/package"
SRC="$PACKAGE_ROOT/src"
OUT="$PACKAGE_ROOT/out"
UNPACKED="$PACKAGE_ROOT/unpacked"
MANIFEST="$OUT/package-manifest.txt"
IMAGE_TAG="${LIBBZ2_DEB_BUILD_IMAGE:-libbz2-safe-deb-build:ubuntu24.04}"

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

stage_safe_tree() {
  tar -C "$ROOT/safe" \
    --exclude='./target' \
    --exclude='./debian/.debhelper' \
    --exclude='./debian/tmp' \
    --exclude='./debian/files' \
    --exclude='./debian/debhelper-build-stamp' \
    --exclude='./debian/cargo-home' \
    --exclude='./debian/*.debhelper.log' \
    --exclude='./debian/*.substvars' \
    --exclude='./debian/libbz2-1.0' \
    --exclude='./debian/libbz2-dev' \
    --exclude='./debian/bzip2' \
    --exclude='./debian/bzip2-doc' \
    -cf - . | tar -C "$SRC" -xf -
}

copy_stage_asset() {
  local source="$1"
  local dest="$2"
  install -D -m 0644 "$source" "$dest"
}

copy_stage_executable() {
  local source="$1"
  local dest="$2"
  install -D -m 0755 "$source" "$dest"
}

lookup_manifest_value() {
  local key="$1"
  local value

  value="$(grep -E "^${key}=" "$MANIFEST" | tail -n1 | cut -d= -f2-)"
  [[ -n "$value" ]] || die "manifest entry missing: $key"
  printf '%s\n' "$value"
}

require_tool docker
require_tool dpkg-deb
require_tool python3
require_tool tar

[[ -d "$ROOT/safe" ]] || die "missing safe/ source tree"
[[ -d "$ROOT/original" ]] || die "missing original/ source tree"

for required in \
  "$ROOT/safe/Cargo.toml" \
  "$ROOT/safe/Cargo.lock" \
  "$ROOT/safe/build.rs" \
  "$ROOT/safe/include/bzlib.h" \
  "$ROOT/safe/debian/changelog" \
  "$ROOT/safe/debian/control" \
  "$ROOT/safe/debian/rules" \
  "$ROOT/safe/debian/clean" \
  "$ROOT/safe/debian/copyright" \
  "$ROOT/safe/debian/not-installed" \
  "$ROOT/safe/debian/bzip2.install" \
  "$ROOT/safe/debian/bzip2.links" \
  "$ROOT/safe/debian/bzip2.manpages" \
  "$ROOT/safe/debian/bzip2-doc.doc-base" \
  "$ROOT/safe/debian/bzip2-doc.docs" \
  "$ROOT/safe/debian/bzip2-doc.info" \
  "$ROOT/safe/debian/libbz2-1.0.install" \
  "$ROOT/safe/debian/libbz2-1.0.shlibs" \
  "$ROOT/safe/debian/libbz2-dev.install" \
  "$ROOT/safe/debian/source/format" \
  "$ROOT/safe/debian/source/options"
do
  require_file "$required"
done

rm -rf "$PACKAGE_ROOT"
mkdir -p "$SRC" "$OUT" "$UNPACKED"
stage_safe_tree

copy_stage_asset "$ROOT/original/bzip2.c" "$SRC/bzip2.c"
copy_stage_asset "$ROOT/original/bzip2recover.c" "$SRC/bzip2recover.c"
copy_stage_executable "$ROOT/original/bzdiff" "$SRC/bzdiff"
copy_stage_executable "$ROOT/original/bzgrep" "$SRC/bzgrep"
copy_stage_executable "$ROOT/original/bzmore" "$SRC/bzmore"
copy_stage_asset "$ROOT/original/bzip2.1" "$SRC/bzip2.1"
copy_stage_asset "$ROOT/original/bzgrep.1" "$SRC/bzgrep.1"
copy_stage_asset "$ROOT/original/bzmore.1" "$SRC/bzmore.1"
copy_stage_asset "$ROOT/original/bzdiff.1" "$SRC/bzdiff.1"
copy_stage_asset "$ROOT/original/manual.xml" "$SRC/manual.xml"
copy_stage_asset "$ROOT/original/entities.xml" "$SRC/entities.xml"
copy_stage_asset "$ROOT/original/manual.html" "$SRC/manual.html"
copy_stage_asset "$ROOT/original/manual.pdf" "$SRC/manual.pdf"
copy_stage_asset "$ROOT/original/manual.ps" "$SRC/manual.ps"
copy_stage_executable "$ROOT/original/debian/bzexe" "$SRC/debian/bzexe"
copy_stage_asset "$ROOT/original/debian/bzexe.1" "$SRC/debian/bzexe.1"
copy_stage_asset "$ROOT/original/debian/bzip2-doc.docs" "$SRC/debian/bzip2-doc.docs"
copy_stage_asset "$ROOT/original/debian/bzip2-doc.doc-base" "$SRC/debian/bzip2-doc.doc-base"
copy_stage_asset "$ROOT/original/debian/bzip2-doc.info" "$SRC/debian/bzip2-doc.info"

find "$PACKAGE_ROOT" -maxdepth 1 -type f \
  \( -name '*.deb' -o -name '*.changes' -o -name '*.buildinfo' \) -delete

docker build -t "$IMAGE_TAG" - <<'DOCKERFILE'
FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential \
      ca-certificates \
      cargo \
      debhelper \
      devscripts \
      docbook-xml \
      docbook2x \
      dpkg-dev \
      fakeroot \
      pkg-config \
      rustc \
      texinfo \
      xsltproc \
 && rm -rf /var/lib/apt/lists/*
DOCKERFILE

docker run --rm \
  -u "$(id -u):$(id -g)" \
  -e HOME=/tmp/libbz2-safe-build \
  -v "$ROOT:/work" \
  -w /work/target/package/src \
  "$IMAGE_TAG" \
  bash -lc '
    set -euo pipefail
    mkdir -p "$HOME"
    make -f debian/rules bzip2.info
    dpkg-buildpackage -us -uc -b -nc
  '

shopt -s nullglob
artifacts=( "$PACKAGE_ROOT"/*.deb "$PACKAGE_ROOT"/*.changes "$PACKAGE_ROOT"/*.buildinfo )
(( ${#artifacts[@]} > 0 )) || die "dpkg-buildpackage did not produce package artifacts"
cp -a "${artifacts[@]}" "$OUT/"

version="$(dpkg-parsechangelog -l "$SRC/debian/changelog" -SVersion)"

{
  printf 'version=%s\n' "$version"
  printf 'source_dir=target/package/src\n'
  for deb in "$OUT"/*.deb; do
    pkg="$(dpkg-deb -f "$deb" Package)"
    printf 'package:%s=%s\n' "$pkg" "$(basename "$deb")"
  done

  changes=( "$OUT"/*.changes )
  buildinfos=( "$OUT"/*.buildinfo )
  [[ ${#changes[@]} -eq 1 ]] || die "expected exactly one .changes artifact in $OUT"
  [[ ${#buildinfos[@]} -eq 1 ]] || die "expected exactly one .buildinfo artifact in $OUT"
  printf 'artifact:changes=%s\n' "$(basename "${changes[0]}")"
  printf 'artifact:buildinfo=%s\n' "$(basename "${buildinfos[0]}")"
} > "$MANIFEST"

for pkg in libbz2-1.0 libbz2-dev bzip2 bzip2-doc; do
  deb_name="$(lookup_manifest_value "package:$pkg")"
  [[ -f "$OUT/$deb_name" ]] || die "required package artifact missing from $OUT: $deb_name"
  dpkg-deb -x "$OUT/$deb_name" "$UNPACKED/$pkg"
done
