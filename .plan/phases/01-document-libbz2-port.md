# Document The Port And Reconcile Evidence

## Phase Name
Document The Port And Reconcile Evidence

## Implement Phase ID
`impl_document_libbz2_port`

## Preexisting Inputs
- Required repository inputs:
  - `safe/Cargo.toml`, `safe/Cargo.lock`, and `safe/build.rs`
  - `safe/src/`
  - `safe/tests/`
  - `safe/abi/`
  - `safe/include/bzlib.h`
  - `safe/debian/`
  - `safe/scripts/`
  - `safe/docs/unsafe-audit.md`
  - `dependents.json`
  - `relevant_cves.json`
  - `all_cves.json`
  - `original/`
  - `test-original.sh`
- Baseline generated evidence already expected in this checkout:
  - `target/original-baseline/`, including the prepared baseline objects `public_api_test.o`, `bzip2.o`, and `dlltest.o`
- Consume-if-present artifacts. These are inputs when already present, but they are not prerequisites and must not be promoted into required inputs or regenerated unless a later step actually needs fresh evidence:
  - `target/compat/`
  - `target/package/out/package-manifest.txt`
  - `target/package/unpacked/`
  - `target/bench/summary.txt`
  - `target/security/summary.txt`
  - any referenced `target/security/*.log`
- Optional rerun input:
  - `safe/PORT.md` if it already exists before execution

## New Outputs
- `safe/PORT.md`, created on the first run or refreshed in place on reruns
- One git commit containing the documentation pass and any narrowly justified incidental fix
- Optional refreshed artifacts under `target/original-baseline/`, `target/compat/`, `target/package/`, and `target/bench/` only when the document needs fresh evidence and the corresponding prepared artifact is absent, stale, or insufficient; optional refreshed `target/security/` outputs only if `safe/scripts/run-full-suite.sh` is intentionally rerun

## File Changes
- Create or update `safe/PORT.md`
- No committed source, script, or packaging edits are planned
- If a documentation-blocking defect is discovered while reconciling code or harnesses, allow one narrowly scoped incidental fix in the same phase, but justify it in `safe/PORT.md` and in the commit summary

## Implementation Details
1. Preserve the workflow contract exactly:
   - Keep the workflow linear only.
   - Keep the generated YAML inline-only and self-contained.
   - Use exactly one implement phase, `impl_document_libbz2_port`.
   - Keep every verifier as an explicit top-level `check` phase with fixed `bounce_target: impl_document_libbz2_port`.
   - Put any required builds, tests, ABI inspections, grep scans, packaging commands, downstream smokes, or benchmarks directly in checker instructions rather than separate non-agentic phases.
   - Keep the direct-Cargo-versus-staged-artifact split explicit: use `safe/target/...` for direct Cargo outputs and repo-root `target/...` for staged baseline, compat, packaging, benchmark, and optional security evidence.
2. Handle `safe/PORT.md` correctly:
   - If it is absent, create it from scratch.
   - If it already exists on a rerun, update it in place and preserve prose that still matches the tree instead of replacing it wholesale.
3. Consume existing artifacts before regenerating anything:
   - Treat `safe/`, `original/`, `safe/abi/*`, `safe/include/bzlib.h`, `safe/debian/*`, `safe/scripts/*`, `safe/tests/*`, `safe/docs/unsafe-audit.md`, `dependents.json`, `relevant_cves.json`, and `all_cves.json` as authoritative inputs.
   - Consume prepared artifacts in place when they already exist: `target/original-baseline/`, `target/compat/`, `target/package/out/package-manifest.txt`, `target/package/unpacked/`, `target/bench/summary.txt`, and optional `target/security/summary.txt` plus any referenced `target/security/*.log`.
   - Do not regenerate optional aggregate security artifacts merely for completeness. If `target/security/summary.txt` is absent, rely on the direct per-harness evidence required elsewhere in the plan.
