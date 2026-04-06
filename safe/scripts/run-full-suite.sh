#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cargo check --manifest-path "$ROOT/safe/Cargo.toml"
bash "$ROOT/safe/scripts/build-original-baseline.sh"
bash "$ROOT/safe/scripts/build-safe.sh" --release
bash "$ROOT/safe/scripts/check-abi.sh" --baseline-only
python3 -m json.tool "$ROOT/all_cves.json" >/dev/null
python3 -m json.tool "$ROOT/relevant_cves.json" >/dev/null
python3 -m json.tool "$ROOT/dependents.json" >/dev/null
