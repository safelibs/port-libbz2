# Validator Bootstrap And Initial Run Report

Phase impl_validator_bootstrap_and_initial_run base commit: fab3f69085c81ed94cf3f7bc19cfdacbe03609b6

## Revisions

- Validator commit: `5d908be26e33f071e119ffe1a52e3149f1e5ec4e`.
- Safe source commit used to build staged port packages: `00f86c80804824acac7b35b28a3644aaa0346c8e`.
- Parent `.git/info/exclude` locally contains `/validator/`; the nested checkout was not staged as a parent gitlink.
- Local override provenance in the port deb lock uses `repository: local/libbz2-port`, `release_tag: local-libbz2-port`, and `tag_ref: refs/tags/local-libbz2-port`.

## Package Lock

Lock file: `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json`

Staged override root: `validator/artifacts/libbz2-safe/debs/local/libbz2/`

Canonical packages staged, in lock order:

| Package | Filename | Arch |
| --- | --- | --- |
| `libbz2-1.0` | `libbz2-1.0_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |
| `libbz2-dev` | `libbz2-dev_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |
| `bzip2` | `bzip2_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |

The current `local-port-debs-lock.json` is the authoritative source for staged package sizes and SHA256 values because verifier phases rebuild and restage these `.deb` files. `bzip2-doc_*.deb` was not copied. `unported_original_packages` is `[]`.

## Commands Executed

- `git rev-parse HEAD`
- `grep -qxF '/validator/' .git/info/exclude || printf '/validator/\n' >> .git/info/exclude`
- `git clone https://github.com/safelibs/validator validator`
- `git -C validator rev-parse HEAD`
- `test -x safe/scripts/stage-validator-debs.sh`
- `bash -n safe/scripts/stage-validator-debs.sh`
- `git commit -m "test: add libbz2 validator package staging helper"`
- `cargo test --manifest-path safe/Cargo.toml --release`
- `bash safe/scripts/build-safe.sh --release`
- `bash safe/scripts/check-abi.sh --strict`
- `bash safe/scripts/build-debs.sh`
- `bash safe/scripts/check-package-layout.sh`
- `bash safe/scripts/stage-validator-debs.sh`
- `python3 -m unittest discover -s unit -v`
- `python3 tools/testcases.py --config repositories.yml --tests-root tests --check --library libbz2 --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
- `bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode original --library libbz2 --record-casts`
- `python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-original-validation-proof.json --mode original --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
- `bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
- `python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
- `python3 tools/render_site.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-path artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json --proof-path artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json --output-root site/libbz2-safe`
- `bash scripts/verify-site.sh --config repositories.yml --tests-root tests --artifacts-root artifacts/libbz2-safe --proof-path artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json --proof-path artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json --site-root site/libbz2-safe --library libbz2`
- Support rerun after neutralizing local provenance strings: `git commit -m "test: use neutral libbz2 validator provenance"`, `bash safe/scripts/build-debs.sh`, `bash safe/scripts/check-package-layout.sh`, `bash safe/scripts/stage-validator-debs.sh`, port matrix, port proof verification, site render, and site verification.

## Validator Outcomes

- Validator unit tests: 110 tests passed.
- Manifest check: passed with 135 `libbz2` cases, including 5 source cases and 130 usage cases.
- Original-mode proof: `validator/artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json`.
- Port-mode proof: `validator/artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json`.
- Review site: rendered under `validator/site/libbz2-safe/` and verified after the neutral local provenance support fix.

## Summaries

Original mode:

- Summary path: `validator/artifacts/libbz2-safe/results/libbz2/summary.json`
- Mode: `original`
- Cases: 135
- Source cases: 5
- Usage cases: 130
- Passed: 135
- Failed: 0
- Casts: 135

Port mode:

- Summary path: `validator/artifacts/libbz2-safe/port/results/libbz2/summary.json`
- Mode: `port`
- Cases: 135
- Source cases: 5
- Usage cases: 130
- Passed: 134
- Failed: 1
- Casts: 135

## Failures

### `usage-bzip2-vv-double-verbose`

- Title: `bzip2 -vv double verbose compress`
- Kind: `usage`
- Mode: `port`
- Result JSON: `validator/artifacts/libbz2-safe/port/results/libbz2/usage-bzip2-vv-double-verbose.json`
- Log path: `validator/artifacts/libbz2-safe/port/logs/libbz2/usage-bzip2-vv-double-verbose.log`
- Observed error: `testcase command exited with status 1`; the test expected stderr to contain `block 1`, but the port run only emitted the final compression ratio line for the sample payload.
- Original-mode comparison: the same testcase passed under original Ubuntu packages.
- Harness/setup classification: not a validator setup failure. Override packages installed successfully, the port lock matched copied debs, original mode passed, and proof validation accepted both modes.
- Preliminary port classification: libbz2-safe compression verbosity gap. `original/compress.c` prints per-block CRC and final combined CRC diagnostics at `verbosity >= 2` inside `BZ2_compressBlock`; the current Rust `safe/src/compress.rs` path preserves the compressed stream behavior but does not emit those `-vv` diagnostic lines.

