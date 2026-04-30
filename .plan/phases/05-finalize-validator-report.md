# Phase Name

Finalize Validator Evidence And Report

# Implement Phase ID

`impl_finalize_validator_report`

# Preexisting Inputs

- All code/test/package fixes from earlier phases.
- `validator/` checkout at the commit recorded in phase 1. Do not pull, reclone, or otherwise update the validator suite in this phase.
- `safe/scripts/stage-validator-debs.sh` from phase 1.
- Required existing local scripts: `safe/scripts/build-safe.sh`, `safe/scripts/check-abi.sh`, `safe/scripts/link-original-tests.sh`, `safe/scripts/build-original-cli-against-safe.sh`, `safe/scripts/build-debs.sh`, `safe/scripts/check-package-layout.sh`, and `safe/scripts/run-debian-tests.sh`.
- Required validator entry points: `validator/test.sh`, `validator/tools/testcases.py`, `validator/tools/verify_proof_artifacts.py`, `validator/tools/render_site.py`, `validator/scripts/verify-site.sh`, `validator/repositories.yml`, `validator/tests/`, and `validator/unit/`.
- Current `validator/artifacts/libbz2-safe/` results, logs, casts, proofs, and local override deb copies.
- Current package artifacts under `target/package/out/`.
- Existing validator-relevant generated artifacts under `target/original-baseline/`, `target/compat/`, `target/package/unpacked/`, and `target/compat/`; preserve and reuse unless refreshed by required final commands.
- Full safe crate, tests, scripts, packaging, ABI maps, and header files: `safe/Cargo.toml`, `safe/Cargo.lock`, `safe/build.rs`, `safe/src/`, `safe/tests/`, `safe/scripts/`, `safe/debian/`, `safe/abi/`, and `safe/include/bzlib.h`.

Do not use documentation, CVE, downstream, benchmark, or aggregate-security artifacts such as `safe/docs/unsafe-audit.md`, `dependents.json`, `relevant_cves.json`, `all_cves.json`, `target/bench/`, or `target/security/` as required validator inputs for this phase unless final validation exposes a focused validator-related reason to invoke an existing safe-side harness.

# New Outputs

- Final `validator-report.md` summarizing:
  - validator commit;
  - safe source commit(s) tested;
  - package artifacts and hashes;
  - exact commands executed;
  - source and usage case counts;
  - original-mode outcome;
  - port-mode outcome;
  - failures found;
  - fixes applied;
  - regression tests added;
  - any skipped validator bugs with justification;
  - final clean run status.
- Final validator artifacts under `validator/artifacts/libbz2-safe/`.
- Final rendered site under `validator/site/libbz2-safe/`.
- Final git commit for report updates.
- If finalization discovers a safelib-caused regression that earlier phases missed, a focused regression test/fix in `safe/` and a separate git commit before the final report commit.

# File Changes

- Update `validator-report.md`.
- No new source changes are expected in this phase.
- If the final run finds a safelib-caused regression, handle it inside `impl_finalize_validator_report`; the fixed verifier `bounce_target` remains `impl_finalize_validator_report`. Apply a focused regression test and fix in `safe/`, commit it, rebuild/rerun validator evidence, and update the report, or mark the run blocked with exact evidence instead of claiming a clean validator run.
- Do not modify validator testcase files, manifests, scripts, tools, or inventory.

# Implementation Details

Port and compatibility context:

