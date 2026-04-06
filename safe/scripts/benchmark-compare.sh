#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT="$ROOT/target/bench"
WORK="$OUT/work"
TMP="$OUT/tmp"
BASELINE_DIR="$ROOT/target/original-baseline"
SAFE_DIR="$ROOT/target/compat"
RUNS="${LIBBZ2_BENCH_RUNS:-3}"

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

require_file() {
  [[ -e "$1" ]] || die "missing required path: $1"
}

command -v python3 >/dev/null 2>&1 || die "missing required host tool: python3"
[[ "$RUNS" =~ ^[1-9][0-9]*$ ]] || die "LIBBZ2_BENCH_RUNS must be a positive integer"

if [[ ! -f "$SAFE_DIR/libbz2.so.1.0.4" ]]; then
  bash "$ROOT/safe/scripts/build-safe.sh" --release >/dev/null
fi

if [[ ! -x "$SAFE_DIR/bzip2" ]]; then
  bash "$ROOT/safe/scripts/build-original-cli-against-safe.sh" >/dev/null
fi

require_file "$BASELINE_DIR/bzip2"
require_file "$BASELINE_DIR/libbz2.so.1.0.4"
require_file "$SAFE_DIR/bzip2"
require_file "$SAFE_DIR/libbz2.so.1.0.4"

rm -rf "$OUT"
mkdir -p "$WORK" "$TMP"

python3 - "$ROOT" "$OUT" "$WORK" "$TMP" "$RUNS" <<'PY'
from __future__ import annotations

import hashlib
import json
import os
import random
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

root = Path(sys.argv[1])
out = Path(sys.argv[2])
work = Path(sys.argv[3])
tmp = Path(sys.argv[4])
runs = int(sys.argv[5])

baseline_dir = root / "target/original-baseline"
safe_dir = root / "target/compat"
baseline_cli = baseline_dir / "bzip2"
safe_cli = safe_dir / "bzip2"

baseline_env = os.environ.copy()
safe_env = os.environ.copy()
baseline_env["LD_LIBRARY_PATH"] = str(baseline_dir) + (
    os.pathsep + baseline_env["LD_LIBRARY_PATH"] if baseline_env.get("LD_LIBRARY_PATH") else ""
)
safe_env["LD_LIBRARY_PATH"] = str(safe_dir) + (
    os.pathsep + safe_env["LD_LIBRARY_PATH"] if safe_env.get("LD_LIBRARY_PATH") else ""
)


def repeat_to_size(target_size: int, chunks: list[bytes]) -> bytes:
    output = bytearray()
    idx = 0
    while len(output) < target_size:
        output.extend(chunks[idx % len(chunks)])
        idx += 1
    return bytes(output[:target_size])


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def run_to_file(command: list[str], env: dict[str, str], output_path: Path) -> float:
    start = time.perf_counter()
    with output_path.open("wb") as handle:
        subprocess.run(command, check=True, stdout=handle, env=env)
    return time.perf_counter() - start


def median(values: list[float]) -> float:
    return statistics.median(values)


sample1 = (root / "original/sample1.ref").read_bytes()
sample2 = (root / "original/sample2.ref").read_bytes()
sample3 = (root / "original/sample3.ref").read_bytes()

cases = [
    ("textual-16m", repeat_to_size(16 * 1024 * 1024, [sample1])),
    ("mixed-24m", repeat_to_size(24 * 1024 * 1024, [sample1, sample2, sample3])),
    ("random-8m", random.Random(0xB2).randbytes(8 * 1024 * 1024)),
]

case_paths: list[tuple[str, Path]] = []
for name, payload in cases:
    input_path = work / f"{name}.bin"
    input_path.write_bytes(payload)
    case_paths.append((name, input_path))

compression_runs: list[dict[str, object]] = []
decompression_runs: list[dict[str, object]] = []
summaries: list[dict[str, object]] = []

