# Phase Name

Fix CLI, Packaging, And Usage Validator Failures

# Implement Phase ID

`impl_fix_validator_cli_usage_failures`

# Preexisting Inputs

- Validator usage-case failures from earlier phases, including result JSON/logs under `validator/artifacts/libbz2-safe/port/results/libbz2/` and `validator/artifacts/libbz2-safe/port/logs/libbz2/`.
- Original-mode result JSON/logs under `validator/artifacts/libbz2-safe/results/libbz2/` and `validator/artifacts/libbz2-safe/logs/libbz2/` for environment-control comparison.
- `validator-report.md` from earlier phases.
- `validator/` checkout at the commit recorded in phase 1. Do not pull, reclone, or otherwise update the validator suite in this phase.
- `safe/scripts/stage-validator-debs.sh` from phase 1.
- Required build and ABI scripts: `safe/scripts/build-safe.sh` and `safe/scripts/check-abi.sh`.
- Relink harnesses in `safe/scripts/build-original-cli-against-safe.sh` and `safe/scripts/link-original-tests.sh`.
- Package build, layout, and autopkgtest harnesses in `safe/scripts/build-debs.sh`, `safe/scripts/check-package-layout.sh`, and `safe/scripts/run-debian-tests.sh`.
- Required validator entry points: `validator/test.sh`, `validator/tools/verify_proof_artifacts.py`, `validator/repositories.yml`, and `validator/tests/`.
- Packaging manifests in `safe/debian/`, including `control`, `rules`, `*.install`, `*.links`, `*.manpages`, and `tests/control`.
- Original CLI and scripts in `original/bzip2.c`, `original/bzip2recover.c`, `original/bzdiff`, `original/bzgrep`, `original/bzmore`, and related manpages.
- Current package outputs under `target/package/out/` and existing staged compatibility artifacts under `target/compat/`; preserve and reuse unless source/package changes make them stale.
- Existing original baseline artifacts under `target/original-baseline/` used by relink scripts.
- Full safe source and tests, especially `safe/src/ffi.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/stdio.rs`, `safe/src/types.rs`, `safe/tests/link_contract.rs`, `safe/tests/original_port.rs`, `safe/tests/compression_port.rs`, `safe/tests/decompress_port.rs`, and `safe/tests/golden_streams.rs`.

Do not use documentation, CVE, downstream, benchmark, or aggregate-security artifacts such as `safe/docs/unsafe-audit.md`, `dependents.json`, `relevant_cves.json`, `all_cves.json`, `target/bench/`, or `target/security/` as required validator inputs for this phase.

# New Outputs

- Minimal regression tests for each CLI/usage failure.
- Fixes in `safe/src/*`, `safe/debian/*`, or `safe/scripts/*`, depending on the isolated cause.
- Rebuilt packages and refreshed validator artifacts.
- Updated `validator-report.md`.
- Git commit(s) for code/tests and report evidence.

# File Changes

- Extend `safe/tests/link_contract.rs` for failures that reproduce through relinked original objects or original CLI source.
- Extend `safe/tests/original_port.rs`, `safe/tests/compression_port.rs`, or `safe/tests/decompress_port.rs` when the underlying issue is library API behavior.
- Extend `safe/tests/golden_streams.rs` for validator sample fixture mismatches.
- Modify `safe/src/ffi.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/stdio.rs`, `safe/src/types.rs`, or algorithm support modules only when the CLI/usage failure is proven to come from library behavior.
- Modify `safe/debian/*.install`, `safe/debian/*.links`, `safe/debian/*.manpages`, `safe/debian/rules`, or `safe/scripts/build-original-cli-against-safe.sh` only when the failing validator case proves a package or CLI install problem rather than a library semantic problem.
- Avoid editing upstream copied files in `original/`.
- Do not edit validator testcase files, manifests, or tools.

# Implementation Details

Port and ABI context:

- `safe/` is the Rust crate `libbz2-safe` with C-facing crate name `bz2`; `safe/include/bzlib.h` must remain byte-identical to `original/bzlib.h`.
- ABI-visible structs and handles are in `safe/src/types.rs`; public buffer APIs are `BZ2_bzBuffToBuffCompress` and `BZ2_bzBuffToBuffDecompress` in `safe/src/ffi.rs:65` and `safe/src/ffi.rs:115`.
- Stream APIs are `BZ2_bzCompressInit` / `BZ2_bzCompress` / `BZ2_bzCompressEnd` in `safe/src/compress.rs:797`, `safe/src/compress.rs:881`, and `safe/src/compress.rs:943`; `BZ2_bzDecompressInit` / `BZ2_bzDecompress` / `BZ2_bzDecompressEnd` in `safe/src/decompress.rs:1064`, `safe/src/decompress.rs:1099`, and `safe/src/decompress.rs:1169`.
- CLI and usage cases heavily exercise wrapper APIs from `BZ2_bzReadOpen` through `BZ2_bzerror` in `safe/src/stdio.rs:182-621`, plus package layout and the relinked original CLI.

