# Phase Name

Fix Source And C API Validator Failures

# Implement Phase ID

`impl_fix_validator_source_api_failures`

# Preexisting Inputs

- Initial validator artifacts and `validator-report.md` from `impl_validator_bootstrap_and_initial_run`.
- `validator/` checkout at the commit recorded in phase 1. Do not pull, reclone, or otherwise update the validator suite in this phase.
- `safe/scripts/stage-validator-debs.sh` from phase 1.
- Required existing local scripts: `safe/scripts/build-safe.sh`, `safe/scripts/check-abi.sh`, `safe/scripts/build-debs.sh`, and `safe/scripts/check-package-layout.sh`.
- Required validator entry points: `validator/test.sh`, `validator/tools/verify_proof_artifacts.py`, `validator/repositories.yml`, and `validator/tests/`.
- Failing source-case result JSON/logs under `validator/artifacts/libbz2-safe/port/results/libbz2/` and `validator/artifacts/libbz2-safe/port/logs/libbz2/`.
- Original-mode result JSON/logs under `validator/artifacts/libbz2-safe/results/libbz2/` and `validator/artifacts/libbz2-safe/logs/libbz2/` for environment-control comparison.
- Validator source-case scripts under `validator/tests/libbz2/tests/cases/source/`.
- `safe/src/ffi.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/stdio.rs`, `safe/src/types.rs`, and algorithm support modules `safe/src/blocksort.rs`, `safe/src/huffman.rs`, `safe/src/crc.rs`, `safe/src/rand.rs`, `safe/src/alloc.rs`, and `safe/src/constants.rs`.
- Relevant tests under `safe/tests/`, including `original_port.rs`, `golden_streams.rs`, `malformed_inputs.rs`, and `security_regressions.rs`.
- ABI and header contracts in `safe/build.rs`, `safe/abi/*`, and `safe/include/bzlib.h`.
- `original/` sample fixtures and C tests for parity comparison.
- Existing validator-relevant generated artifacts under `target/original-baseline/`, `target/compat/`, `target/package/out/`, and validator artifacts from phase 1; preserve and reuse unless a committed source/package change makes them stale.

Do not use documentation, CVE, downstream, benchmark, or aggregate-security artifacts such as `safe/docs/unsafe-audit.md`, `dependents.json`, `relevant_cves.json`, `all_cves.json`, `target/bench/`, or `target/security/` as required validator inputs for this phase.

# New Outputs

- Minimal regression tests reproducing each source/API validator failure.
- Fixes in `safe/src/*`, or only if the isolated issue is package/header/ABI related, in `safe/debian/*`, `safe/include/bzlib.h`, `safe/abi/*`, or `safe/scripts/*`.
- Updated local `.deb` packages and validator artifacts when source/package changes are made.
- Updated `validator-report.md` with source/API failure classification, regression tests, fixes, commands, and result status.
- One or more git commits. If source changes are made, commit the test/code fix before rebuilding packages for validator.

# File Changes

- Prefer a new integration test file `safe/tests/validator_regressions.rs` for validator-specific reproductions that do not naturally fit existing tests.
- Alternatively extend:
  - `safe/tests/original_port.rs` for public C API contract failures.
  - `safe/tests/golden_streams.rs` for fixture bit-for-bit failures.
  - `safe/tests/malformed_inputs.rs` for malformed stream and return-code failures.
  - `safe/tests/security_regressions.rs` for safety, termination, and integer-overflow style failures.
- Possible source changes:
  - `safe/src/ffi.rs` for `BZ2_bzBuffToBuffCompress` and `BZ2_bzBuffToBuffDecompress`, buffer-to-buffer wrapper return codes, length accounting, parameter validation, and error propagation.
  - `safe/src/compress.rs` for compressor state machine, block flushing, total counters, and stream end behavior.
  - `safe/src/decompress.rs` for parser/output state, concatenation, malformed data, unused input, CRC, and EOF behavior.
  - `safe/src/stdio.rs` for `BZ2_bzRead*`, `BZ2_bzWrite*`, `bzopen`, `bzdopen`, `bzread`, `bzwrite`, and `bzerror` semantics.
  - `safe/src/types.rs` for ABI-visible struct layout or opaque handle state only when the validator failure proves a layout/state defect; preserve ABI compatibility.
  - `safe/src/blocksort.rs`, `safe/src/huffman.rs`, `safe/src/crc.rs`, `safe/src/rand.rs`, `safe/src/alloc.rs`, and `safe/src/constants.rs` only when a concrete source/API failure traces to algorithm, CRC, allocation, randomization, or constant behavior.
