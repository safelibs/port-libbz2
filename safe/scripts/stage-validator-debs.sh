#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/target/package/out"
ARTIFACT_ROOT="$ROOT/validator/artifacts/libbz2-safe"
DEB_DEST="$ARTIFACT_ROOT/debs/local/libbz2"
PROOF_DIR="$ARTIFACT_ROOT/proof"
LOCK_PATH="$PROOF_DIR/local-port-debs-lock.json"
REPOSITORY="local/libbz2-port"
RELEASE_TAG="local-libbz2-port"
CANONICAL_PACKAGES=(libbz2-1.0 libbz2-dev bzip2)

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_tool() {
  command -v "$1" >/dev/null 2>&1 || die "missing required host tool: $1"
}

is_canonical_package() {
  local package="$1"

  for canonical in "${CANONICAL_PACKAGES[@]}"; do
    [[ "$package" == "$canonical" ]] && return 0
  done
  return 1
}

require_tool dpkg-deb
require_tool git
require_tool python3

[[ -d "$ROOT/validator/.git" ]] || die "missing validator checkout at $ROOT/validator"
[[ -d "$OUT" ]] || die "missing package output directory: $OUT"

mkdir -p "$DEB_DEST" "$PROOF_DIR" \
  "$ARTIFACT_ROOT/results/libbz2" \
  "$ARTIFACT_ROOT/port/results/libbz2"

rm -f "$DEB_DEST"/*.deb

shopt -s nullglob
source_debs=( "$OUT"/*.deb )
(( ${#source_debs[@]} > 0 )) || die "no .deb files found in $OUT"

declare -A selected_deb_by_package=()
declare -A selected_count_by_package=()

for deb in "${source_debs[@]}"; do
  package="$(dpkg-deb --field "$deb" Package)"
  architecture="$(dpkg-deb --field "$deb" Architecture)"
  [[ -n "$package" ]] || die "unable to read Package field from $deb"
  [[ -n "$architecture" ]] || die "unable to read Architecture field from $deb"

  if ! is_canonical_package "$package"; then
    continue
  fi
  if [[ "$architecture" != "amd64" && "$architecture" != "all" ]]; then
    die "unsupported architecture for $package in $deb: $architecture"
  fi

  selected_count_by_package["$package"]=$(( ${selected_count_by_package["$package"]:-0} + 1 ))
  selected_deb_by_package["$package"]="$deb"
done

for package in "${CANONICAL_PACKAGES[@]}"; do
  count="${selected_count_by_package["$package"]:-0}"
  if [[ "$count" -eq 0 ]]; then
    die "missing canonical package in $OUT: $package"
  fi
  if [[ "$count" -ne 1 ]]; then
    die "expected exactly one $package package in $OUT, found $count"
  fi
  cp -f "${selected_deb_by_package["$package"]}" "$DEB_DEST/"
done

copied_debs=( "$DEB_DEST"/*.deb )
if [[ "${#copied_debs[@]}" -ne "${#CANONICAL_PACKAGES[@]}" ]]; then
  die "expected ${#CANONICAL_PACKAGES[@]} copied .deb files in $DEB_DEST, found ${#copied_debs[@]}"
fi

declare -A copied_count_by_package=()
for deb in "${copied_debs[@]}"; do
  package="$(dpkg-deb --field "$deb" Package)"
  architecture="$(dpkg-deb --field "$deb" Architecture)"
  [[ -n "$package" ]] || die "unable to read Package field from copied deb $deb"
  [[ -n "$architecture" ]] || die "unable to read Architecture field from copied deb $deb"
  is_canonical_package "$package" || die "copied noncanonical package into validator override tree: $package"
  if [[ "$architecture" != "amd64" && "$architecture" != "all" ]]; then
    die "unsupported architecture for copied $package package: $architecture"
  fi
  copied_count_by_package["$package"]=$(( ${copied_count_by_package["$package"]:-0} + 1 ))
done

for package in "${CANONICAL_PACKAGES[@]}"; do
  count="${copied_count_by_package["$package"]:-0}"
  [[ "$count" -eq 1 ]] || die "expected one copied $package package, found $count"
done

commit="$(git -C "$ROOT" rev-parse HEAD)"
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || die "parent repository HEAD is not a full lowercase SHA-1: $commit"
generated_at="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"

export DEB_DEST LOCK_PATH REPOSITORY RELEASE_TAG commit generated_at
python3 - <<'PY'
import hashlib
import json
import os
import subprocess
from pathlib import Path

canonical_packages = ["libbz2-1.0", "libbz2-dev", "bzip2"]
deb_dest = Path(os.environ["DEB_DEST"])
lock_path = Path(os.environ["LOCK_PATH"])
repository = os.environ["REPOSITORY"]
release_tag = os.environ["RELEASE_TAG"]
commit = os.environ["commit"]
generated_at = os.environ["generated_at"]


def deb_field(path: Path, field: str) -> str:
    return subprocess.check_output(
        ["dpkg-deb", "--field", str(path), field],
        text=True,
    ).strip()


debs = []
for package in canonical_packages:
    matches = [
        path
        for path in deb_dest.glob("*.deb")
        if deb_field(path, "Package") == package
    ]
    if len(matches) != 1:
        raise SystemExit(f"expected exactly one copied deb for {package}, found {len(matches)}")
    path = matches[0]
    architecture = deb_field(path, "Architecture")
    if architecture not in {"amd64", "all"}:
        raise SystemExit(f"unsupported architecture for {package}: {architecture}")
    debs.append(
        {
            "package": package,
            "filename": path.name,
            "architecture": architecture,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "size": path.stat().st_size,
        }
    )

lock = {
    "schema_version": 1,
    "mode": "port",
    "generated_at": generated_at,
    "source_config": repository,
    "source_inventory": repository,
    "libraries": [
        {
            "library": "libbz2",
            "repository": repository,
            "tag_ref": f"refs/tags/{release_tag}",
            "commit": commit,
            "release_tag": release_tag,
            "debs": debs,
            "unported_original_packages": [],
        }
    ],
}

tmp_path = lock_path.with_name(f"{lock_path.name}.tmp")
tmp_path.write_text(json.dumps(lock, indent=2) + "\n")
tmp_path.replace(lock_path)
PY

printf 'staged %d validator package(s) in %s\n' "${#CANONICAL_PACKAGES[@]}" "$DEB_DEST"
printf 'wrote validator port deb lock: %s\n' "$LOCK_PATH"
