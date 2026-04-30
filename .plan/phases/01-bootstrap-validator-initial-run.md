# Phase Name

Bootstrap Validator And Capture Initial Run

# Implement Phase ID

`impl_validator_bootstrap_and_initial_run`

# Preexisting Inputs

- `safe/`: established Rust port crate named `libbz2-safe`, with library outputs `cdylib`, `staticlib`, and `rlib` under the C-facing crate name `bz2`.
- `original/`: upstream C libbz2 implementation, fixtures, Debian packaging inputs, CLI scripts, and original C test programs.
- `safe/Cargo.toml`, `safe/Cargo.lock`, `safe/build.rs`, `safe/src/`, `safe/tests/`, `safe/debian/`, `safe/scripts/`, `safe/abi/`, and `safe/include/bzlib.h`.
- Required existing local scripts: `safe/scripts/build-safe.sh`, `safe/scripts/check-abi.sh`, `safe/scripts/build-debs.sh`, and `safe/scripts/check-package-layout.sh`.
- Existing validator-relevant generated artifacts under `target/original-baseline/`, especially `public_api_test.o`, `bzip2.o`, and `dlltest.o`.
- Existing validator-relevant generated outputs under `target/compat/`, preserved unless refreshed by `safe/scripts/build-safe.sh --release`.
- Current package outputs under `target/package/out/`, including canonical package candidates for `libbz2-1.0`, `libbz2-dev`, and `bzip2`; rebuild only when absent, incomplete, or stale.
- Parent git worktree may already contain unrelated changes in `.plan/phases/01-document-libbz2-port.md`, `.plan/workflow-structure.yaml`, and generated `target/original-baseline/*` files. Do not revert them or include them in phase commits unless this phase intentionally updates them.
- Network access to `https://github.com/safelibs/validator`.
- Docker, `python3`, `cargo`, `rustc`, `gcc`, `dpkg-deb`, and the tools required by `safe/scripts/build-debs.sh`.

Do not treat documentation, CVE, downstream, benchmark, or aggregate-security artifacts such as `safe/docs/unsafe-audit.md`, `dependents.json`, `relevant_cves.json`, `all_cves.json`, `target/bench/`, or `target/security/` as validator inputs for this phase.

# New Outputs

- Parent repo `.git/info/exclude` contains `/validator/`, preventing the nested checkout from being staged as a parent-repo gitlink.
- `validator/` checkout cloned or fast-forwarded with `git pull --ff-only` if a prior interrupted run already created it, with the resulting commit recorded in `validator-report.md`. The planned target suite version is `5d908be26e33f071e119ffe1a52e3149f1e5ec4e`.
- `safe/scripts/stage-validator-debs.sh`, executable and committed, stages exactly the canonical local `.deb` files into the validator override tree and regenerates `local-port-debs-lock.json` from the copied files.
- Fresh or confirmed local `.deb` packages in `target/package/out/`.
- Validator-local override package copies in `validator/artifacts/libbz2-safe/debs/local/libbz2/`.
- Validator-local lock file at `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json`.
- Original-mode results under `validator/artifacts/libbz2-safe/results/libbz2/`.
- Port-mode results under `validator/artifacts/libbz2-safe/port/results/libbz2/`.
- Proof JSON files under `validator/artifacts/libbz2-safe/proof/`.
- Rendered review site under `validator/site/libbz2-safe/` when proof files are produced.
- Initial `validator-report.md`.
- Git commit(s) containing `safe/scripts/stage-validator-debs.sh`, `validator-report.md`, and any other intentionally committed support changes.

# File Changes

- Create or rewrite `validator-report.md`.
- Create `safe/scripts/stage-validator-debs.sh`.
- Do not commit the nested `validator/` checkout to the parent repository.
- Do not commit `.git/info/exclude`; it is only a local guard.
- Do not modify validator source files, manifests, tests, tools, or scripts.
- Do not modify `safe/` in this phase beyond `safe/scripts/stage-validator-debs.sh` unless the package build exposes a trivial setup defect that must be fixed before validator execution. If that happens, add a regression or packaging test as appropriate and commit it before running the validator.

# Implementation Details

Port architecture to preserve:

- `safe/src/lib.rs` preserves the upstream module split.
- ABI-visible structs and handles are in `safe/src/types.rs`.
- Low-level compression is in `safe/src/compress.rs`; decompression is in `safe/src/decompress.rs`.
- Buffer wrappers, version, and assertion ABI functions are in `safe/src/ffi.rs`.
- `FILE*`, fd, and `BZFILE*` wrappers are in `safe/src/stdio.rs`.
- Algorithm support and constants live in `safe/src/blocksort.rs`, `safe/src/huffman.rs`, `safe/src/crc.rs`, `safe/src/rand.rs`, `safe/src/alloc.rs`, and `safe/src/constants.rs`.
- Key public ABI entry points are `BZ2_bzBuffToBuffCompress` and `BZ2_bzBuffToBuffDecompress` in `safe/src/ffi.rs:65` and `safe/src/ffi.rs:115`; `BZ2_bzCompressInit`, `BZ2_bzCompress`, and `BZ2_bzCompressEnd` in `safe/src/compress.rs:797`, `safe/src/compress.rs:881`, and `safe/src/compress.rs:943`; `BZ2_bzDecompressInit`, `BZ2_bzDecompress`, and `BZ2_bzDecompressEnd` in `safe/src/decompress.rs:1064`, `safe/src/decompress.rs:1099`, and `safe/src/decompress.rs:1169`; and wrapper APIs from `BZ2_bzReadOpen` through `BZ2_bzerror` in `safe/src/stdio.rs:182-621`.
- `safe/build.rs` wires SONAME/version-script behavior; `safe/abi/` stores expected exports and platform export maps; `safe/include/bzlib.h` must remain byte-identical to `original/bzlib.h`.