- Do not edit validator scripts or manifests.
- Avoid header or ABI map changes unless upstream compatibility demands them; `safe/include/bzlib.h` must remain byte-identical to `original/bzlib.h`.

# Implementation Details

Port architecture and entry points to keep in view:

- `safe/` is a single Rust crate named `libbz2-safe`, with C-facing crate name `bz2` and no direct Rust dependencies.
- `safe/src/lib.rs` preserves the upstream module split.
- ABI-visible structs and handles are in `safe/src/types.rs`.
- Low-level compression is in `safe/src/compress.rs`; decompression is in `safe/src/decompress.rs`; buffer wrappers, version, and assertion ABI functions are in `safe/src/ffi.rs`; `FILE*`, fd, and `BZFILE*` wrappers are in `safe/src/stdio.rs`.
- Key ABI entry points are `BZ2_bzBuffToBuffCompress` and `BZ2_bzBuffToBuffDecompress` in `safe/src/ffi.rs:65` and `safe/src/ffi.rs:115`; `BZ2_bzCompressInit`, `BZ2_bzCompress`, and `BZ2_bzCompressEnd` in `safe/src/compress.rs:797`, `safe/src/compress.rs:881`, and `safe/src/compress.rs:943`; `BZ2_bzDecompressInit`, `BZ2_bzDecompress`, and `BZ2_bzDecompressEnd` in `safe/src/decompress.rs:1064`, `safe/src/decompress.rs:1099`, and `safe/src/decompress.rs:1169`; and wrapper APIs from `BZ2_bzReadOpen` through `BZ2_bzerror` in `safe/src/stdio.rs:182-621`.

Required flow:

- Before editing files, record the phase base commit:

```bash
phase_base=$(git rev-parse HEAD)
```

- Start from concrete validator failures, not speculation. For each failed source/API testcase:
  - Read its result JSON and log.
  - Read the validator script under `validator/tests/libbz2/tests/cases/source/<id>.sh`.
  - Compare with matching original-mode result JSON/log before changing `safe/`.
  - Reproduce the behavior in a minimal Rust integration test that calls the same public API or wrapper path.
  - Confirm the regression test fails before the fix when the prior code can still be executed.
  - Fix the underlying `safe/` implementation, preserving upstream libbz2 ABI and return-code semantics.

Expected source-case mappings:

- `c-api-buffer-roundtrip` failures usually map to `BZ2_bzBuffToBuffCompress` / `BZ2_bzBuffToBuffDecompress` in `safe/src/ffi.rs:65` and `safe/src/ffi.rs:115`, plus compressor/decompressor state behavior in `safe/src/compress.rs` and `safe/src/decompress.rs`.
- `corrupted-stream-rejection` failures usually map to malformed-input paths in `safe/src/decompress.rs:1099` and `safe/src/ffi.rs:115`, with possible CRC/EOF support in `safe/src/crc.rs` and stream state in `safe/src/types.rs`.
- `debian-sample-parity` failures usually map to golden stream decompression, package fixture availability, or bit-for-bit algorithm support in `safe/src/blocksort.rs`, `safe/src/huffman.rs`, `safe/src/crc.rs`, and `safe/src/constants.rs`.
- `stream-concatenation` failures usually map to CLI or decompressor handling of trailing/unused input; add a direct library test to isolate `safe/src/decompress.rs` and `safe/src/stdio.rs` behavior before changing CLI/package files.
- `cli-compress-decompress` failures may be core compression/decompression, package install layout, or the relinked original CLI; isolate with `safe/scripts/build-original-cli-against-safe.sh --run-samples`.

Validator and artifact rules:

- Do not edit validator scripts, manifests, tools, or tests to make failures pass.
- If the original-mode validator case also fails in the same way, investigate validator/environment first and document it rather than changing `safe/` blindly.
- Later phases must consume the validator checkout commit recorded by phase 1 and must not reclone, pull, or otherwise change the validator suite version.
- If `safe/scripts/stage-validator-debs.sh` is run after source, packaging, or script changes, first run:

```bash
bash safe/scripts/build-debs.sh
bash safe/scripts/check-package-layout.sh
```

- Confirm copied override `.deb` files and `local-port-debs-lock.json` describe the same three canonical packages `libbz2-1.0`, `libbz2-dev`, and `bzip2`.
- If `safe/scripts/check-abi.sh --strict` is run to verify current code, first run:

```bash
bash safe/scripts/build-safe.sh --release
```

Validation and commits:

- After source/test fixes, run focused Cargo tests, commit the fix, run `bash safe/scripts/build-safe.sh --release`, run `bash safe/scripts/check-abi.sh --strict`, rebuild packages, run `bash safe/scripts/check-package-layout.sh`, run `bash safe/scripts/stage-validator-debs.sh`, rerun the validator port matrix, and update `validator-report.md`.
- Include the exact line `Phase impl_fix_validator_source_api_failures base commit: <phase_base>` in `validator-report.md`.
- Commit all source/test fixes and the updated `validator-report.md` before yielding.
- If this phase has no applicable source/API failures or source changes, still leave an explicit linear history marker before yielding by committing a report-only no-op note, or use a narrowly named empty commit with `git commit --allow-empty` only if `validator-report.md` already contains all required phase evidence and base-line content.

# Verification Phases

## `check_source_api_regressions_software_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_fix_validator_source_api_failures`
- Purpose: confirm that each source/API validator failure has a minimal Rust regression test and that the relevant local validator cases now pass.
- Commands:
  - `cargo test --manifest-path safe/Cargo.toml --release --test validator_regressions` if `safe/tests/validator_regressions.rs` exists.
  - `cargo test --manifest-path safe/Cargo.toml --release --test original_port`
  - `cargo test --manifest-path safe/Cargo.toml --release --test golden_streams`
  - `cargo test --manifest-path safe/Cargo.toml --release --test malformed_inputs`
  - `cargo test --manifest-path safe/Cargo.toml --release --test security_regressions`
  - `bash safe/scripts/build-safe.sh --release`
  - `bash safe/scripts/check-abi.sh --strict`
  - `bash safe/scripts/build-debs.sh`
  - `bash safe/scripts/check-package-layout.sh`
  - `bash safe/scripts/stage-validator-debs.sh`
  - A short Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and the copied override `.deb` files.
  - `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
  - `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - A short Python summary over `validator/artifacts/libbz2-safe/port/results/libbz2/*.json` to list remaining failed testcase IDs.

## `check_source_api_senior_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_fix_validator_source_api_failures`
- Purpose: review the code changes for compatibility with upstream libbz2 semantics, minimality of regression tests, and absence of validator-suite modifications.
- Commands:
  - `base=$(awk '/^Phase impl_fix_validator_source_api_failures base commit: / {print $NF}' validator-report.md | tail -n1); test -n "$base"; git log --oneline "$base"..HEAD; git diff "$base"..HEAD -- safe/src safe/tests safe/debian safe/include safe/abi safe/scripts validator-report.md`
  - `git -C validator status --short`
  - `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`
  - Inspect the specific failing and now-passing result JSON/log files named in `validator-report.md`.

# Success Criteria

- Each fixed source/API failure has a named regression test in `safe/tests/`.
- `cargo test --manifest-path safe/Cargo.toml --release` passes.
- `bash safe/scripts/build-safe.sh --release` refreshes `target/compat/`, then `bash safe/scripts/check-abi.sh --strict` passes unless the failure intentionally required an ABI change. Any ABI change must be justified and still match upstream `libbz2`.
- Package rebuild, package layout check, staging, and lock/package consistency checks pass before port-mode validator execution.
- Port-mode validator no longer reports the fixed source/API failures.
- `validator-report.md` links each source/API failure to the regression test, fix commit, result JSON/log evidence, and final status.
- Validator upstream files remain unmodified.

# Git Commit Requirement

The implementer must commit work to git before yielding. If source or tests change, commit the test/code fix before building local `.deb` packages for validator execution, then commit the updated `validator-report.md` evidence. Do not commit unrelated dirty files.