4. Build section `1. **High-level architecture.**` from `safe/Cargo.toml`, `safe/src/lib.rs`, `safe/src/types.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/ffi.rs`, `safe/src/stdio.rs`, `safe/build.rs`, `safe/abi/*`, `safe/debian/rules`, `safe/scripts/build-safe.sh`, `safe/scripts/link-original-tests.sh`, and `safe/scripts/build-original-cli-against-safe.sh`:
   - Describe the single-crate layout and declared `cdylib`, `staticlib`, and `rlib` outputs.
   - Explain the boundary between the public C ABI/API surface and the internal Rust implementation.
   - Trace the data flow from `bz_stream` and `BZFILE`-style handles into `EState`, `DState`, and wrapper state.
   - Describe Linux SONAME/version-script wiring, Windows `.def` export wiring, Debian packaging, and how original CLI sources are relinked against the safe library.
   - Include a short directory map covering `safe/src/`, `safe/tests/`, `safe/abi/`, `safe/include/`, `safe/docs/`, `safe/debian/`, `safe/scripts/`, `original/`, and `target/original-baseline/`.
5. Build section `2. **Where the unsafe Rust lives.**` from a full-tree unsafe scan plus direct reads of every real hit in `safe/src/*.rs` and `safe/tests/*.rs`:
   - Start from `grep -RIn '\bunsafe\b' safe`.
   - Reconcile the whole-tree results with focused scans such as `rg -n '\bunsafe\b|extern "C"|no_mangle|unsafe fn|unsafe impl|unsafe extern|static mut' safe/src safe/tests`.
   - Group the inventory by purpose, but list every real site explicitly with file:line references and a one-sentence justification.
   - Cover at minimum exported ABI entry points and mutable ABI globals, raw-pointer reconstruction from `bz_stream.state` and `BZFILE`-style handles, allocator callback bridging through `bzalloc` and `bzfree`, `FILE*` and fd wrapper interop, and test-only unsafe for calling the C ABI or zero-initializing `bz_stream`.
   - Use `safe/docs/unsafe-audit.md` as the prior bucket summary, but revalidate it against the current tree and note any drift.
   - Call out unsafe code that is not strictly required by the original C ABI/API boundary as a separate subsection.
   - Filter out false positives from comments, strings, docs, and build outputs rather than treating them as real unsafe sites.
6. Build section `3. **Remaining unsafe FFI beyond the original ABI/API boundary.**` from the actual runtime imports in `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/ffi.rs`, and `safe/src/stdio.rs`:
   - Inventory only true foreign-library or system-runtime surfaces beyond the original libbz2 export set.
   - Include symbols such as `malloc`, `free`, `fprintf`, `fputs`, `exit`, `fdopen`, `fclose`, `fflush`, `fread`, `fwrite`, `ferror`, `fgetc`, `ungetc`, `fopen64` or `fopen`, and the `stdin`, `stdout`, and `stderr` globals when they are actually imported.
   - Do not misclassify crate-internal `extern "C"` declarations that target symbols exported by the same crate, including `BZ2_bz__AssertH__fail`, `BZ2_blockSort`, `BZ2_hbAssignCodes`, `BZ2_hbCreateDecodeTables`, `BZ2_hbMakeCodeLengths`, `BZ2_decompress`, and `BZ2_indexIntoF`.
   - Keep runtime port surface separate from test scaffolding. Test-only `extern` imports may be cited only as verifier details.
   - For each true foreign surface, record the symbol set, provider library, why it is needed, and what a plausible safer Rust replacement would require.
7. Build section `4. **Remaining issues.**` from observed evidence, not assumptions:
   - Report the `TODO`/`FIXME`/`XXX`/`HACK` scan result.
   - Record failing or skipped harnesses, current packaging or downstream caveats, any known or observed non-bit-for-bit-equivalent behavior against upstream, performance evidence or the lack of reproducible benchmark evidence, dependents exercised or not exercised from `dependents.json`, and in-scope CVE classes from `relevant_cves.json` versus broader exclusions from `all_cves.json`.
   - For any build, test, package, benchmark, or downstream harness run during the refresh, capture the exact command, whether it exited zero, and the relevant failure output before moving on.
   - Treat a nonzero exit from an evidence harness as section 4 evidence unless the underlying defect is fixed in the same phase; do not stop the documentation pass early just because a harness failed.
   - Actively run and interpret `safe/tests/golden_streams.rs`, `safe/tests/original_port.rs`, `safe/tests/link_contract.rs`, `safe/scripts/link-original-tests.sh`, `safe/tests/regression_mk251.rs`, `safe/tests/malformed_inputs.rs`, and `safe/tests/security_regressions.rs`.
   - If all parity checks pass, describe the exact scope of that evidence instead of claiming universal equivalence.
   - If `target/bench/summary.txt` is absent and fresh benchmark evidence is necessary, first confirm `target/compat/cargo/release/libbz2.so` exists and restage compat with `bash safe/scripts/build-safe.sh --release` if it does not before running `env LIBBZ2_BENCH_CAPTURE_SECURITY_LOG=0 bash safe/scripts/benchmark-compare.sh`.
   - If `target/package/out/package-manifest.txt` is absent and current package evidence is necessary, attempt `bash safe/scripts/build-debs.sh` before package or downstream verification.
   - If `target/security/summary.txt` exists, consult it and any referenced `target/security/*.log` files before making consolidated release-gate claims. If it is absent, rely on directly captured harness results instead of inventing a missing-summary issue.
