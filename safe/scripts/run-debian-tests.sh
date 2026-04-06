#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_ROOT="$ROOT/target/package"
SRC="$PACKAGE_ROOT/src"
OUT="$PACKAGE_ROOT/out"
MANIFEST="$OUT/package-manifest.txt"
IMAGE_TAG="${LIBBZ2_DEB_TEST_IMAGE:-libbz2-safe-deb-test:ubuntu24.04}"

DEFAULT_TESTS=(link-with-shared bigfile bzexe-test compare compress grep)
SELECTED_TESTS=()

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

while (($#)); do
  case "$1" in
    --tests)
      shift
      while (($#)); do
        SELECTED_TESTS+=("$1")
        shift
      done
      ;;
    --help|-h)
      cat <<'EOF'
usage: run-debian-tests.sh [--tests <name>...]

Runs the staged Debian autopkgtests against the installed safe packages from
target/package/out/.
EOF
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

[[ -f "$MANIFEST" ]] || die "missing package manifest: $MANIFEST; run bash safe/scripts/build-debs.sh first"
[[ -f "$SRC/debian/control" ]] || die "missing staged Debian control file: $SRC/debian/control"
[[ -f "$SRC/debian/tests/control" ]] || die "missing staged autopkgtest control file: $SRC/debian/tests/control"

for pkg in libbz2-1.0 libbz2-dev bzip2 bzip2-doc; do
  deb_name="$(lookup_manifest_value "package:$pkg")"
  [[ -f "$OUT/$deb_name" ]] || die "required package artifact missing from $OUT: $deb_name"
done

if (( ${#SELECTED_TESTS[@]} == 0 )); then
  SELECTED_TESTS=( "${DEFAULT_TESTS[@]}" )
fi

for test_name in "${SELECTED_TESTS[@]}"; do
  case "$test_name" in
    link-with-shared|bigfile|bzexe-test|compare|compress|grep)
      ;;
    *)
      die "unknown Debian autopkgtest: $test_name"
      ;;
  esac
done

require_builddeps=0
for test_name in "${SELECTED_TESTS[@]}"; do
  if [[ "$test_name" == "link-with-shared" ]]; then
    require_builddeps=1
    break
  fi
done

builddeps="$(
  python3 - "$SRC/debian/control" <<'PY'
import re
import sys
from pathlib import Path

text = Path(sys.argv[1]).read_text(encoding="utf-8")
fields = {}
current = None
for line in text.splitlines():
    if not line:
        current = None
        continue
    if line[0].isspace() and current is not None:
        fields[current] += " " + line.strip()
        continue
    if ":" not in line:
        continue
    key, value = line.split(":", 1)
    current = key
    fields[key] = value.strip()

deps = []
for key in ("Build-Depends", "Build-Depends-Indep"):
    for entry in fields.get(key, "").split(","):
        entry = re.sub(r"\[[^]]*\]", "", entry)
        entry = re.sub(r"<[^>]*>", "", entry)
        entry = entry.strip()
        if not entry:
            continue
        candidate = entry.split("|", 1)[0].strip()
        candidate = re.sub(r"\s*\(.*?\)", "", candidate).strip()
        if candidate == "debhelper-compat":
            candidate = "debhelper"
        if candidate and candidate not in deps:
            deps.append(candidate)

print(" ".join(deps))
PY
)"

package_paths=()
for pkg in libbz2-1.0 libbz2-dev bzip2 bzip2-doc; do
  package_paths+=( "/work/target/package/out/$(lookup_manifest_value "package:$pkg")" )
done

docker build -t "$IMAGE_TAG" - <<'DOCKERFILE'
FROM ubuntu:24.04

ARG DEBIAN_FRONTEND=noninteractive

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
DOCKERFILE

tests_string="${SELECTED_TESTS[*]}"
deb_paths_string="${package_paths[*]}"

docker run --rm \
  -e "LIBBZ2_AUTOPKGTESTS=$tests_string" \
  -e "LIBBZ2_BUILDDEPS=$builddeps" \
  -e "LIBBZ2_REQUIRE_BUILDDEPS=$require_builddeps" \
  -e "LIBBZ2_PACKAGE_DEBS=$deb_paths_string" \
  -v "$ROOT:/work:ro" \
  "$IMAGE_TAG" \
  bash -s <<'CONTAINER'
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

apt-get update

if [[ "${LIBBZ2_REQUIRE_BUILDDEPS}" == "1" ]]; then
  apt-get install -y --no-install-recommends build-essential ${LIBBZ2_BUILDDEPS}
fi

apt-get install -y --no-install-recommends ${LIBBZ2_PACKAGE_DEBS}

for test_name in ${LIBBZ2_AUTOPKGTESTS}; do
  export AUTOPKGTEST_TMP="/tmp/libbz2-autopkgtest/${test_name}"
  rm -rf "$AUTOPKGTEST_TMP"
  mkdir -p "$AUTOPKGTEST_TMP"
  /bin/sh "/work/safe/debian/tests/${test_name}"
done
CONTAINER