## Fixes Applied In This Phase

- Added and committed `safe/scripts/stage-validator-debs.sh` to copy only canonical `libbz2` validator packages and generate `local-port-debs-lock.json` from copied files.
- Adjusted the helper's local provenance placeholders from `local/libbz2-safe` / `local-libbz2-safe` to `local/libbz2-port` / `local-libbz2-port`, because `verify-site.sh` rejects final user-facing safe/unsafe wording rendered outside testcase rows.
- No libbz2-safe runtime behavior fix was applied in this bootstrap phase.

## Next Failure Classes

- Port behavior: implement upstream-compatible `verbosity >= 2` compression diagnostics in the Rust compression path, including per-block CRC lines and final combined CRC output.
- Regression coverage: add a focused test around `BZ2_bzWriteOpen`/`BZ2_bzWriteClose64` or the packaged `bzip2 -vv` path to assert the upstream diagnostic text without weakening stream compatibility tests.

# Fix Source And C API Validator Failures Report

Phase impl_fix_validator_source_api_failures base commit: 809ee857b7c5ba82e62445abdbe51fe30e191646

## Source/API Failure Classification

The current port-mode validator artifacts do not contain source/API failures to reproduce in `safe/tests/`. The five `kind: source` cases named by this phase all passed in both original mode and port mode:

| Testcase | Original result | Port result | Port result/log evidence |
| --- | --- | --- | --- |
| `c-api-buffer-roundtrip` | `passed` | `passed` | `validator/artifacts/libbz2-safe/port/results/libbz2/c-api-buffer-roundtrip.json`, `validator/artifacts/libbz2-safe/port/logs/libbz2/c-api-buffer-roundtrip.log` |
| `cli-compress-decompress` | `passed` | `passed` | `validator/artifacts/libbz2-safe/port/results/libbz2/cli-compress-decompress.json`, `validator/artifacts/libbz2-safe/port/logs/libbz2/cli-compress-decompress.log` |
| `corrupted-stream-rejection` | `passed` | `passed` | `validator/artifacts/libbz2-safe/port/results/libbz2/corrupted-stream-rejection.json`, `validator/artifacts/libbz2-safe/port/logs/libbz2/corrupted-stream-rejection.log` |
| `debian-sample-parity` | `passed` | `passed` | `validator/artifacts/libbz2-safe/port/results/libbz2/debian-sample-parity.json`, `validator/artifacts/libbz2-safe/port/logs/libbz2/debian-sample-parity.log` |
| `stream-concatenation` | `passed` | `passed` | `validator/artifacts/libbz2-safe/port/results/libbz2/stream-concatenation.json`, `validator/artifacts/libbz2-safe/port/logs/libbz2/stream-concatenation.log` |

The matching validator scripts inspected for the classification are under `validator/tests/libbz2/tests/cases/source/`. Their current logs show successful C API buffer round trip, CLI round trip, corrupted-stream rejection, Debian sample parity, and concatenated-stream decompression through the staged port packages.

## Regression Tests And Fixes

- No new `safe/tests/validator_regressions.rs` test was added because there is no failing source/API validator case in the current artifacts.
- No `safe/src/*`, `safe/include/bzlib.h`, `safe/abi/*`, `safe/debian/*`, or `safe/scripts/*` changes were made in this phase.
- Existing source/API-adjacent regression coverage remains in `safe/tests/original_port.rs`, `safe/tests/golden_streams.rs`, `safe/tests/malformed_inputs.rs`, and `safe/tests/security_regressions.rs`; these passed as part of the release Cargo suite.
- Because no source, package, header, ABI, or script change was made, local `.deb` packages and validator artifacts were not rebuilt or restaged in this phase.

## Commands Executed

- `git rev-parse HEAD`
- `git status --short`
- Read the five current port source-case result JSON files and the five matching original-mode result JSON files.
- Read the five source-case scripts under `validator/tests/libbz2/tests/cases/source/`.
- Read the five current port source-case logs and the five matching original-mode logs.
- Python source-case comparison over `validator/artifacts/libbz2-safe/{results,port/results}/libbz2/*.json`
- Python remaining-failure summary over `validator/artifacts/libbz2-safe/port/results/libbz2/*.json`
- `git -C validator status --short`
- `cargo test --manifest-path safe/Cargo.toml --release`

