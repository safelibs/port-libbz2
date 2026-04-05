#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
IMAGE_TAG="${LIBBZ2_ORIGINAL_TEST_IMAGE:-libbz2-original-test:ubuntu24.04}"
ONLY=""

usage() {
  cat <<'EOF'
usage: test-original.sh [--only <binary-package>]

Builds and installs the vendored original libbz2 inside Docker, then smoke-tests
the Ubuntu 24.04 dependent packages recorded in dependents.json against that
local libbz2 build.

--only runs just one dependent by exact .dependents[].binary_package.
EOF
}

while (($#)); do
  case "$1" in
    --only)
      ONLY="${2:?missing value for --only}"
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      printf 'unknown option: %s\n' "$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

for tool in docker python3; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'missing required host tool: %s\n' "$tool" >&2
    exit 1
  }
done

[[ -d "$ROOT/original" ]] || {
  echo "missing original source tree" >&2
  exit 1
}

[[ -f "$ROOT/dependents.json" ]] || {
  echo "missing dependents.json" >&2
  exit 1
}

python3 - "$ROOT/dependents.json" "$ONLY" <<'PY'
import json
import sys
from pathlib import Path

expected = [
    "libapt-pkg6.0t64",
    "bzip2",
    "libpython3.12-stdlib",
    "php8.3-bz2",
    "pike8.0-bzip2",
    "libcompress-raw-bzip2-perl",
    "mariadb-plugin-provider-bzip2",
    "gpg",
    "zip",
    "unzip",
    "libarchive13t64",
    "libfreetype6",
    "gstreamer1.0-plugins-good",
]

data = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
actual = [entry["binary_package"] for entry in data["dependents"]]

if actual != expected:
    raise SystemExit(
        f"unexpected dependents.json contents: expected {expected}, found {actual}"
    )

only = sys.argv[2]
if only and only not in set(actual):
    raise SystemExit(f"unknown --only binary package: {only}")
PY

