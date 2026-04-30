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

| Package | Filename | Arch | Size | SHA256 |
| --- | --- | --- | ---: | --- |
| `libbz2-1.0` | `libbz2-1.0_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` | 183444 | `d5a0f2bfbfed8e889840af6681c84ecccf7b1a5ce5bb4b6a4eb08486d305b1f3` |
| `libbz2-dev` | `libbz2-dev_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` | 8579914 | `ca350e3b8d2b4153530a82de7711093af0142f79494c92ed14badcb990bb8501` |
| `bzip2` | `bzip2_1.0.8-5.1build0.1+safelibs1_amd64.deb` | `amd64` | 35080 | `e87e2f066463618d4df26064e9e1450c5bdaa9a53d01f8591443efcf51b95ce3` |

`bzip2-doc_*.deb` was not copied. `unported_original_packages` is `[]`.

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