8. Build section `5. **Dependencies and other libraries used.**` from `safe/Cargo.toml`, `cargo metadata`, `cargo tree`, `safe/debian/control`, `safe/debian/rules`, `safe/scripts/build-debs.sh`, `safe/scripts/run-debian-tests.sh`, `safe/scripts/benchmark-compare.sh`, and `test-original.sh`:
   - State explicitly that the crate has zero direct Rust dependencies.
   - Keep runtime C-library usage, host build and test tools, and Debian packaging dependencies as separate sublists.
   - Include the declared and invoked dependencies called out by the plan, including `debhelper-compat (= 13)`, `cargo`, `rustc`, `gcc`, `make`, `docker`, `dpkg-buildpackage`, `dpkg-deb`, `python3`, `tar`, `docbook-xml`, `docbook2x`, `texinfo`, `xsltproc`, and container-image prerequisites such as `build-essential`, `ca-certificates`, `dpkg-dev`, `fakeroot`, and `pkg-config`.
   - Identify any dependency or tool with meaningful unsafe exposure and explain why that exposure is acceptable in this port.
   - Note that there are no third-party Rust crates to audit and that `cargo geiger` was unavailable unless the environment changes.
9. Build section `6. **How this document was produced.**` from the commands actually run during the refresh:
   - Record the concrete grep, Cargo, ABI, test, package, and benchmark commands actually used.
   - Record the key files consulted so the refresh is reproducible.
10. Run a final citation and sanity pass before committing:
   - Every referenced path must exist.
   - Every referenced symbol must be discoverable with `rg`.
   - Every named dependency must appear in a cited manifest, control file, or script.
   - The section 2 unsafe inventory must match the real source-side unsafe occurrences after false-positive filtering.
   - The section 4 TODO statement must match the actual scan result.
   - The section 4 parity statement must match the actual results of `golden_streams`, `original_port`, `link_contract`, and any rerun benchmark or relink harnesses.
   - Any checker or implementation step that gathers evidence from a build, test, package, benchmark, or downstream harness that may legitimately fail must use capture-and-continue shell logic rather than bare hard-failing invocation.
   - If a checker reveals a documentation-blocking defect in code or harnesses, make a narrowly justified incidental fix in this phase, recommit, and rerun the failed checker.

## Verification Phases
### `check_port_md_source_inventory`
- `phase_id`: `check_port_md_source_inventory`
- `type`: `check`
- `bounce_target`: `impl_document_libbz2_port`
- `purpose`: verify the crate and module layout, direct dependency story, public ABI surface, unsafe inventory inputs, non-ABI FFI inventory, and TODO/FIXME scan grounding directly from the source tree and Cargo metadata
- `commands`:

```bash
cargo metadata --format-version 1 --no-deps --manifest-path safe/Cargo.toml
cargo tree --manifest-path safe/Cargo.toml
sed -n '1,240p' safe/docs/unsafe-audit.md
grep -RIn '\bunsafe\b' safe
rg -n '\bunsafe\b|extern "C"|no_mangle|unsafe fn|unsafe impl|unsafe extern|static mut' safe/src safe/tests
rg -n 'fdopen|fopen64|fopen|fclose|fflush|fread|fwrite|ferror|fgetc|ungetc|fprintf|fputs|exit|malloc|free|stdin|stdout|stderr' safe/src safe/tests
rg -n '\b(TODO|FIXME|XXX|HACK)\b' safe original -g '!safe/target/**' -g '!target/**' || true
if command -v cargo-geiger >/dev/null 2>&1; then cargo geiger --manifest-path safe/Cargo.toml; else echo 'cargo-geiger unavailable'; fi
```