docker build -t "$IMAGE_TAG" - <<'DOCKERFILE'
FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN sed -i 's/^Types: deb$/Types: deb deb-src/' /etc/apt/sources.list.d/ubuntu.sources \
 && apt-get update \
 && apt-get install -y --no-install-recommends \
      bzip2 \
      build-essential \
      ca-certificates \
      gnupg \
      gstreamer1.0-plugins-good \
      gstreamer1.0-tools \
      gzip \
      libarchive-dev \
      libarchive-tools \
      libcompress-raw-bzip2-perl \
      libfreetype-dev \
      mariadb-client \
      mariadb-plugin-provider-bzip2 \
      mariadb-server \
      php8.3-bz2 \
      php8.3-cli \
      pike8.0 \
      pike8.0-bzip2 \
      pkg-config \
      python3 \
      unzip \
      xfonts-base \
      zip \
 && rm -rf /var/lib/apt/lists/*
DOCKERFILE

docker run --rm -i \
  -e "LIBBZ2_TEST_ONLY=$ONLY" \
  -v "$ROOT:/work:ro" \
  "$IMAGE_TAG" \
  bash -s <<'CONTAINER'
set -Eeuo pipefail

export LANG=C.UTF-8
export LC_ALL=C.UTF-8
export DEBIAN_FRONTEND=noninteractive

READ_ONLY_ROOT=/work
BUILD_ROOT=/tmp/libbz2-original
TEST_ROOT=/tmp/libbz2-dependent-tests
ONLY="${LIBBZ2_TEST_ONLY:-}"
CURRENT_STEP=""
MULTIARCH="$(gcc -print-multiarch)"
ACTIVE_LIBBZ2=""
APT_LIB="/usr/lib/${MULTIARCH}/libapt-pkg.so.6.0"
LIBARCHIVE_SO="/usr/lib/${MULTIARCH}/libarchive.so.13"
FREETYPE_SO="/usr/lib/${MULTIARCH}/libfreetype.so.6"
GST_MATROSKA_SO="/usr/lib/${MULTIARCH}/gstreamer-1.0/libgstmatroska.so"

trap 'rc=$?; if [[ "$rc" -ne 0 && -n "$CURRENT_STEP" ]]; then printf "failed during: %s\n" "$CURRENT_STEP" >&2; fi; exit "$rc"' EXIT

log_step() {
  printf '\n==> %s\n' "$1"
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_nonempty_file() {
  local path="$1"

  [[ -s "$path" ]] || die "expected non-empty file: $path"
}

require_contains() {
  local path="$1"
  local needle="$2"

  if ! grep -F -- "$needle" "$path" >/dev/null 2>&1; then
    printf 'missing expected text in %s: %s\n' "$path" "$needle" >&2
    printf -- '--- %s ---\n' "$path" >&2
    cat "$path" >&2
    exit 1
  fi
}

should_run() {
  local package="$1"

  [[ -z "$ONLY" || "$ONLY" == "$package" ]]
}

reset_test_dir() {
  local name="$1"
  local dir="$TEST_ROOT/$name"

  rm -rf "$dir"
  mkdir -p "$dir"
  printf '%s\n' "$dir"
}

assert_links_to_active_libbz2() {
  local target="$1"
  local resolved=""

  [[ -e "$target" ]] || die "missing link target: $target"

  resolved="$(ldd "$target" | awk '$1 == "libbz2.so.1.0" { print $3; exit }')"
  [[ -n "$resolved" ]] || die "ldd did not report libbz2.so.1.0 for $target"
  resolved="$(readlink -f "$resolved")"
  [[ "$resolved" == "$ACTIVE_LIBBZ2" ]] || {
    printf 'expected %s to resolve libbz2.so.1.0 from %s, got %s\n' "$target" "$ACTIVE_LIBBZ2" "$resolved" >&2
    ldd "$target" >&2
    exit 1
  }
}

build_original_libbz2() {
  log_step "Building and installing original libbz2"
  rm -rf "$BUILD_ROOT" "$TEST_ROOT"
  mkdir -p "$TEST_ROOT"
  cp -a "$READ_ONLY_ROOT/original" "$BUILD_ROOT"
  cd "$BUILD_ROOT"
  make clean >/tmp/libbz2-make-clean.log 2>&1
  make -j"$(nproc)" >/tmp/libbz2-make.log 2>&1
  make install PREFIX=/usr/local >/tmp/libbz2-install.log 2>&1
  printf '/usr/local/lib\n' >/etc/ld.so.conf.d/000-local-libbz2.conf
  ldconfig
  ACTIVE_LIBBZ2="$(readlink -f /usr/local/lib/libbz2.so.1.0)"
  [[ -n "$ACTIVE_LIBBZ2" && -f "$ACTIVE_LIBBZ2" ]] || die "failed to install local libbz2 shared library"
  cd /
}

test_libapt_pkg() {
  local dir

  log_step "libapt-pkg6.0t64"
  assert_links_to_active_libbz2 "$APT_LIB"
  dir="$(reset_test_dir apt)"
  mkdir -p \
    "$dir/repo/dists/stable/main/binary-amd64" \
    "$dir/root/state/lists/partial" \
    "$dir/root/cache/archives/partial" \
    "$dir/root/etc/apt/sources.list.d"

  : >"$dir/repo/dists/stable/main/binary-amd64/Packages"
  bzip2 -9 -c "$dir/repo/dists/stable/main/binary-amd64/Packages" >"$dir/repo/dists/stable/main/binary-amd64/Packages.bz2"
  cat >"$dir/repo/dists/stable/Release" <<'EOF'
Origin: libbz2 smoke test
Label: libbz2 smoke test
Suite: stable
Codename: stable
Architectures: amd64
Components: main
Date: Sun, 01 Jan 2023 00:00:00 UTC
EOF
  cat >"$dir/root/etc/apt/sources.list" <<'EOF'
deb [trusted=yes] http://127.0.0.1:18080 stable main
EOF

  (
    set -euo pipefail
    cd "$dir/repo"
    python3 -m http.server 18080 >"$dir/http.log" 2>&1 &
    http_pid="$!"
    trap 'kill "$http_pid" >/dev/null 2>&1 || true; wait "$http_pid" >/dev/null 2>&1 || true' EXIT
    sleep 1
    timeout 60 apt-get \
      -o Dir::State="$dir/root/state" \
      -o Dir::Cache="$dir/root/cache" \
      -o Dir::Etc::sourcelist="$dir/root/etc/apt/sources.list" \
      -o Dir::Etc::sourceparts="$dir/root/etc/apt/sources.list.d" \
      -o APT::Get::List-Cleanup=0 \
      update >"$dir/apt-update.log" 2>&1
  )

  require_contains "$dir/http.log" "Packages.bz2"
  require_contains "$dir/apt-update.log" "Reading package lists..."
}

test_bzip2_cli() {
  local dir

  log_step "bzip2"
  assert_links_to_active_libbz2 "$(command -v bzip2)"
  dir="$(reset_test_dir bzip2)"
  printf 'bzip2 cli smoke test\n%.0s' {1..200} >"$dir/input.txt"
  bzip2 -k "$dir/input.txt"
  bzip2 -t "$dir/input.txt.bz2"
  bunzip2 -c "$dir/input.txt.bz2" >"$dir/output.txt"
  bzcat "$dir/input.txt.bz2" >"$dir/output-bzcat.txt"
  cmp "$dir/input.txt" "$dir/output.txt"
  cmp "$dir/input.txt" "$dir/output-bzcat.txt"
}

test_python_bz2() {
  local dir
  local py_ext

  log_step "libpython3.12-stdlib"
  py_ext="$(python3 - <<'PY'
import _bz2
print(_bz2.__file__)
PY
)"
  assert_links_to_active_libbz2 "$py_ext"
  dir="$(reset_test_dir python)"
  python3 - "$dir" <<'PY'
import bz2
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
payload = ("python bz2 smoke\n" * 128).encode()
compressed = bz2.compress(payload, compresslevel=9)
assert bz2.decompress(compressed) == payload

archive = root / "payload.txt.bz2"
with bz2.open(archive, "wb", compresslevel=9) as handle:
    handle.write(payload)

with bz2.open(archive, "rb") as handle:
    restored = handle.read()

assert restored == payload
assert archive.stat().st_size > 0
PY
}

test_php_bz2() {
  local dir
  local extension_dir

  log_step "php8.3-bz2"
  extension_dir="$(php -r 'echo ini_get("extension_dir"), PHP_EOL;')"
  assert_links_to_active_libbz2 "${extension_dir}/bz2.so"
  dir="$(reset_test_dir php)"
  php -r '
$dir = $argv[1];
$payload = str_repeat("php bz2 smoke\n", 128);
$compressed = bzcompress($payload, 9);
if (!is_string($compressed)) {
    fwrite(STDERR, "bzcompress failed\n");
    exit(1);
}
if (bzdecompress($compressed) !== $payload) {
    fwrite(STDERR, "bzdecompress mismatch\n");
    exit(1);
}
$path = $dir . "/payload.txt.bz2";
$writer = bzopen($path, "w");
if ($writer === false) {
    fwrite(STDERR, "bzopen write failed\n");
    exit(1);
}
if (bzwrite($writer, $payload) !== strlen($payload)) {
    fwrite(STDERR, "bzwrite failed\n");
    exit(1);
}
bzclose($writer);
$reader = bzopen($path, "r");
if ($reader === false) {
    fwrite(STDERR, "bzopen read failed\n");
    exit(1);
}
$decoded = "";
while (true) {
    $chunk = bzread($reader, 4096);
    if (!is_string($chunk)) {
        fwrite(STDERR, "bzread failed\n");
        exit(1);
    }
    if ($chunk === "") {
        break;
    }
    $decoded .= $chunk;
}
bzclose($reader);
if ($decoded !== $payload) {
    fwrite(STDERR, "file round-trip mismatch\n");
    exit(1);
}
' "$dir"
  bzip2 -t "$dir/payload.txt.bz2"
}

test_pike_bz2() {
  local dir
  local module_so

  log_step "pike8.0-bzip2"
  module_so="$(find /usr/lib/pike8.0/modules -maxdepth 1 -name '___Bz2.so' | head -n1)"
  [[ -n "$module_so" ]] || die "unable to locate Pike bzip2 module"
  assert_links_to_active_libbz2 "$module_so"
  dir="$(reset_test_dir pike)"
  cat >"$dir/pike-bz2-smoke.pike" <<'PIKE'
int main(int argc, array(string) argv)
{
  string out_path = argv[1] + "/payload.txt.bz2";
  string payload = "pike bz2 smoke\n" * 128;
  object writer = Bz2.File();
  writer->write_open(out_path);
  writer->write(payload);
  writer->close();

  object reader = Bz2.File();
  reader->read_open(out_path);
  string decoded = reader->read();
  reader->close();

  if (decoded != payload) {
    werror("Pike bzip2 round-trip mismatch\n");
    return 1;
  }

  return 0;
}
PIKE
  pike "$dir/pike-bz2-smoke.pike" "$dir"
}

test_perl_bz2() {
  local dir
  local module_so

  log_step "libcompress-raw-bzip2-perl"
  module_so="$(find /usr/lib -path '*/auto/Compress/Raw/Bzip2/Bzip2.so' | head -n1)"
  [[ -n "$module_so" ]] || die "unable to locate Perl Bzip2 XS module"
  assert_links_to_active_libbz2 "$module_so"
  dir="$(reset_test_dir perl)"
  perl - "$dir" <<'PERL'