for name, input_path in case_paths:
    input_sha = sha256(input_path)
    input_bytes = input_path.stat().st_size
    baseline_bz2 = work / f"{name}.baseline.bz2"
    safe_probe_bz2 = tmp / f"{name}.safe-probe.bz2"

    run_to_file([str(baseline_cli), "-9c", str(input_path)], baseline_env, baseline_bz2)
    run_to_file([str(safe_cli), "-9c", str(input_path)], safe_env, safe_probe_bz2)

    baseline_compressed_sha = sha256(baseline_bz2)
    safe_compressed_sha = sha256(safe_probe_bz2)
    if safe_compressed_sha != baseline_compressed_sha:
        raise SystemExit(
            f"safe compression output drifted from the captured upstream baseline for {name}"
        )
    safe_probe_bz2.unlink()

    compressed_bytes = baseline_bz2.stat().st_size

    for tool_name, cli, env in (
        ("baseline", baseline_cli, baseline_env),
        ("safe", safe_cli, safe_env),
    ):
        measured: list[float] = []
        for run_index in range(1, runs + 1):
            output_path = tmp / f"{name}-{tool_name}-compress-{run_index}.bz2"
            elapsed = run_to_file([str(cli), "-9c", str(input_path)], env, output_path)
            output_sha = sha256(output_path)
            if output_sha != baseline_compressed_sha:
                raise SystemExit(
                    f"{tool_name} compression output drifted on run {run_index} for {name}"
                )
            measured.append(elapsed)
            compression_runs.append(
                {
                    "phase": "compress",
                    "case": name,
                    "tool": tool_name,
                    "run": run_index,
                    "seconds": elapsed,
                    "input_bytes": input_bytes,
                    "output_bytes": compressed_bytes,
                    "sha256": output_sha,
                }
            )
            output_path.unlink()

        if tool_name == "baseline":
            baseline_compress_times = measured
        else:
            safe_compress_times = measured

    for tool_name, cli, env in (
        ("baseline", baseline_cli, baseline_env),
        ("safe", safe_cli, safe_env),
    ):
        measured = []
        for run_index in range(1, runs + 1):
            output_path = tmp / f"{name}-{tool_name}-decompress-{run_index}.bin"
            elapsed = run_to_file([str(cli), "-dc", str(baseline_bz2)], env, output_path)
            output_sha = sha256(output_path)
            if output_sha != input_sha:
                raise SystemExit(
                    f"{tool_name} decompression output drifted on run {run_index} for {name}"
                )
            measured.append(elapsed)
            decompression_runs.append(
                {
                    "phase": "decompress",
                    "case": name,
                    "tool": tool_name,
                    "run": run_index,
                    "seconds": elapsed,
                    "input_bytes": compressed_bytes,
                    "output_bytes": input_bytes,
                    "sha256": output_sha,
                }
            )
            output_path.unlink()

        if tool_name == "baseline":
            baseline_decompress_times = measured
        else:
            safe_decompress_times = measured

    summaries.append(
        {
            "case": name,
            "input_bytes": input_bytes,
            "compressed_bytes": compressed_bytes,
            "compression": {
                "baseline_runs_s": baseline_compress_times,
                "safe_runs_s": safe_compress_times,
                "baseline_median_s": median(baseline_compress_times),
                "safe_median_s": median(safe_compress_times),
            },
            "decompression": {
                "baseline_runs_s": baseline_decompress_times,
                "safe_runs_s": safe_decompress_times,
                "baseline_median_s": median(baseline_decompress_times),
                "safe_median_s": median(safe_decompress_times),
            },
            "sha256": {
                "input": input_sha,
                "compressed": baseline_compressed_sha,
            },
        }
    )


def write_tsv(path: Path, rows: list[dict[str, object]]) -> None:
    headers = ["phase", "case", "tool", "run", "seconds", "input_bytes", "output_bytes", "sha256"]
    with path.open("w", encoding="utf-8") as handle:
        handle.write("\t".join(headers) + "\n")
        for row in rows:
            handle.write("\t".join(str(row[header]) for header in headers) + "\n")


write_tsv(out / "compression-times.tsv", compression_runs)
write_tsv(out / "decompression-times.tsv", decompression_runs)

with (out / "results.json").open("w", encoding="utf-8") as handle:
    json.dump(
        {
            "generated_at_utc": datetime.now(timezone.utc).isoformat(),
            "runs_per_case": runs,
            "baseline_cli": "target/original-baseline/bzip2",
            "safe_cli": "target/compat/bzip2",
            "cases": summaries,
        },
        handle,
        indent=2,
    )
    handle.write("\n")

summary_lines = [
    "libbz2 benchmark comparison",
    f"generated_at_utc={datetime.now(timezone.utc).isoformat()}",
    f"runs_per_case={runs}",
    "baseline_cli=target/original-baseline/bzip2",
    "safe_cli=target/compat/bzip2",
    "note=performance is informational only; this script fails only on correctness drift or missing artifacts.",
    "",
    "Compression (median seconds)",
    "case\tinput_bytes\tcompressed_bytes\tbaseline_s\tsafe_s\tsafe_over_baseline",
]

for entry in summaries:
    compression = entry["compression"]
    baseline_s = compression["baseline_median_s"]
    safe_s = compression["safe_median_s"]
    ratio = safe_s / baseline_s if baseline_s else float("inf")
    summary_lines.append(
        f"{entry['case']}\t{entry['input_bytes']}\t{entry['compressed_bytes']}\t"
        f"{baseline_s:.6f}\t{safe_s:.6f}\t{ratio:.3f}x"
    )

summary_lines.extend(
    [
        "",
        "Decompression (median seconds)",
        "case\tcompressed_bytes\toutput_bytes\tbaseline_s\tsafe_s\tsafe_over_baseline",
    ]
)

for entry in summaries:
    decompression = entry["decompression"]
    baseline_s = decompression["baseline_median_s"]
    safe_s = decompression["safe_median_s"]
    ratio = safe_s / baseline_s if baseline_s else float("inf")
    summary_lines.append(
        f"{entry['case']}\t{entry['compressed_bytes']}\t{entry['input_bytes']}\t"
        f"{baseline_s:.6f}\t{safe_s:.6f}\t{ratio:.3f}x"
    )

(out / "summary.txt").write_text("\n".join(summary_lines) + "\n", encoding="utf-8")
PY

rm -rf "$WORK" "$TMP"

if [[ "${LIBBZ2_BENCH_CAPTURE_SECURITY_LOG:-1}" != 0 ]]; then
  mkdir -p "$ROOT/target/security"
  cp "$OUT/summary.txt" "$ROOT/target/security/10-benchmark.log"
fi

cat "$OUT/summary.txt"