- `safe/` is the `libbz2-safe` crate with C-facing crate name `bz2`.
- `safe/src/types.rs` owns ABI-visible layouts and opaque handle state; `safe/src/ffi.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, and `safe/src/stdio.rs` own the public C API paths.
- Key ABI entry points are `BZ2_bzBuffToBuffCompress` and `BZ2_bzBuffToBuffDecompress` in `safe/src/ffi.rs:65` and `safe/src/ffi.rs:115`; `BZ2_bzCompressInit`, `BZ2_bzCompress`, and `BZ2_bzCompressEnd` in `safe/src/compress.rs:797`, `safe/src/compress.rs:881`, and `safe/src/compress.rs:943`; `BZ2_bzDecompressInit`, `BZ2_bzDecompress`, and `BZ2_bzDecompressEnd` in `safe/src/decompress.rs:1064`, `safe/src/decompress.rs:1099`, and `safe/src/decompress.rs:1169`; and wrapper APIs from `BZ2_bzReadOpen` through `BZ2_bzerror` in `safe/src/stdio.rs:182-621`.
- `safe/src/blocksort.rs`, `safe/src/huffman.rs`, `safe/src/crc.rs`, `safe/src/rand.rs`, `safe/src/alloc.rs`, and `safe/src/constants.rs` are algorithm support modules that may be relevant if final failures remain.
- `safe/build.rs`, `safe/abi/*`, and `safe/include/bzlib.h` preserve SONAME/export/header contracts; `safe/include/bzlib.h` must remain byte-identical to `original/bzlib.h`.

Required flow:

- Before editing files, record the phase base commit:

```bash
phase_base=$(git rev-parse HEAD)
```

- Refresh `target/compat/` from the committed safe source with `bash safe/scripts/build-safe.sh --release` before running final ABI, relink, or original-CLI sample checks.
- Rebuild packages from the committed safe source with `bash safe/scripts/build-debs.sh`, then run `bash safe/scripts/check-package-layout.sh`.
- Run `bash safe/scripts/stage-validator-debs.sh` to recopy the canonical three package files and regenerate the lock from the copied files.
- Confirm the copied override `.deb` files and `local-port-debs-lock.json` describe the same three canonical packages `libbz2-1.0`, `libbz2-dev`, and `bzip2`.
- Rerun original and port validator matrices with casts and proof generation.
- Render and verify the review site.
- Parse result JSONs and confirm:
  - original-mode result count is 135 with 5 source and 130 usage cases;
  - port-mode result count is 135 with 5 source and 130 usage cases;
  - all port cases pass, unless a validator-bug skip is documented;
  - every documented skip has original-mode evidence, an exact testcase ID, and one marker line `Validator-bug skip: <testcase_id>`.
- Update `validator-report.md` to be self-contained and include the exact line `Phase impl_finalize_validator_report base commit: <phase_base>`.
- The report must not require a reader to infer commands or failure state from raw artifacts.
- Commit final report updates before yielding.
- If this phase has no source changes, still leave an explicit linear history marker by committing the final report update, or use a narrowly named empty commit only if the report already contains all required phase evidence and base-line content.

# Verification Phases

## `check_final_validator_clean_run`

- Type: `check`
- Fixed `bounce_target`: `impl_finalize_validator_report`
- Purpose: independently run final build/test/package/validator commands and confirm the final validator evidence is clean, except for any explicitly documented validator-bug skips.
- Commands:
  - `cargo test --manifest-path safe/Cargo.toml --release`
  - `bash safe/scripts/build-safe.sh --release`
  - `bash safe/scripts/check-abi.sh --strict`
  - `bash safe/scripts/link-original-tests.sh --all`
  - `bash safe/scripts/build-original-cli-against-safe.sh --run-samples`
  - `bash safe/scripts/build-debs.sh`
  - `bash safe/scripts/check-package-layout.sh`
  - `bash safe/scripts/run-debian-tests.sh --tests link-with-shared bigfile bzexe-test compare compress grep`
  - `bash safe/scripts/stage-validator-debs.sh`
  - `find validator/artifacts/libbz2-safe/debs/local/libbz2 -maxdepth 1 -type f -name '*.deb' | sort`
  - `test "$(find validator/artifacts/libbz2-safe/debs/local/libbz2 -maxdepth 1 -type f -name '*.deb' | wc -l)" -eq 3`
  - `test -f validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json`
  - A short Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and the copied override `.deb` files.
  - `python3 validator/tools/testcases.py --config validator/repositories.yml --tests-root validator/tests --check --library libbz2 --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - `cd validator && python3 -m unittest discover -s unit -v`
  - `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode original --library libbz2 --record-casts`
  - `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-original-validation-proof.json --mode original --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
  - `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - `cd validator && python3 tools/render_site.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-path artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json --proof-path artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json --output-root site/libbz2-safe`
  - `cd validator && bash scripts/verify-site.sh --config repositories.yml --tests-root tests --artifacts-root artifacts/libbz2-safe --proof-path artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json --proof-path artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json --site-root site/libbz2-safe --library libbz2`
  - Run this final clean-result assertion from the parent repository root:

```bash
python3 - <<'PY'
import json
import re
import sys
from pathlib import Path

report_path = Path("validator-report.md")
report = report_path.read_text()
documented_skips = set(re.findall(r"^Validator-bug skip:\s*(\S+)\s*$", report, re.MULTILINE))
expected_counts = {"source": 5, "usage": 130}
mode_roots = {
    "original": Path("validator/artifacts/libbz2-safe/results/libbz2"),
    "port": Path("validator/artifacts/libbz2-safe/port/results/libbz2"),
}
all_seen_ids = set()
nonpassing_ids = set()
errors = []

for mode, root in mode_roots.items():
    result_paths = sorted(path for path in root.glob("*.json") if path.name != "summary.json")
    if len(result_paths) != 135:
        errors.append(f"{mode}: expected 135 testcase result files, found {len(result_paths)}")
    counts = {"source": 0, "usage": 0}
    failures = []
    for path in result_paths:
        payload = json.loads(path.read_text())
        testcase_id = payload.get("testcase_id") or path.stem
        all_seen_ids.add(testcase_id)
        kind = payload.get("kind")
        if kind in counts:
            counts[kind] += 1
        else:
            errors.append(f"{mode}: {testcase_id} has unexpected kind {kind!r}")
        if payload.get("status") != "passed":
            nonpassing_ids.add(testcase_id)
            failures.append((testcase_id, payload.get("status"), payload.get("title"), str(path)))
    if counts != expected_counts:
        errors.append(f"{mode}: expected counts {expected_counts}, found {counts}")
    unexplained = [failure for failure in failures if failure[0] not in documented_skips]
    if unexplained:
        for testcase_id, status, title, path in unexplained:
            errors.append(f"{mode}: unexplained non-passing result {testcase_id} status={status!r} title={title!r} path={path}")

unknown_skips = documented_skips - all_seen_ids
if unknown_skips:
    errors.append(f"validator-report.md documents skip IDs with no matching result JSON: {sorted(unknown_skips)}")

unused_skips = documented_skips - nonpassing_ids
if unused_skips:
    errors.append(f"validator-report.md documents validator-bug skips that are not attached to a non-passing result: {sorted(unused_skips)}")

if errors:
    for error in errors:
        print(error, file=sys.stderr)
    sys.exit(1)
PY
```

## `check_final_senior_review`

- Type: `check`
- Fixed `bounce_target`: `impl_finalize_validator_report`
- Purpose: review final artifact flow, report completeness, git commit shape, and adherence to the no-validator-modification rule.
- Commands:
  - `git status --short`
  - `git log --oneline --decorate -8`
  - `git ls-files --stage validator`
  - `test -d validator/.git`
  - `git -C validator rev-parse HEAD`
  - `base=$(awk '/^Phase impl_finalize_validator_report base commit: / {print $NF}' validator-report.md | tail -n1); test -n "$base"; git log --oneline "$base"..HEAD; git diff --stat "$base"..HEAD; git diff --name-only "$base"..HEAD`
  - `git -C validator status --short`
  - `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`
  - `sed -n '1,420p' validator-report.md`

# Success Criteria

- Final validator run is clean or has only justified validator-bug skips.
- `validator-report.md` names the final validator commit and safe source commit tested.
- The report lists exact commands run, package artifacts/hashes, case counts, failures found, regression tests added, source/package fixes applied, and final outcome.
- Original-mode validator results are recorded as the environment control.
- Port-mode validator results have zero unexplained failures.
- Every safelib-caused validator failure has a minimal regression test under `safe/tests/` and a fix in `safe/`.
- `cargo test --manifest-path safe/Cargo.toml --release`, ABI checks, relink checks, package layout checks, and final validator proof generation pass.
- No upstream validator testcase, manifest, or tool file was modified to make checks pass.
- `git ls-files --stage validator` prints nothing, so the nested validator checkout is not committed to the parent repo.
- Parent git history contains commits for each implementation phase.

# Git Commit Requirement

The implementer must commit work to git before yielding. Commit any unexpected source/test/package fix separately before rebuilding final evidence, then commit the final `validator-report.md` update. Do not commit unrelated dirty files or validator-suite modifications.