use strict;
use warnings;
use Compress::Raw::Bzip2;

my $payload = "perl bz2 smoke\n" x 128;
my ($bz, $status) = new Compress::Raw::Bzip2(1, 1, 0);
die "compress init failed: $status\n" unless $status == BZ_OK;

my $compressed = q();
$status = $bz->bzdeflate($payload, $compressed);
die "bzdeflate failed: $status\n" unless $status == BZ_RUN_OK;

my $tail = q();
$status = $bz->bzclose($tail);
die "bzclose failed: $status\n" unless $status == BZ_STREAM_END;
$compressed .= $tail;

my ($bun, $inflate_status) = new Compress::Raw::Bunzip2(1, 0, 0, 0, 0);
die "decompress init failed: $inflate_status\n" unless $inflate_status == BZ_OK;

my $decoded = q();
$inflate_status = $bun->bzinflate($compressed, $decoded);
die "bzinflate failed: $inflate_status\n" unless $inflate_status == BZ_STREAM_END;
die "payload mismatch\n" unless $decoded eq $payload;
PERL
}

test_mariadb_provider() {
  local dir

  log_step "mariadb-plugin-provider-bzip2"
  assert_links_to_active_libbz2 /usr/lib/mysql/plugin/provider_bzip2.so
  dir="$(reset_test_dir mariadb)"
  rm -f /tmp/mysqld.pid /tmp/mysqld.sock
  install -d -o mysql -g mysql /run/mysqld
  mariadbd \
    --user=mysql \
    --skip-networking \
    --socket=/tmp/mysqld.sock \
    --pid-file=/tmp/mysqld.pid >"$dir/mariadbd.log" 2>&1 &
  local mariadb_pid="$!"
  trap 'kill "$mariadb_pid" >/dev/null 2>&1 || true; wait "$mariadb_pid" >/dev/null 2>&1 || true' RETURN

  for _ in $(seq 1 30); do
    if mysqladmin --socket=/tmp/mysqld.sock ping >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
  mysqladmin --socket=/tmp/mysqld.sock ping >/dev/null
  mariadb --socket=/tmp/mysqld.sock -NBe \
    "SELECT PLUGIN_NAME, PLUGIN_STATUS FROM INFORMATION_SCHEMA.PLUGINS WHERE PLUGIN_NAME='provider_bzip2';" \
    >"$dir/provider-status.txt"
  require_contains "$dir/provider-status.txt" $'provider_bzip2\tACTIVE'

  trap - RETURN
  kill "$mariadb_pid" >/dev/null 2>&1 || true
  wait "$mariadb_pid" >/dev/null 2>&1 || true
}

