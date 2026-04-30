# Phase Name

Fix Remaining Safety Or Compatibility Failures

# Implement Phase ID

`impl_fix_validator_remaining_failures`

# Preexisting Inputs

- Updated validator artifacts and `validator-report.md` from phases 1-3.
- `validator/` checkout at the commit recorded in phase 1. Do not pull, reclone, or otherwise update the validator suite in this phase.
- `safe/scripts/stage-validator-debs.sh` from phase 1.
- Required existing local scripts: `safe/scripts/build-safe.sh`, `safe/scripts/check-abi.sh`, `safe/scripts/run-full-suite.sh`, `safe/scripts/build-debs.sh`, and `safe/scripts/check-package-layout.sh`.
- Required validator entry points: `validator/test.sh`, `validator/tools/verify_proof_artifacts.py`, `validator/repositories.yml`, and `validator/tests/`.
- Any still-failing validator result JSON/logs under `validator/artifacts/libbz2-safe/port/results/libbz2/` and `validator/artifacts/libbz2-safe/port/logs/libbz2/`.
- Original-mode result JSON/logs under `validator/artifacts/libbz2-safe/results/libbz2/` and `validator/artifacts/libbz2-safe/logs/libbz2/`.
- Full safe source, tests, scripts, and packaging: `safe/src/`, `safe/tests/`, `safe/scripts/`, `safe/debian/`, `safe/abi/`, and `safe/include/bzlib.h`.
- Existing validator-relevant generated artifacts under `target/original-baseline/`, `target/compat/`, `target/package/out/`, `target/package/unpacked/`, and current validator artifacts; preserve and reuse unless source/package changes make them stale.

Do not use documentation, CVE, downstream, benchmark, or aggregate-security artifacts such as `safe/docs/unsafe-audit.md`, `dependents.json`, `relevant_cves.json`, `all_cves.json`, `target/bench/`, or `target/security/` as required validator inputs unless a remaining validator failure is traced to a package, CLI, or safety regression that specifically requires one existing safe-side test or script as a focused regression harness.

# New Outputs

- Final regression tests and code/package fixes for all remaining safelib-caused failures.
- A documented validator-bug finding for any failure not caused by `libbz2-safe`, including testcase ID, original-mode evidence, port-mode evidence, exact reason it is a validator issue, and why it is skipped.
- Updated validator artifacts and `validator-report.md`.
- Git commit(s).

# File Changes

- Add or extend `safe/tests/validator_regressions.rs` for cross-cutting failures.
- Modify `safe/src/*`, `safe/debian/*`, or `safe/scripts/*` only according to the isolated cause.
- Include `safe/src/types.rs` and algorithm support modules `safe/src/blocksort.rs`, `safe/src/huffman.rs`, `safe/src/crc.rs`, `safe/src/rand.rs`, `safe/src/alloc.rs`, and `safe/src/constants.rs` in the investigation when failures point to ABI-visible state, bitstream encoding/decoding, CRC, allocation, or constants.
- Update `validator-report.md`.
- Do not modify validator testcase files to skip failures. If a skip is required, record it only in `validator-report.md` and keep the full validator result available as evidence.

# Implementation Details

Port architecture to preserve:

- `safe/` is a single Rust crate named `libbz2-safe`, with no direct Rust dependencies and C-facing crate name `bz2`.
- ABI-visible structs and handles are in `safe/src/types.rs`.
- Compression, decompression, buffer wrappers, and stdio wrappers live in `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/ffi.rs`, and `safe/src/stdio.rs`.
- Key ABI entry points are `BZ2_bzBuffToBuffCompress` and `BZ2_bzBuffToBuffDecompress` in `safe/src/ffi.rs:65` and `safe/src/ffi.rs:115`; `BZ2_bzCompressInit`, `BZ2_bzCompress`, and `BZ2_bzCompressEnd` in `safe/src/compress.rs:797`, `safe/src/compress.rs:881`, and `safe/src/compress.rs:943`; `BZ2_bzDecompressInit`, `BZ2_bzDecompress`, and `BZ2_bzDecompressEnd` in `safe/src/decompress.rs:1064`, `safe/src/decompress.rs:1099`, and `safe/src/decompress.rs:1169`; and wrapper APIs from `BZ2_bzReadOpen` through `BZ2_bzerror` in `safe/src/stdio.rs:182-621`.
- `safe/build.rs`, `safe/abi/*`, and `safe/include/bzlib.h` define ABI/export/header compatibility; `safe/include/bzlib.h` must remain byte-identical to `original/bzlib.h`.