Workflow requirements:

- Before editing files, capture the phase base commit:

```bash
phase_base=$(git rev-parse HEAD)
```

- Clone or update the validator without committing it to the parent repository. Use the plan's fast-forward/update flow and record the resulting validator commit; later phases must consume that recorded commit and must not reclone, pull, or otherwise change the validator suite version:

```bash
grep -qxF '/validator/' .git/info/exclude || printf '/validator/\n' >> .git/info/exclude
if [ -d validator/.git ]; then
  git -C validator pull --ff-only
else
  git clone https://github.com/safelibs/validator validator
fi
git -C validator rev-parse HEAD
```

- Validate the existing local `safe/` crate before packaging:

```bash
cargo test --manifest-path safe/Cargo.toml --release
bash safe/scripts/build-safe.sh --release
bash safe/scripts/check-abi.sh --strict
```

- Create `safe/scripts/stage-validator-debs.sh` as an executable support script and use it for every validator port-mode run in this plan.
- The script must stage only the canonical validator packages `libbz2-1.0`, `libbz2-dev`, and `bzip2`. It must exclude `bzip2-doc_*.deb`.
- The script must create needed `validator/artifacts/libbz2-safe/...` directories, copy from `target/package/out/` into `validator/artifacts/libbz2-safe/debs/local/libbz2/`, remove stale copied `.deb` files first, reject zero or multiple matches for any canonical package, and reject any copied noncanonical package.
- The script must generate `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` from the copied files, not from `target/package/out/`.
- The lock must use schema version 1 with top-level `schema_version: 1`, `mode: "port"`, `generated_at`, `source_config`, `source_inventory`, and `libraries`.
- The lock must contain exactly one library entry with `library: "libbz2"`, non-empty `repository`, `tag_ref`, `commit`, and `release_tag`, `tag_ref == refs/tags/<release_tag>`, and `commit` equal to the 40-character lowercase parent repo `git rev-parse HEAD` used to build the packages.
- The lock `debs` must be in canonical package order for `libbz2-1.0`, `libbz2-dev`, and `bzip2`, each with `package`, `filename`, `architecture`, lowercase `sha256`, and byte `size`.
- The lock must set `unported_original_packages: []`.
- Stable explicit local placeholders such as `local/libbz2-safe` and `local-libbz2-safe` may be used for `repository`, `source_inventory`, and `release_tag`; report them as local override provenance in `validator-report.md`.
- The script must read `Package` and `Architecture` from copied files with `dpkg-deb --field`, verify canonical package names, and allow only validator-supported native architectures `amd64` and `all`.
- Validate the script:

```bash
test -x safe/scripts/stage-validator-debs.sh
bash -n safe/scripts/stage-validator-debs.sh
```

- Commit `safe/scripts/stage-validator-debs.sh` before building packages and generating the lock, so the commit recorded in the lock corresponds to a committed tree. Suitable commit message: `test: add libbz2 validator package staging helper`.
- Build fresh local Debian packages if `target/package/out/` is absent, incomplete, or older than source changes:

```bash
bash safe/scripts/build-debs.sh
bash safe/scripts/check-package-layout.sh
```

- Stage validator override packages and generate the lock:

```bash
bash safe/scripts/stage-validator-debs.sh
```

- Run validator tool and manifest checks:

```bash
cd validator && python3 -m unittest discover -s unit -v
cd validator && python3 tools/testcases.py --config repositories.yml --tests-root tests --check --library libbz2 --min-source-cases 5 --min-usage-cases 130 --min-cases 135
```

- Run original-mode libbz2 matrix as validator environment control:

```bash
cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode original --library libbz2 --record-casts
cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-original-validation-proof.json --mode original --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135
```

- Run port-mode matrix against local override packages:

```bash
cd validator && bash test.sh --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --mode port --override-deb-root artifacts/libbz2-safe/debs/local --port-deb-lock artifacts/libbz2-safe/proof/local-port-debs-lock.json --library libbz2 --record-casts
cd validator && python3 tools/verify_proof_artifacts.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135
```

- Do not rely on the port matrix exit code alone. Inspect `validator/artifacts/libbz2-safe/port/results/libbz2/summary.json` and every result JSON whose `status` is `failed`.
- Render and verify a local review site if both proof files were produced:

```bash
cd validator && python3 tools/render_site.py --config repositories.yml --tests-root tests --artifact-root artifacts/libbz2-safe --proof-path artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json --proof-path artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json --output-root site/libbz2-safe
cd validator && bash scripts/verify-site.sh --config repositories.yml --tests-root tests --artifacts-root artifacts/libbz2-safe --proof-path artifacts/libbz2-safe/proof/libbz2-original-validation-proof.json --proof-path artifacts/libbz2-safe/proof/libbz2-port-validation-proof.json --site-root site/libbz2-safe --library libbz2
```

- Write `validator-report.md` with the exact line `Phase impl_validator_bootstrap_and_initial_run base commit: <phase_base>`.
- Include validator commit, safe source commit used to build packages, package filenames and SHA256 values, validator commands executed, original-mode summary, port-mode summary, every failed testcase ID, title, kind, result JSON path, log path, observed error, preliminary classification, fixes applied in this phase, and next failure classes for later phases.
- If no applicable failures or source changes are needed, still commit a report-only no-op note or a narrowly named empty commit after ensuring the report contains required evidence and base-line content.

# Verification Phases

## `check_initial_validator_run_software_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_validator_bootstrap_and_initial_run`
- Purpose: verify the validator checkout, local package staging, lock generation, original and port matrix execution, and initial failure capture.
- Commands:
  - `git -C validator rev-parse HEAD`
  - `test -x safe/scripts/stage-validator-debs.sh`
  - `bash -n safe/scripts/stage-validator-debs.sh`
  - `python3 validator/tools/testcases.py --config validator/repositories.yml --tests-root validator/tests --check --library libbz2 --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - `test -f validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json`
  - `find validator/artifacts/libbz2-safe/debs/local/libbz2 -maxdepth 1 -type f -name '*.deb' | sort`
  - A short Python lock/package consistency check over `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` and `validator/artifacts/libbz2-safe/debs/local/libbz2/*.deb`, asserting schema version 1, mode `port`, one library entry with `library == "libbz2"`, canonical deb packages exactly `libbz2-1.0`, `libbz2-dev`, and `bzip2`, `unported_original_packages == []`, and matching file sizes and SHA256 values.
  - `python3 validator/tools/verify_proof_artifacts.py --config validator/repositories.yml --tests-root validator/tests --artifact-root validator/artifacts/libbz2-safe --proof-output proof/libbz2-original-validation-proof.json --mode original --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - `python3 validator/tools/verify_proof_artifacts.py --config validator/repositories.yml --tests-root validator/tests --artifact-root validator/artifacts/libbz2-safe --proof-output proof/libbz2-port-validation-proof.json --mode port --library libbz2 --require-casts --min-source-cases 5 --min-usage-cases 130 --min-cases 135`
  - A short Python summary over `validator/artifacts/libbz2-safe/{results,port/results}/libbz2/*.json` to list failed testcase IDs.

## `check_initial_validator_run_senior_tester`

- Type: `check`
- Fixed `bounce_target`: `impl_validator_bootstrap_and_initial_run`
- Purpose: review whether the initial report correctly distinguishes validator harness/setup failures, original-Ubuntu failures, and libbz2-safe port failures.
- Commands:
  - `sed -n '1,260p' validator-report.md`
  - `git status --short`
  - `base=$(awk '/^Phase impl_validator_bootstrap_and_initial_run base commit: / {print $NF}' validator-report.md | tail -n1); test -n "$base"; git log --oneline "$base"..HEAD; git diff --stat "$base"..HEAD -- safe/scripts validator-report.md; git diff --name-only "$base"..HEAD`
  - `git ls-files --stage validator`
  - `git -C validator status --short`
  - `git -C validator diff -- tests tools scripts unit inventory repositories.yml test.sh conftest.py Makefile README.md`
  - Inspect representative failing `validator/artifacts/libbz2-safe/port/logs/libbz2/*.log` files if the summary reports failures.

# Success Criteria

- `validator/` exists at the recorded validator commit and no parent-repo gitlink for `validator/` is staged or committed.
- `git -C validator status --short` is clean except ignored generated artifacts, and validator source files are not modified.
- `validator/artifacts/libbz2-safe/debs/local/libbz2/` contains exactly one copied `.deb` each for `libbz2-1.0`, `libbz2-dev`, and `bzip2`, and no `bzip2-doc`.
- `validator/artifacts/libbz2-safe/proof/local-port-debs-lock.json` matches those copied files and is accepted by the port validator invocation.
- The validator manifest check confirms 135 `libbz2` cases: 5 source and 130 usage.
- `validator-report.md` exists and records validator commit, safe commit, commands, counts, package hashes, original-mode results, port-mode results, and every initial failure with evidence paths.

# Git Commit Requirement

The implementer must commit work to git before yielding. Commit `safe/scripts/stage-validator-debs.sh` before package staging and lock generation, then commit `validator-report.md` after the initial validator run. Do not include unrelated dirty files or the nested `validator/` checkout in parent-repo commits.