test_gpg_bzip2() {
  local dir

  log_step "gpg"
  assert_links_to_active_libbz2 "$(command -v gpg)"
  dir="$(reset_test_dir gpg)"
  mkdir -p "$dir/gnupg"
  chmod 700 "$dir/gnupg"
  printf 'gpg bzip2 smoke\n%.0s' {1..200} >"$dir/input.txt"
  GNUPGHOME="$dir/gnupg" gpg \
    --batch \
    --yes \
    --pinentry-mode loopback \
    --passphrase testpass \
    --symmetric \
    --cipher-algo AES256 \
    --compress-algo bzip2 \
    --bzip2-compress-level 9 \
    -o "$dir/data.gpg" \
    "$dir/input.txt" >"$dir/encrypt.log" 2>&1
  GNUPGHOME="$dir/gnupg" gpg \
    --batch \
    --yes \
    --pinentry-mode loopback \
    --passphrase testpass \
    --list-packets "$dir/data.gpg" >"$dir/packets.txt" 2>&1
  require_contains "$dir/packets.txt" "compressed packet: algo=3"
  GNUPGHOME="$dir/gnupg" gpg \
    --batch \
    --yes \
    --pinentry-mode loopback \
    --passphrase testpass \
    -o "$dir/output.txt" \
    -d "$dir/data.gpg" >"$dir/decrypt.log" 2>&1
  cmp "$dir/input.txt" "$dir/output.txt"
}