Required flow:

- Before editing files, record the phase base commit:

```bash
phase_base=$(git rev-parse HEAD)
```

- Classify usage failures by command family:
  - `bzip2` / `bunzip2` / `bzcat` roundtrip, streaming, suffix, overwrite, stdin/stdout, level, corruption, concatenation, and verbose-status failures.
  - `bzcmp` / `bzdiff` script behavior.
  - `bzgrep` regex, count, filename, and pipe behavior.
  - `bzmore` / `bzless` pager behavior.
  - `bzip2recover` recovery and listing behavior.
- For each family, inspect the validator testcase script and isolate whether the safe shared library, the relinked original CLI, a shell script, or package layout is responsible.
- Compare original-mode behavior before changing `safe/`. If original mode fails the same way, document validator/environment evidence instead of changing the port.
- For core library drift, add a Rust regression test that exercises the public library API.
- For CLI-only drift, add or extend a relink/package test so the same installed binary or script path is covered before changing packaging.
- Use `safe/scripts/build-original-cli-against-safe.sh --run-samples` and `safe/scripts/link-original-tests.sh --all` to isolate relinked original CLI behavior, but first refresh `target/compat/` with `bash safe/scripts/build-safe.sh --release`.
- Rebuild `target/compat/` with `bash safe/scripts/build-safe.sh --release` after every committed source or packaging fix and before any ABI, relink, or original-CLI sample check.
- Rebuild packages, run `bash safe/scripts/check-package-layout.sh`, and run `bash safe/scripts/stage-validator-debs.sh` to recopy only canonical `.deb` files into the validator override leaf and regenerate the local lock.
- Confirm the copied override `.deb` files and `local-port-debs-lock.json` describe the same three canonical packages `libbz2-1.0`, `libbz2-dev`, and `bzip2`.
- Rerun the full libbz2 port validator matrix after each family-level fix. Do not mark a family fixed based only on a hand reproduction unless the validator result JSON confirms it.
- Include the exact line `Phase impl_fix_validator_cli_usage_failures base commit: <phase_base>` in `validator-report.md`.
- Commit all source/package/test fixes and the updated `validator-report.md` before yielding.
- If this phase has no applicable usage failures or source/package changes, still leave an explicit linear history marker before yielding by committing a report-only no-op note, or use a narrowly named empty commit only if the report already contains required evidence and base-line content.

# Verification Phases

## `check_cli_usage_regressions_software_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_fix_validator_cli_usage_failures`
- Purpose: verify fixed usage cases across relinked original CLI binaries, Debian package layout, and validator port results.
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
  - A short Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and the copied override `.deb` files.
  - `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
  - `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - A short Python summary over `validator/artifacts/libbz2-safe/port/results/libbz2/*.json` to list remaining failed testcase IDs.

## `check_cli_usage_senior_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_fix_validator_cli_usage_failures`
- Purpose: review whether failures were fixed at the correct layer: Rust library, original CLI relink, Debian packaging, or copied upstream scripts.
- Commands:
  - `base=$(awk '/^Phase impl_fix_validator_cli_usage_failures base commit: / {print $NF}' validator-report.md | tail -n1); test -n "$base"; git log --oneline "$base"..HEAD; git diff "$base"..HEAD -- safe/src safe/tests safe/debian safe/scripts validator-report.md`
  - `find target/package/out -maxdepth 1 -type f -name '*.deb' | sort`
  - `git -C validator status --short`
  - `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`
  - Inspect failing/now-passing validator logs named in `validator-report.md`.

# Success Criteria

- All fixed usage failures have regression coverage in `safe/tests/` or an existing script harness invoked by `safe/scripts/run-full-suite.sh`.
- `cargo test --manifest-path safe/Cargo.toml --release`, relink checks, original-CLI sample checks, Debian package checks, and selected Debian tests pass.
- The port validator summary shows fewer failures, and no original-mode validator regressions are introduced.
- Package staging still contains exactly the canonical packages `libbz2-1.0`, `libbz2-dev`, and `bzip2`, and the lock matches the copied files.
- `validator-report.md` records which CLI/package family was fixed, which files changed, regression coverage, commands, result JSON/log evidence, and final status.
- Validator upstream files remain unmodified.

# Git Commit Requirement

The implementer must commit work to git before yielding. If source, package, or script files change, commit the fix before rebuilding packages and running the validator, then commit the updated `validator-report.md` evidence. Do not commit unrelated dirty files.