## Result Status

- Release Cargo suite: passed.
- Validator checkout status: clean.
- Source/API validator status: no current source/API failures; all five source cases passed in original and port artifacts.
- Remaining port validator failure: `usage-bzip2-vv-double-verbose` (`kind: usage`), already documented in the bootstrap report as a verbose CLI diagnostics gap and outside the source/API scope of this phase.

# Fix CLI, Packaging, And Usage Validator Failures Report

Phase impl_fix_validator_cli_usage_failures base commit: e4f179b08a19ea41b059ea2441a3fcfc6d6bef5b

## Usage Failure Classification

- Current pre-fix port artifacts had one usage failure: `usage-bzip2-vv-double-verbose`.
- Family: `bzip2` verbose-status behavior. The testcase compresses with `bzip2 -vv -c`, round-trips the output, and requires `block 1`, `crc`, `combined CRC`, and `bits/byte` on stderr.
- Original-mode comparison: `validator/artifacts/libbz2-safe/results/libbz2/usage-bzip2-vv-double-verbose.json` passed.
- Port-mode pre-fix evidence: `validator/artifacts/libbz2-safe/port/results/libbz2/usage-bzip2-vv-double-verbose.json` failed before this phase because stderr only contained the final ratio line.
- Isolated cause: Rust library behavior in `safe/src/compress.rs`. Upstream `original/compress.c` emits per-block CRC accounting and final combined CRC diagnostics from `BZ2_compressBlock` at `verbosity >= 2`; the Rust port tracked the CRC values but did not print those diagnostics.
- Other CLI/script/package families (`bzcmp`/`bzdiff`, `bzgrep`, `bzmore`/`bzless`, `bzip2recover`, and package layout) had no remaining failed testcase IDs in the refreshed port validator summary.

## Fixes And Regression Coverage

- Source/test fix commit: `7db8d9a1861e3235ee1c1274fe261672cbc870dd` (`fix: restore libbz2 verbose compression diagnostics`).
- Changed `safe/src/compress.rs` to emit upstream-compatible `block ... crc ... combined CRC ... size` and `final combined CRC` stderr diagnostics from `BZ2_compressBlock` when `verbosity >= 2`.
- Added `safe/tests/link_contract.rs::relinked_original_bzip2_double_verbose_reports_block_crc`, which rebuilds the original `bzip2` CLI against `target/compat/libbz2.so.1.0.4`, runs `bzip2 -vv -c`, checks the validator-required stderr markers, and round-trips the compressed stream.
- No `safe/debian/*`, `safe/scripts/*`, `original/*`, or validator testcase/tool files were changed.

## Package Lock

Lock file: `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json`

Staged override root: `validator/artifacts/libbz2-safe/debs/local/libbz2/`

Canonical packages staged, in lock order:

| Package | Filename | Arch |
| --- | --- | --- |
| `libbz2-1.0` | `libbz2-1.0_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |
| `libbz2-dev` | `libbz2-dev_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |
| `bzip2` | `bzip2_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |

The lock file is the authoritative source for package sizes and SHA256 values because verifier phases rebuild and restage these `.deb` files. A lock/package consistency check confirmed the copied files and `local-port-debs-lock.json` describe the same three canonical packages. `bzip2-doc_*.deb` was rebuilt in `target/package/out/` but was not copied into the validator override root. `unported_original_packages` is `[]`.

## Commands Executed

- `git rev-parse HEAD`
- `cargo test --manifest-path safe/Cargo.toml --release relinked_original_bzip2_double_verbose_reports_block_crc -- --nocapture`
- `git commit -m "fix: restore libbz2 verbose compression diagnostics"`
- `cargo test --manifest-path safe/Cargo.toml --release`
- `bash safe/scripts/build-safe.sh --release`
- `bash safe/scripts/check-abi.sh --strict`
- `bash safe/scripts/link-original-tests.sh --all`
- `bash safe/scripts/build-original-cli-against-safe.sh --run-samples`
- `bash safe/scripts/build-debs.sh`
- `bash safe/scripts/check-package-layout.sh`
- `bash safe/scripts/run-debian-tests.sh --tests link-with-shared bigfile bzexe-test compare compress grep`
- `bash safe/scripts/stage-validator-debs.sh`
- Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and `validator/artifacts/libbz2-safe/debs/local/libbz2/*.deb`
- `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
- `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
- Python remaining-failure summary over `validator/artifacts/libbz2-safe/port/results/libbz2/*.json`
- `git -C validator status --short`
- `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`

## Validator Outcomes

- Focused regression test: passed.
- Release Cargo suite: passed.
- ABI strict check: passed.
- Relinked original object tests: passed.
- Relinked original CLI sample checks: passed.
- Package build, layout check, and selected Debian tests: passed.
- Validator checkout status: clean, with no diffs under testcase/tool/manifests checked by the senior tester command.
- Refreshed port result for fixed case: `validator/artifacts/libbz2-safe/port/results/libbz2/usage-bzip2-vv-double-verbose.json` now has `"status": "passed"` and `"exit_code": 0`.
- Fixed-case log evidence: `validator/artifacts/libbz2-safe/port/logs/libbz2/usage-bzip2-vv-double-verbose.log`.
- Port proof: `validator/artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json`.

## Final Port Summary

- Summary path: `validator/artifacts/libbz2-safe/port/results/libbz2/summary.json`
- Mode: `port`
- Cases: 135
- Source cases: 5
- Usage cases: 130
- Passed: 135
- Failed: 0
- Casts: 135
- Remaining failed testcase IDs: none.

# Fix Remaining Safety Or Compatibility Failures Report

Phase impl_fix_validator_remaining_failures base commit: f7ba86618ace872601ead029d216044002a7ed67

## Remaining Failure Extraction

- Re-extracted current per-case statuses from `validator/artifacts/libbz2-safe/port/results/libbz2/*.json`.
- Port mode currently has 135 `passed` case JSON files and no failed testcase IDs.
- Matching original mode currently has 135 `passed` case JSON files and no failed testcase IDs.
- The previously fixed `usage-bzip2-vv-double-verbose` result remains passed in refreshed artifacts: `validator/artifacts/libbz2-safe/port/results/libbz2/usage-bzip2-vv-double-verbose.json`.

## Fixes, Skips, And Blocks

- No new safelib-caused failures remained for this phase, so no additional `safe/src/*`, `safe/tests/*`, `safe/debian/*`, or `safe/scripts/*` edits were required.
- Existing regression coverage for the prior validator failure remains `safe/tests/link_contract.rs::relinked_original_bzip2_double_verbose_reports_block_crc`; the full release Cargo suite passed in this phase.
- Validator-bug skips: none.
- Environmental limitations: none encountered.
- Validator checkout status: clean; no diffs under validator testcase/tool/manifests checked by `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`.

## Package Lock

Lock file: `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json`

Staged override root: `validator/artifacts/libbz2-safe/debs/local/libbz2/`

Canonical packages staged, in lock order:

| Package | Filename | Arch |
| --- | --- | --- |
| `libbz2-1.0` | `libbz2-1.0_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |
| `libbz2-dev` | `libbz2-dev_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |
| `bzip2` | `bzip2_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` |

The copied override `.deb` files and `local-port-debs-lock.json` were checked against file names, sizes, and SHA256 values. Exact sizes and hashes are intentionally not duplicated in this report because verifier phases rebuild and restage these packages; the current lock is authoritative. The lock describes exactly `libbz2-1.0`, `libbz2-dev`, and `bzip2`; `unported_original_packages` is `[]`.

## Commands Executed

- `git rev-parse HEAD`
- Python failure summary over `validator/artifacts/libbz2-safe/{results,port/results}/libbz2/*.json`
- `cargo test --manifest-path safe/Cargo.toml --release`
- `bash safe/scripts/build-safe.sh --release`
- `bash safe/scripts/check-abi.sh --strict`
- `bash safe/scripts/build-debs.sh`
- `bash safe/scripts/check-package-layout.sh`
- `bash safe/scripts/stage-validator-debs.sh`
- Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and `validator/artifacts/libbz2-safe/debs/local/libbz2/*.deb`
- `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode original --library libbz2 --record-casts`
- `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-original-validation-proof.json --mode original --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
- `cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts`
- `cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
- Python remaining-failure summary over refreshed original and port result JSON files
- `git -C validator status --short`
- `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`

## Validator Outcomes

Original mode:

- Summary path: `validator/artifacts/libbz2-safe/results/libbz2/summary.json`
- Cases: 135
- Source cases: 5
- Usage cases: 130
- Passed: 135
- Failed: 0
- Casts: 135
- Proof: `validator/artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json`

Port mode:

- Summary path: `validator/artifacts/libbz2-safe/port/results/libbz2/summary.json`
- Cases: 135
- Source cases: 5
- Usage cases: 130
- Passed: 135
- Failed: 0
- Casts: 135
- Proof: `validator/artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json`
- Remaining failed testcase IDs: none.