test_zip_bzip2() {
  local dir

  log_step "zip"
  assert_links_to_active_libbz2 "$(command -v zip)"
  dir="$(reset_test_dir zip)"
  printf 'zip bzip2 smoke\n%.0s' {1..120} >"$dir/input.txt"
  (
    cd "$dir"
    zip -q -Z bzip2 archive.zip input.txt
    zip -T archive.zip >"$dir/zip-test.log" 2>&1
    zipinfo -v archive.zip >"$dir/zipinfo.log" 2>&1
  )
  require_contains "$dir/zipinfo.log" "compression method:                             bzipped"
  require_contains "$dir/zip-test.log" "test of archive.zip OK"
}

test_unzip_bzip2() {
  local dir

  log_step "unzip"
  assert_links_to_active_libbz2 "$(command -v unzip)"
  dir="$(reset_test_dir unzip)"
  printf 'unzip bzip2 smoke\n%.0s' {1..120} >"$dir/input.txt"
  (
    cd "$dir"
    zip -q -Z bzip2 archive.zip input.txt
    unzip -p archive.zip input.txt >output.txt
    unzip -v archive.zip >unzip-info.log
  )
  cmp "$dir/input.txt" "$dir/output.txt"
  require_contains "$dir/unzip-info.log" "BZip2"
}

test_libarchive_bzip2() {
  local dir

  log_step "libarchive13t64"
  assert_links_to_active_libbz2 "$LIBARCHIVE_SO"
  dir="$(reset_test_dir libarchive)"
  cat >"$dir/libarchive-bz2-smoke.c" <<'C'
#include <archive.h>
#include <archive_entry.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv)
{
  const char *root = argv[1];
  const char *payload = "libarchive via bzip2\n";
  char archive_path[4096];
  struct archive *writer = archive_write_new();
  struct archive_entry *entry = archive_entry_new();

  snprintf(archive_path, sizeof(archive_path), "%s/archive.tar.bz2", root);
  archive_write_set_format_pax_restricted(writer);
  if (archive_write_add_filter_bzip2(writer) != ARCHIVE_OK)
    return 1;
  if (archive_write_open_filename(writer, archive_path) != ARCHIVE_OK)
    return 1;

  archive_entry_set_pathname(entry, "payload.txt");
  archive_entry_set_filetype(entry, AE_IFREG);
  archive_entry_set_perm(entry, 0644);
  archive_entry_set_size(entry, strlen(payload));
  if (archive_write_header(writer, entry) != ARCHIVE_OK)
    return 1;
  if (archive_write_data(writer, payload, strlen(payload)) < 0)
    return 1;

  archive_entry_free(entry);
  archive_write_close(writer);
  archive_write_free(writer);

  struct archive *reader = archive_read_new();
  struct archive_entry *read_entry = NULL;
  char buffer[128] = {0};
  archive_read_support_format_tar(reader);
  archive_read_support_filter_bzip2(reader);
  if (archive_read_open_filename(reader, archive_path, 10240) != ARCHIVE_OK)
    return 1;
  if (archive_read_next_header(reader, &read_entry) != ARCHIVE_OK)
    return 1;
  if (archive_read_data(reader, buffer, sizeof(buffer)) < 0)
    return 1;
  archive_read_close(reader);
  archive_read_free(reader);

  if (strcmp(buffer, payload) != 0)
    return 1;

  return 0;
}
C
  cc "$dir/libarchive-bz2-smoke.c" -o "$dir/libarchive-bz2-smoke" -larchive
  "$dir/libarchive-bz2-smoke" "$dir"
  require_nonempty_file "$dir/archive.tar.bz2"
}

