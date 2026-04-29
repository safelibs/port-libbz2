#!/usr/bin/env bash
# libbz2: stage upstream assets that the safe debian packaging consumes
# from safe/, then run the standard safe-debian build via the shared
# helper. Idempotent: re-copies on each run.
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=/dev/null
. "$repo_root/scripts/lib/build-deb-common.sh"

prepare_rust_env
prepare_dist_dir "$repo_root"

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
stamp_safelibs_changelog "$repo_root"
build_with_dpkg_buildpackage "$repo_root"
