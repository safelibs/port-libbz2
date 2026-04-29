#!/usr/bin/env bash
# Build the safe port via dpkg-buildpackage rooted in safe/.
# Stamps the changelog with `+safelibs<commit-epoch>` so the produced
# .deb files have a deterministic version that wins over Ubuntu's copy
# under the apt pin in safelibs/apt.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
dist_dir="$repo_root/dist"

# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

if [[ -d "$HOME/.cargo/bin" ]]; then
  case ":$PATH:" in
    *":$HOME/.cargo/bin:"*) ;;
    *) export PATH="$HOME/.cargo/bin:$PATH" ;;
  esac
fi

rm -rf -- "$dist_dir"
mkdir -p -- "$dist_dir"

# libbz2-specific setup: stage upstream assets that the safe debian
# packaging consumes from safe/. Idempotent: re-copies on each run.
python3 - <<'PY'
from pathlib import Path
import shutil

repo_root = Path(".")
copies = {
    "original/bzip2.c": "safe/bzip2.c",
    "original/bzip2recover.c": "safe/bzip2recover.c",
    "original/bzdiff": "safe/bzdiff",
    "original/bzgrep": "safe/bzgrep",
    "original/bzmore": "safe/bzmore",
    "original/bzip2.1": "safe/bzip2.1",
    "original/bzgrep.1": "safe/bzgrep.1",
    "original/bzmore.1": "safe/bzmore.1",
    "original/bzdiff.1": "safe/bzdiff.1",
    "original/manual.xml": "safe/manual.xml",
    "original/entities.xml": "safe/entities.xml",
    "original/manual.html": "safe/manual.html",
    "original/manual.pdf": "safe/manual.pdf",
    "original/manual.ps": "safe/manual.ps",
    "original/debian/bzexe": "safe/debian/bzexe",
    "original/debian/bzexe.1": "safe/debian/bzexe.1",
    "original/debian/bzip2-doc.docs": "safe/debian/bzip2-doc.docs",
    "original/debian/bzip2-doc.doc-base": "safe/debian/bzip2-doc.doc-base",
    "original/debian/bzip2-doc.info": "safe/debian/bzip2-doc.info",
}

for source_rel, dest_rel in copies.items():
    source = repo_root / source_rel
    dest = repo_root / dest_rel
    if not source.exists():
        raise SystemExit(f"missing required staged asset: {source}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, dest)
PY

cd "$repo_root/safe"

upstream_version="$(dpkg-parsechangelog -S Version | sed -E 's/\+safelibs[0-9]+$//')"
package_name="$(dpkg-parsechangelog -S Source)"
distribution="$(dpkg-parsechangelog -S Distribution)"

if [[ -n "${SAFELIBS_COMMIT_SHA:-}" ]] \
   && command -v git >/dev/null 2>&1 \
   && git -C "$repo_root" cat-file -e "$SAFELIBS_COMMIT_SHA^{commit}" 2>/dev/null; then
  commit_epoch="$(git -C "$repo_root" log -1 --format=%ct "$SAFELIBS_COMMIT_SHA")"
elif command -v git >/dev/null 2>&1 && git -C "$repo_root" rev-parse HEAD >/dev/null 2>&1; then
  commit_epoch="$(git -C "$repo_root" log -1 --format=%ct HEAD)"
else
  commit_epoch="$(date -u +%s)"
fi

new_version="${upstream_version}+safelibs${commit_epoch}"
release_date="$(date -u -R -d "@${commit_epoch}")"

{
  printf '%s (%s) %s; urgency=medium\n\n  * Automated SafeLibs rebuild.\n\n -- SafeLibs CI <ci@safelibs.org>  %s\n\n' \
    "$package_name" "$new_version" "$distribution" "$release_date"
  cat debian/changelog
} > debian/changelog.new
mv debian/changelog.new debian/changelog

sudo mk-build-deps -i -r -t "apt-get -y --no-install-recommends" debian/control
dpkg-buildpackage -us -uc -b

cp -v ../*.deb "$dist_dir"/