- `review_checks`:
  - Section 2 must enumerate every real `unsafe` block, `unsafe fn`, `unsafe extern`, `unsafe impl`, and mutable ABI global read/write site in `safe/src/*.rs` and `safe/tests/*.rs`, grouped by purpose and cited with file:line references.
  - Any extra `grep -RIn '\bunsafe\b' safe` hits coming only from docs, comments, strings, or build outputs must be explicitly recognized as false positives.
  - Section 2 must either confirm that `safe/docs/unsafe-audit.md` still matches the current unsafe buckets or explain the drift.
  - Section 3 must separate intended libbz2 ABI exports from remaining libc/system FFI and must not misclassify crate-internal `extern "C"` references such as `BZ2_bz__AssertH__fail`, `BZ2_blockSort`, `BZ2_hb*`, `BZ2_decompress`, or `BZ2_indexIntoF` as extra foreign dependencies.
  - Section 4 must explicitly report the TODO/FIXME scan result.
  - Section 5 must remain grounded in dependency, build, and packaging sources rather than unsafe-audit or CVE/downstream JSON files.

### `check_port_md_build_abi_and_symbol_grounding`
- `phase_id`: `check_port_md_build_abi_and_symbol_grounding`
- `type`: `check`
- `bounce_target`: `impl_document_libbz2_port`
- `purpose`: verify build wiring, symbol and export claims, SONAME grounding, baseline undefined-symbol evidence, and the distinction between direct Cargo artifacts and staged compat artifacts
- `commands`:

```bash
capture() {
  local name="$1"
  shift
  echo ">>> $name"
  if "$@"; then
    echo "__STATUS__ $name 0"
  else
    local status=$?
    echo "__STATUS__ $name $status"
  fi
}

if [[ ! -f target/original-baseline/dlltest.o ]]; then
  capture build_original_baseline bash safe/scripts/build-original-baseline.sh
fi

capture cargo_build_release cargo build --manifest-path safe/Cargo.toml --release --locked

if [[ -f safe/target/release/libbz2.so ]]; then
  capture safe_release_soname bash -lc "objdump -p safe/target/release/libbz2.so | grep SONAME"
  capture safe_release_exports bash -lc "nm -D --defined-only safe/target/release/libbz2.so | grep ' BZ2_'"
else
  echo '__MISSING__ safe/target/release/libbz2.so'
fi

for object in public_api_test.o bzip2.o dlltest.o; do
  if [[ -f "target/original-baseline/$object" ]]; then
    capture "baseline_undefined_${object}" bash -lc "readelf -Ws target/original-baseline/$object | awk '\$7 == \"UND\" { print \$8 }' | sort -u"
  else
    echo "__MISSING__ target/original-baseline/$object"
  fi
done

if [[ ! -f target/compat/libbz2.so.1.0.4 ]]; then
  capture build_safe_release bash safe/scripts/build-safe.sh --release
fi
capture check_abi_strict bash safe/scripts/check-abi.sh --strict
if [[ -f target/compat/libbz2.so.1.0.4 ]]; then
  capture compat_soname bash -lc "objdump -p target/compat/libbz2.so.1.0.4 | grep SONAME"
  capture compat_exports bash -lc "nm -D --defined-only target/compat/libbz2.so.1.0.4 | grep ' BZ2_'"
else
  echo '__MISSING__ target/compat/libbz2.so.1.0.4'
fi
```

- `review_checks`:
  - Section 1 must explain how `safe/build.rs`, `safe/abi/*`, `safe/include/bzlib.h`, `safe/scripts/build-safe.sh`, `safe/scripts/link-original-tests.sh`, and `safe/scripts/build-original-cli-against-safe.sh` fit together.
  - The document must use the correct direct Cargo artifact path, `safe/target/release/libbz2.so`, and distinguish it from the staged compat artifact, `target/compat/libbz2.so.1.0.4`.
  - If the baseline refresh, compat build, or ABI script fails, section 4 must record the exact failing harness and failure mode instead of implying success.
  - A captured nonzero exit from one of the commands above is only a checker failure when `safe/PORT.md` omits or misstates it.

### `check_port_md_equivalence_and_remaining_issues`
- `phase_id`: `check_port_md_equivalence_and_remaining_issues`
- `type`: `check`
- `bounce_target`: `impl_document_libbz2_port`
- `purpose`: verify upstream behavioral parity, bit-for-bit coverage scope, regression and CVE coverage, remaining-issues statements, and the benchmark-evidence rules around release-staged compat artifacts
- `commands`:

```bash
capture() {
  local name="$1"
  shift
  echo ">>> $name"
  if "$@"; then
    echo "__STATUS__ $name 0"
  else
    local status=$?
    echo "__STATUS__ $name $status"
  fi
}

if [[ ! -f target/original-baseline/dlltest.o ]]; then
  capture build_original_baseline bash safe/scripts/build-original-baseline.sh
fi

sed -n '1,220p' relevant_cves.json
rg -n 'selected_cve_count|current_upstream_product_cve_count|debian_tracker_extra_cve_ids|"cve_id"' relevant_cves.json all_cves.json
if [[ -f target/security/summary.txt ]]; then
  sed -n '1,200p' target/security/summary.txt
else
  echo 'target/security/summary.txt absent; using direct harness evidence instead'
fi

capture test_original_port cargo test --manifest-path safe/Cargo.toml --release --test original_port
capture test_golden_streams cargo test --manifest-path safe/Cargo.toml --release --test golden_streams
capture test_compression_port cargo test --manifest-path safe/Cargo.toml --release --test compression_port
capture test_decompress_port cargo test --manifest-path safe/Cargo.toml --release --test decompress_port
capture test_regression_mk251 cargo test --manifest-path safe/Cargo.toml --release --test regression_mk251
capture test_malformed_inputs cargo test --manifest-path safe/Cargo.toml --release --test malformed_inputs
capture test_security_regressions cargo test --manifest-path safe/Cargo.toml --release --test security_regressions
if [[ -f target/bench/summary.txt ]]; then
  sed -n '1,200p' target/bench/summary.txt
else
  if [[ ! -f target/compat/cargo/release/libbz2.so ]]; then
    capture build_safe_release_for_bench bash safe/scripts/build-safe.sh --release
  fi
  capture benchmark_compare env LIBBZ2_BENCH_CAPTURE_SECURITY_LOG=0 bash safe/scripts/benchmark-compare.sh
  [[ -f target/bench/summary.txt ]] && sed -n '1,200p' target/bench/summary.txt || echo 'target/bench/summary.txt missing after benchmark attempt'
fi
capture test_link_contract cargo test --manifest-path safe/Cargo.toml --release --test link_contract
if [[ ! -f target/compat/libbz2.so.1.0.4 ]]; then
  capture build_safe_release_for_relink bash safe/scripts/build-safe.sh --release
fi
capture link_original_tests bash safe/scripts/link-original-tests.sh --all
capture cargo_test_release cargo test --manifest-path safe/Cargo.toml --release
```

- `review_checks`:
  - Section 4 must explicitly cover the TODO/FIXME scan result, any observed non-bit-for-bit-equivalent behavior against upstream, the exact scope of passing evidence from `safe/tests/golden_streams.rs`, `safe/tests/original_port.rs`, `safe/tests/link_contract.rs`, `safe/tests/regression_mk251.rs`, and `safe/tests/security_regressions.rs`, and any remaining equivalence gaps that are only covered by sampled fixtures or specific harnesses.
  - Section 4 must use `relevant_cves.json` as the authoritative in-scope CVE set and may use `all_cves.json` only to explain broader upstream CVE scoping or exclusions.
  - If `target/security/summary.txt` exists and section 4 cites consolidated release-gate evidence, the cited claims must match that summary. If it is absent, section 4 must rely on directly captured harness results instead of treating the missing summary as a port issue.
  - If any parity or benchmark command fails, the document must name the harness and summarize the failure.
  - If `target/original-baseline/dlltest.o` was absent at the start of the session but `bash safe/scripts/build-original-baseline.sh` regenerated it successfully, do not report that transient local artifact gap as a remaining port issue. Document it only if regeneration itself fails or remains an operational caveat.
  - Any fresh benchmark evidence cited in section 4 must be grounded in a release-staged compat tree, not merely any populated `target/compat/`.
  - If the benchmark harness cannot produce `target/bench/summary.txt`, the document must say that performance claims are currently unsupported rather than inventing a regression story.
  - A captured nonzero exit from one of the commands above is only a checker failure when `safe/PORT.md` omits or misstates it.

### `check_port_md_packaging_and_downstream_evidence`
- `phase_id`: `check_port_md_packaging_and_downstream_evidence`
- `type`: `check`
- `bounce_target`: `impl_document_libbz2_port`
- `purpose`: verify Debian packaging, autopkgtest and downstream coverage, and dependent caveats by consuming existing package artifacts first and using project harnesses only when fresher evidence is necessary
- `commands`:

```bash
capture() {
  local name="$1"
  shift
  echo ">>> $name"
  if "$@"; then
    echo "__STATUS__ $name 0"
  else
    local status=$?
    echo "__STATUS__ $name $status"
  fi
}

if [[ ! -f target/package/out/package-manifest.txt ]]; then
  capture build_debs bash safe/scripts/build-debs.sh
fi

if [[ -f target/package/out/package-manifest.txt ]]; then
  capture check_package_layout bash safe/scripts/check-package-layout.sh
  capture run_debian_autopkgtests bash safe/scripts/run-debian-tests.sh --tests link-with-shared bigfile bzexe-test compare compress grep
  capture test_original_all bash test-original.sh
  capture test_original_libapt_pkg bash test-original.sh --only libapt-pkg6.0t64
  capture test_original_bzip2 bash test-original.sh --only bzip2
  capture test_original_libpython bash test-original.sh --only libpython3.12-stdlib
  capture test_original_php_bz2 bash test-original.sh --only php8.3-bz2
else
  echo 'target/package/out/package-manifest.txt missing after build-debs attempt'
fi
```

- `review_checks`:
  - Section 4 must distinguish between repository-declared coverage from `safe/debian/tests/control`, `safe/tests/dependents.rs`, and `dependents.json` versus fresh runtime and package evidence.
  - If Docker-backed package or downstream harnesses do not run successfully, the document must say that explicitly and then limit itself to describing the available harnesses and curated dependents rather than claiming fresh pass or fail status.
  - Downstream caveats must come from `dependents.json`, `safe/tests/dependents.rs`, and any actual rerun results.
  - A captured nonzero exit from one of the commands above is only a checker failure when `safe/PORT.md` omits or misstates it.

### `check_port_md_final_sanity`
- `phase_id`: `check_port_md_final_sanity`
- `type`: `check`
- `bounce_target`: `impl_document_libbz2_port`
- `purpose`: verify section ordering, path and symbol validity, dependency grounding, unsafe completeness, and final commit shape
- `commands`:

```bash
test -f safe/PORT.md
rg -n '^1\. \*\*High-level architecture\.\*\*$|^2\. \*\*Where the unsafe Rust lives\.\*\*$|^3\. \*\*Remaining unsafe FFI beyond the original ABI/API boundary\.\*\*$|^4\. \*\*Remaining issues\.\*\*$|^5\. \*\*Dependencies and other libraries used\.\*\*$|^6\. \*\*How this document was produced\.\*\*$' safe/PORT.md
grep -RIn '\bunsafe\b' safe
git show --stat --name-only HEAD
```

- `review_checks`:
  - Every file path mentioned in `safe/PORT.md` must exist at the current commit.
  - Every named symbol mentioned in `safe/PORT.md` must be findable with `rg -n '<symbol>' safe original`.
  - Every dependency named in section 5 must appear in `safe/Cargo.toml`, `safe/debian/control`, `safe/debian/rules`, or the cited scripts.
  - The final commit message must summarize the documentation pass, and the commit must contain `safe/PORT.md` plus only any explicitly justified incidental fixes.

## Success Criteria
- `safe/PORT.md` exists or is updated in place and uses the six required section headings in the exact required order.
- Section 1 grounds architecture, ABI wiring, packaging, relink flow, and the directory map in the cited repository files.
- Section 2 is a file:line unsafe inventory that reconciles against the current tree and `safe/docs/unsafe-audit.md`.
- Section 3 lists only true runtime FFI surfaces beyond the original libbz2 boundary and keeps crate-internal exports out of that inventory.
- Section 4 reports observed parity, regression, CVE, benchmark, package, downstream, and TODO/FIXME evidence without overclaiming or inventing missing optional artifacts as port issues.
- Section 5 states the zero-direct-Rust-dependency result and separates runtime C surfaces, host tools, and Debian packaging dependencies.
- Section 6 records the actual commands run and the key files consulted.
- Any required incidental fix is narrowly scoped, justified, and included in the same final commit as the documentation update.

## Git Commit Requirement
The implementer must commit all work for this phase to git before yielding. The commit message must summarize the documentation pass, for example `docs: document the libbz2 Rust port`.