Required flow:

- Before editing files, record the phase base commit:

```bash
phase_base=$(git rev-parse HEAD)
```

- Re-extract all remaining failures from `validator/artifacts/libbz2-safe/port/results/libbz2/*.json`.
- For each remaining failure:
  - Confirm whether the matching original-mode case passes.
  - Inspect the port and original result JSON/logs and the validator testcase script.
  - Add a minimal local regression test if the failure reflects `libbz2-safe`.
  - Fix the root cause in `safe/`.
  - Rebuild `target/compat/` with `bash safe/scripts/build-safe.sh --release`.
  - Run `bash safe/scripts/check-abi.sh --strict`.
  - Rebuild packages with `bash safe/scripts/build-debs.sh`.
  - Run `bash safe/scripts/check-package-layout.sh`.
  - Run `bash safe/scripts/stage-validator-debs.sh`.
  - Confirm the copied override `.deb` files and `local-port-debs-lock.json` describe the same three canonical packages `libbz2-1.0`, `libbz2-dev`, and `bzip2`.
  - Rerun validator evidence.

Validator-bug criteria:

- The same testcase fails in original mode on a clean validator checkout, or the testcase asserts behavior that contradicts upstream `libbz2` / Ubuntu 24.04 package behavior.
- The failure can be reproduced without the safe override packages.
- The report includes exact evidence, a narrow skip rationale, and exactly one marker line `Validator-bug skip: <testcase_id>` for each accepted skip.
- Do not use `Validator-bug skip:` for safelib-caused failures, environmental blocks, or failures that still need implementation work.

Environmental limitation criteria:

- Missing Docker/build tooling, network outage, or host capability prevents execution.
- This is not a port pass. Document as blocked and do not claim a clean validator run.

Reporting and commits:

- Include the exact line `Phase impl_fix_validator_remaining_failures base commit: <phase_base>` in `validator-report.md`.
- Commit all final source/package/test fixes and the updated report before yielding.
- If this phase has no applicable remaining failures or source changes, still leave an explicit linear history marker before yielding by committing a report-only no-op note, or use a narrowly named empty commit only if the report already contains all required phase evidence and base-line content.

# Verification Phases

## `check_remaining_validator_failures_software_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_fix_validator_remaining_failures`
- Purpose: run a catch-all validator pass and confirm all remaining non-skipped port failures have regression tests and fixes.
- Commands:
  - `cargo test --manifest-path safe/Cargo.toml --release`
  - `bash safe/scripts/build-safe.sh --release`
  - `bash safe/scripts/check-abi.sh --strict`
  - `bash safe/scripts/run-full-suite.sh` when host prerequisites are available; otherwise run at least the component commands named in the final report.
  - `bash safe/scripts/build-debs.sh`
  - `bash safe/scripts/check-package-layout.sh`
  - `bash safe/scripts/stage-validator-debs.sh`
  - A short Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and the copied override `.deb` files.
  - `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode original --library libbz2 --record-casts`
  - `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-original-validation-proof.json --mode original --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
  - `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - A short Python summary over `validator/artifacts/libbz2-safe/{results,port/results}/libbz2/*.json` to list remaining failed testcase IDs.

## `check_remaining_validator_failures_senior_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_fix_validator_remaining_failures`
- Purpose: decide whether any remaining item is truly a validator bug or environmental limitation and verify that any skip is narrow, documented, and not implemented by modifying validator tests.
- Commands:
  - `git -C validator status --short`
  - `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`
  - `base=$(awk '/^Phase impl_fix_validator_remaining_failures base commit: / {print $NF}' validator-report.md | tail -n1); test -n "$base"; git log --oneline "$base"..HEAD; git diff "$base"..HEAD -- safe validator-report.md`
  - `sed -n '1,320p' validator-report.md`
  - Inspect all remaining failed result JSON/log paths named in the report.

# Success Criteria

- No unexplained port validator failures remain.
- Every safelib-caused failure has a regression test and fix.
- Every validator-bug skip is documented with original-mode evidence, exact testcase ID, and no validator source modification.
- `validator-report.md` cleanly separates fixed, skipped, and blocked items.
- Package staging still contains only the canonical local override packages and the lock matches copied file hashes and sizes.
- Validator upstream files remain unmodified.

# Git Commit Requirement

The implementer must commit work to git before yielding. Commit any source/package/test fix before rebuilding packages and running validator evidence, then commit the updated `validator-report.md`. Do not commit unrelated dirty files or validator-suite modifications.