test_freetype_bzip2() {
  local dir

  log_step "libfreetype6"
  assert_links_to_active_libbz2 "$FREETYPE_SO"
  dir="$(reset_test_dir freetype)"
  gzip -dc /usr/share/fonts/X11/misc/6x13B-ISO8859-14.pcf.gz | bzip2 -9 >"$dir/test-font.pcf.bz2"
  cat >"$dir/freetype-bz2-smoke.c" <<'C'
#include <stdio.h>
#include <ft2build.h>
#include FT_FREETYPE_H

int main(int argc, char **argv)
{
  FT_Library library;
  FT_Face face;

  if (FT_Init_FreeType(&library))
    return 1;
  if (FT_New_Face(library, argv[1], 0, &face))
    return 1;
  if (face->family_name == NULL || face->family_name[0] == '\0')
    return 1;

  printf("family=%s\n", face->family_name);
  FT_Done_Face(face);
  FT_Done_FreeType(library);
  return 0;
}
C
  cc $(pkg-config --cflags freetype2) "$dir/freetype-bz2-smoke.c" -o "$dir/freetype-bz2-smoke" $(pkg-config --libs freetype2)
  "$dir/freetype-bz2-smoke" "$dir/test-font.pcf.bz2" >"$dir/freetype.log"
  require_contains "$dir/freetype.log" "family="
}

test_gstreamer_matroska() {
  local dir

  log_step "gstreamer1.0-plugins-good"
  assert_links_to_active_libbz2 "$GST_MATROSKA_SO"
  dir="$(reset_test_dir gstreamer)"
  gst-inspect-1.0 matroska >"$dir/gst-inspect.log" 2>&1
  require_contains "$dir/gst-inspect.log" "Matroska and WebM stream handling"
  gst-launch-1.0 -q \
    audiotestsrc num-buffers=20 ! \
    vorbisenc ! \
    matroskamux ! \
    filesink location="$dir/plain.mkv"
  require_nonempty_file "$dir/plain.mkv"
  gst-launch-1.0 -q \
    filesrc location="$dir/plain.mkv" ! \
    matroskademux ! \
    fakesink
}

run_test() {
  local package="$1"
  local fn="$2"

  if should_run "$package"; then
    CURRENT_STEP="$package"
    "$fn"
  fi
}

build_original_libbz2
run_test "libapt-pkg6.0t64" test_libapt_pkg
run_test "bzip2" test_bzip2_cli
run_test "libpython3.12-stdlib" test_python_bz2
run_test "php8.3-bz2" test_php_bz2
run_test "pike8.0-bzip2" test_pike_bz2
run_test "libcompress-raw-bzip2-perl" test_perl_bz2
run_test "mariadb-plugin-provider-bzip2" test_mariadb_provider
run_test "gpg" test_gpg_bzip2
run_test "zip" test_zip_bzip2
run_test "unzip" test_unzip_bzip2
run_test "libarchive13t64" test_libarchive_bzip2
run_test "libfreetype6" test_freetype_bzip2
run_test "gstreamer1.0-plugins-good" test_gstreamer_matroska
CURRENT_STEP=""
CONTAINER
