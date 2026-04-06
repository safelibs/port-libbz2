#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PACKAGE_ROOT="$ROOT/target/package"
SRC="$PACKAGE_ROOT/src"
OUT="$PACKAGE_ROOT/out"
MANIFEST="$OUT/package-manifest.txt"
IMAGE_TAG="${LIBBZ2_DEB_TEST_IMAGE:-libbz2-safe-deb-test:ubuntu24.04}"

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
[[ "$(lookup_manifest_value "source_dir")" == "target/package/src" ]] || {
  die "package manifest points at an unexpected source_dir; expected target/package/src"
}
[[ -n "$(lookup_manifest_value "version")" ]] || die "package manifest is missing a version entry"

for pkg in libbz2-1.0 libbz2-dev bzip2 bzip2-doc; do
  deb_name="$(lookup_manifest_value "package:$pkg")"
  [[ -f "$OUT/$deb_name" ]] || die "required package artifact missing from $OUT: $deb_name"
done

autopkgtest_metadata="$(
  python3 - "$SRC/debian/tests/control" "$SRC/debian/control" "${SELECTED_TESTS[@]}" <<'PY'
import re
import sys
from pathlib import Path

def parse_debian_control(path: Path) -> list[dict[str, str]]:
    paragraphs: list[dict[str, str]] = []
    fields: dict[str, str] = {}
    current: str | None = None

    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            if fields:
                paragraphs.append(fields)
                fields = {}
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

    if fields:
        paragraphs.append(fields)

    return paragraphs


def normalize_dependency(entry: str) -> str:
    entry = re.sub(r"\[[^]]*\]", "", entry)
    entry = re.sub(r"<[^>]*>", "", entry)
    entry = entry.strip()
    if not entry:
        return ""
    candidate = entry.split("|", 1)[0].strip()
    candidate = re.sub(r"\s*\(.*?\)", "", candidate).strip()
    if candidate == "debhelper-compat":
        candidate = "debhelper"
    return candidate


tests_control = parse_debian_control(Path(sys.argv[1]))
source_control = parse_debian_control(Path(sys.argv[2]))
requested = sys.argv[3:]

available_tests: list[str] = []
test_dependencies: dict[str, list[str]] = {}
for paragraph in tests_control:
    names = paragraph.get("Tests", "").split()
    depends = [item.strip() for item in paragraph.get("Depends", "").split(",") if item.strip()]
    for name in names:
        available_tests.append(name)
        test_dependencies[name] = depends

if not available_tests:
    raise SystemExit("no Debian autopkgtests declared in debian/tests/control")

selected = requested or available_tests
unknown = [name for name in selected if name not in test_dependencies]
if unknown:
    raise SystemExit(
        "unknown Debian autopkgtest(s): " + ", ".join(unknown)
    )

builddeps: list[str] = []
if source_control:
    source_fields = source_control[0]
    for key in ("Build-Depends", "Build-Depends-Indep"):
        for entry in source_fields.get(key, "").split(","):
            candidate = normalize_dependency(entry)
            if candidate and candidate not in builddeps:
                builddeps.append(candidate)

apt_deps: list[str] = []
needs_packages = False
for test_name in selected:
    for entry in test_dependencies[test_name]:
        candidate = normalize_dependency(entry)
        if not candidate:
            continue
        if candidate == "@":
            needs_packages = True
            continue
        if candidate == "@builddeps@":
            for dep in builddeps:
                if dep not in apt_deps:
                    apt_deps.append(dep)
            continue
        if candidate not in apt_deps:
            apt_deps.append(candidate)

print("tests=" + " ".join(selected))
print("apt_deps=" + " ".join(apt_deps))
print("needs_packages=" + ("1" if needs_packages else "0"))
PY
)"

resolved_tests=""
apt_deps=""
needs_packages="0"

while IFS='=' read -r key value; do
  case "$key" in
    tests)
      resolved_tests="$value"
      ;;
    apt_deps)
      apt_deps="$value"
      ;;
    needs_packages)
      needs_packages="$value"
      ;;
    *)
      ;;
  esac
done <<< "$autopkgtest_metadata"

[[ -n "$resolved_tests" ]] || die "failed to resolve Debian autopkgtests from $SRC/debian/tests/control"

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

tests_string="$resolved_tests"
deb_paths_string="${package_paths[*]}"

docker run --rm -i \
  -e "LIBBZ2_AUTOPKGTESTS=$tests_string" \
  -e "LIBBZ2_APT_DEPS=$apt_deps" \
  -e "LIBBZ2_NEEDS_PACKAGES=$needs_packages" \
  -e "LIBBZ2_PACKAGE_DEBS=$deb_paths_string" \
  -v "$ROOT:/work:ro" \
  "$IMAGE_TAG" \
  bash -s <<'CONTAINER'
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

apt-get update

if [[ -n "${LIBBZ2_APT_DEPS}" ]]; then
  apt-get install -y --no-install-recommends ${LIBBZ2_APT_DEPS}
fi

if [[ "${LIBBZ2_NEEDS_PACKAGES}" == "1" ]]; then
  apt-get install -y --no-install-recommends ${LIBBZ2_PACKAGE_DEBS}
fi

for test_name in ${LIBBZ2_AUTOPKGTESTS}; do
  export AUTOPKGTEST_TMP="/tmp/libbz2-autopkgtest/${test_name}"
  rm -rf "$AUTOPKGTEST_TMP"
  mkdir -p "$AUTOPKGTEST_TMP"
  /bin/sh "/work/target/package/src/debian/tests/${test_name}"
done
CONTAINER
