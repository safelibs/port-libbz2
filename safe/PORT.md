1. **High-level architecture.**

`safe/` is a single-package Rust crate named `libbz2-safe` with one workspace member, zero direct Rust dependencies, and direct Cargo outputs rooted at `safe/target/`; `cargo metadata --format-version 1 --no-deps --manifest-path safe/Cargo.toml` and `cargo tree --manifest-path safe/Cargo.toml` both confirm that the crate stands alone (`safe/Cargo.toml:1-14`). The library target is published as `cdylib`, `staticlib`, and `rlib` under the C-facing crate name `bz2` (`safe/Cargo.toml:9-11`): a direct release build lands at `safe/target/release/libbz2.so`, while repo-root `target/` is reserved for staged baseline, compat, package, benchmark, and optional security-harness artifacts.

The module graph in `safe/src/lib.rs:5-19` preserves the upstream split between algorithm modules and ABI glue. `safe/src/types.rs:19-171` defines the ABI-visible layouts for `bz_stream`, the opaque compressor state `EState`, the opaque decompressor state `DState`, and the wrapper handle state `BzFileState`. The public C ABI entry points live in four places:

- `safe/src/compress.rs:740-955` exports the low-level block writer plus `BZ2_bzCompress*`.
- `safe/src/decompress.rs:1064-1209` exports `BZ2_bzDecompress*`, `BZ2_indexIntoF`, and `BZ2_decompress`.
- `safe/src/ffi.rs:58-176` exports `BZ2_bzlibVersion`, the buffer-to-buffer wrappers, and `BZ2_bz__AssertH__fail`.
- `safe/src/stdio.rs:181-633` exports the `BZ2_bzRead*`, `BZ2_bzWrite*`, `BZ2_bzopen`, `BZ2_bzdopen`, `BZ2_bzread`, `BZ2_bzwrite`, `BZ2_bzflush`, `BZ2_bzclose`, and `BZ2_bzerror` wrapper surface.

The data flow still mirrors upstream `libbz2`. A caller fills `bz_stream` and hands it to `BZ2_bzCompressInit` or `BZ2_bzDecompressInit`; those functions allocate an `EState` or `DState`, store the typed pointer behind `bz_stream.state`, and attach the original stream back-pointer (`safe/src/compress.rs:797-877`, `safe/src/decompress.rs:1064-1095`). Compression then operates on `EState` work arrays aliased through `arr1`, `arr2`, `ptr`, `block`, `mtfv`, and `zbits` (`safe/src/types.rs:37-80`, `safe/src/compress.rs:838-876`); decompression similarly reconstructs `tt`, `ll16`, `ll4`, and saved Huffman-table pointers from `DState` (`safe/src/types.rs:82-151`, `safe/src/decompress.rs:213-260`, `safe/src/decompress.rs:657-950`). The stdio wrappers box a `BzFileState`, hide it behind the historical `BZFILE*`-style `void *` handle, and route `FILE*` or fd-backed streams back into the same core `bz_stream` engine (`safe/src/types.rs:153-171`, `safe/src/stdio.rs:40-179`).

Build and ABI wiring stays explicit. `safe/build.rs:4-39` sets the Linux SONAME to `libbz2.so.1.0` and applies `safe/abi/libbz2.map` as the version script unless `LIBBZ2_SKIP_VERSION_SCRIPT` is set; the same build script points Windows builds at `safe/abi/libbz2.def`. The captured upstream ABI baseline under `safe/abi/original.exports.txt:1-35` and `safe/abi/original.soname.txt:1-3` expects 35 exported `BZ2_*` symbols and the real shared object name `libbz2.so.1.0.4`. The direct Cargo release artifact is `safe/target/release/libbz2.so`; the separately staged compat artifact that the relink and ABI harnesses consume is `target/compat/libbz2.so.1.0.4`. `bash safe/scripts/check-abi.sh --strict` exited 0 against the current tree. `safe/scripts/build-safe.sh:57-100` also refuses to stage the safe library unless `safe/include/bzlib.h` is byte-for-byte identical to `original/bzlib.h`; `cmp -s safe/include/bzlib.h original/bzlib.h` succeeded during this refresh.

Debian packaging keeps the port boundary narrow. `safe/debian/rules:15-41` builds the Rust library into a staged install root, installs `libbz2.so.1.0.4`, `libbz2.a`, and `bzlib.h`, and then calls `safe/scripts/build-original-cli-against-safe.sh` so the package still ships the original `bzip2` and `bzip2recover` frontends relinked against the safe shared object. The same split appears in the release-gate relink harnesses: `safe/scripts/build-safe.sh:96-100` relinks captured upstream objects from `target/original-baseline/`, and `safe/scripts/link-original-tests.sh:100-237` recompiles or relinks the original `public_api_test.c`, `dlltest.c`, and `bzip2.o` fixtures against `target/compat/libbz2.so.1.0.4`.

Directory map for the checked-in workflow:

- `safe/src/`: Rust implementation, ABI structs, tables, wrappers, and allocator glue.
- `safe/tests/`: release tests for ABI layout, parity, relink behavior, malformed inputs, regressions, CVEs, and the downstream-matrix contract.
- `safe/abi/`: Linux version script, Windows `.def`, and captured upstream export / SONAME / undefined-symbol baselines.
- `safe/include/`: staged public header; must remain identical to `original/bzlib.h`.
- `safe/docs/`: prior audit notes such as `unsafe-audit.md`.
- `safe/debian/`: Debian control, rules, install manifests, doc metadata, and autopkgtests.
- `safe/scripts/`: staging, ABI, relink, packaging, autopkgtest, benchmark, and aggregate release-gate harnesses.
- `original/`: upstream C sources, headers, fixtures, CLI programs, and Debian packaging inputs used for comparison.
- `target/original-baseline/`: captured upstream `libbz2.so.1.0.4`, relink fixtures, sample outputs, and prerequisite objects `public_api_test.o`, `bzip2.o`, and `dlltest.o`.

2. **Where the unsafe Rust lives.**

This refresh revalidated the full-tree inventory with `rg -n '\bunsafe\b' safe -g '!safe/target/**' -g '!target/**'` and focused source scans over `safe/src` and `safe/tests`. The broad themes from `safe/docs/unsafe-audit.md:9-76` are still right, but the note is stale in one important way: the current tree has additional internal helper sites in `safe/src/alloc.rs`, `safe/src/blocksort.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/ffi.rs`, `safe/src/types.rs`, and the inline assertion calls in `safe/src/huffman.rs`, so the live tree is not just the four coarse buckets named in that older note.

Exported ABI entry points and mutable ABI globals:

- `safe/src/compress.rs:740,749,797,881,943` expose the upstream low-level compressor and `BZ2_bzCompress*` entry points, so they are `unsafe` because C callers control pointer validity, buffer lengths, and action sequencing.
- `safe/src/blocksort.rs:916` exports `BZ2_blockSort`, which keeps the original low-level blocksort helper callable from C and therefore remains an exported ABI entry point rather than a purely internal helper.
- `safe/src/decompress.rs:1064,1099,1169,1189,1209` expose `BZ2_bzDecompress*`, `BZ2_indexIntoF`, and `BZ2_decompress` over caller-owned `bz_stream` and `DState` pointers.
- `safe/src/ffi.rs:65,115` expose the buffer-to-buffer wrappers; `safe/src/ffi.rs:164` is the internal `unsafe` block inside `BZ2_bz__AssertH__fail` that preserves the original stderr text and exit code.
- `safe/src/huffman.rs:206,227,271` keep the original exported Huffman helper surface callable from C.
- `safe/src/stdio.rs:182,233,251,324,350,397,448,467,552,557,562,580,594,599,621` preserve the historical `BZ2_bzRead*`, `BZ2_bzWrite*`, and `BZFILE*` wrapper API over raw `FILE*` state.
- `safe/src/crc.rs:5,48` and `safe/src/rand.rs:5,53` keep `BZ2_crc32Table` and `BZ2_rNums` as mutable ABI data symbols and read them with `unsafe` because the exported ABI requires `static mut`.

C out-parameter and error-slot helpers:

- `safe/src/ffi.rs:40` writes to caller-supplied `int *` error slots after a null check; it is a real unsafe helper because the pointed-to storage is owned and typed by C callers, not Rust.

Allocator callback bridging through `bzalloc` and `bzfree`:

- `safe/src/types.rs:16,17` declare the C callback signatures as `unsafe extern "C"` function pointers because the library cannot prove anything about the caller's allocator behavior.
- `safe/src/alloc.rs:8,12,18,35,49,58,62` zero raw boxes, drop opaque boxes, reset totals through raw stream pointers, install default callbacks, call `bzfree`, and request zeroed slices through `bzalloc` after checked size arithmetic.
- `safe/src/compress.rs:40,60` and `safe/src/decompress.rs:174,191` provide the default `malloc` / `free` fallback when the caller leaves `bzalloc` or `bzfree` unset.

`FILE*` and fd wrapper interop:

- `safe/src/stdio.rs:40,44,48,55,67,78,87,91,113,124` zero a `BzFileState`, recover it from the opaque handle, write back error codes, decide wrapper ownership for `stdin` / `stdout`, probe `FILE*` EOF state, parse C mode strings, open paths through `fopen64` / `fopen`, and translate `path` or `fd` inputs into `BZ2_bzReadOpen` / `BZ2_bzWriteOpen`.

Raw-pointer reconstruction from `bz_stream.state` and `BZFILE`-style handles:

- `safe/src/types.rs:165,170` cast opaque `void *` handles back to `BzFileState` and typed stream state.

Unsafe code that is not strictly required by the exported ABI boundary:

- `safe/src/compress.rs:34,77,82,87,92,103,120,125,141,147,176,185,190,194,225,232,254,258,297,327,366,376,404,458,468,475,479,487` is post-handoff internal unsafe: it reconstructs slices over `EState` work buffers, preserves the upstream internal assertion path, and mutates compressor state that has already been validated and hung off `bz_stream.state`.
- `safe/src/blocksort.rs:24,63,116,123,130,137,146,180,290,422,488,547,700` does the same for blocksort storage layered over `EState.arr1` / `arr2`; this is internal pointer math, not a new foreign boundary.
- `safe/src/decompress.rs:213,235,266,274,282,303,318,332,346,353,360,376,389,444,510,565,657,671,696,701,711,738,742,746,764,786,857,930,950,1059` reconstructs `DState` work arrays, Huffman-group pointers, and bitstream views after the ABI handoff and keeps overflow / impossible-state handling explicit.
- `safe/src/huffman.rs:83,108` calls `BZ2_bz__AssertH__fail` from checked table builders to preserve upstream internal-assert semantics.

Test-only unsafe for calling the C ABI or zero-initializing `bz_stream`:

- `safe/tests/abi_contract.rs:49,73,86,215,216,233,246,268,309,346,350` uses custom allocator callbacks, zeroed `bz_stream`, raw symbol reads, and direct C ABI calls to verify layout, exported tables, wrapper edge cases, and version behavior.
- `safe/tests/abi_contract.rs:166` is intentionally not counted as a real unsafe site because `size_of::<unsafe extern "C" fn()>()` is a non-executing type reference, not a call, block, or definition that performs unsafe work.
- `safe/tests/compression_port.rs:23,48,93,123,129,139,140,157,181,184,241,245,289` zero-initializes streams and drives the C compression API plus write-side `FILE*` / wrapper entry points.
- `safe/tests/decompress_port.rs:56,66,99,126,133,139,152,160,166,179,230,233,339,344,359,369,371,374,393,406,409` does the same for decompression, concatenated-member handling, `BZ2_bzReadGetUnused`, truncated-stream behavior, and `fdopen` interop.
- `safe/tests/golden_streams.rs:48,72,115,135,138,175,181,192,193` calls the C API and wrappers to prove bit-for-bit output against `original/sample1.bz2`, `original/sample2.bz2`, and `original/sample3.bz2`.
- `safe/tests/malformed_inputs.rs:21`, `safe/tests/regression_mk251.rs:23,56,98`, and `safe/tests/security_regressions.rs:40,46,65` zero streams and call the ABI directly so malformed-input and CVE regressions stay visible in release tests.
- `safe/tests/original_port.rs:36,61,110,154,200,225,240,255,269,293,362,399,457,481,499,507,517,526,532` exercises the public API contract directly, including returned pointers, `BZFILE` wrappers, and concatenated-stream trailer handling.

No additional real `unsafe` sites were found in `safe/tests/dependents.rs` or `safe/tests/link_contract.rs`, and no source-side false positives survived the focused scan once docs and generated output were excluded.

3. **Remaining unsafe FFI beyond the original ABI/API boundary.**

The port does not add new foreign libraries beyond the system C runtime, but it still imports a small set of libc / CRT entry points that are outside the original exported `BZ2_*` surface:

- `safe/src/compress.rs:27-31` imports `malloc` and `free` from libc for the default allocator path, plus the crate-internal `BZ2_bz__AssertH__fail`. Only `malloc` / `free` count as extra foreign surface; they are needed so `BZ2_bzCompressInit` can behave like upstream when callers leave `bzalloc` and `bzfree` null. A future safer replacement would be routing the default path through the Rust global allocator while preserving C ABI ownership and null-on-failure semantics.
- `safe/src/decompress.rs:28-31` imports `malloc` and `free` for the same reason on the decompressor side. The long-term replacement is the same allocator unification problem.
- `safe/src/ffi.rs:11-16` imports `fprintf`, `fputs`, `exit`, and the `stderr` global from the C runtime. These are needed only to preserve the exact upstream `BZ2_bz__AssertH__fail` diagnostics and exit code 3 that `safe/tests/abi_contract.rs:353-375` checks. A future Rust-only replacement could use `std::io::stderr` and `std::process::exit`, but that would need to preserve the message format and process-exit behavior byte-for-byte.
- `safe/src/stdio.rs:17-38` imports `fdopen`, `fclose`, `fflush`, `fread`, `fwrite`, `ferror`, `fgetc`, `ungetc`, `stdin`, `stdout`, and platform-specific `fopen64` or `fopen`. These are the real remaining FFI surface for the wrapper API, because `BZ2_bzRead*`, `BZ2_bzWrite*`, `BZ2_bzopen`, and `BZ2_bzdopen` are defined in terms of `FILE*` ownership and libc stream semantics. Replacing them with safer Rust I/O would require an API change away from the historical wrapper surface; it is not possible while keeping the original ABI intact.

Crate-internal `extern "C"` declarations that target symbols exported by the same crate, such as `BZ2_bz__AssertH__fail`, `BZ2_blockSort`, `BZ2_hbAssignCodes`, `BZ2_hbCreateDecodeTables`, `BZ2_hbMakeCodeLengths`, `BZ2_decompress`, and `BZ2_indexIntoF`, are not counted here because they preserve the original symbol graph instead of introducing new external dependencies.

4. **Remaining issues.**

`rg -n '\b(TODO|FIXME|XXX|HACK)\b' safe original -g '!safe/target/**' -g '!target/**'` now matches only `safe/PORT.md` itself where this document reproduces the required scan pattern in section 4 and section 6. Those self-hits are documentation artifacts, not unresolved source markers; aside from `safe/PORT.md`, the scan found no real TODO / FIXME / XXX / HACK markers under `safe/` or `original/`.

Fresh parity, relink, and regression evidence from this refresh is all passing:

- `cargo test --manifest-path safe/Cargo.toml --release --test abi_contract` exited 0.
- `cargo test --manifest-path safe/Cargo.toml --release --test golden_streams` exited 0.
- `cargo test --manifest-path safe/Cargo.toml --release --test original_port` exited 0.
- `cargo test --manifest-path safe/Cargo.toml --release --test regression_mk251` exited 0.
- `cargo test --manifest-path safe/Cargo.toml --release --test malformed_inputs` exited 0.
- `cargo test --manifest-path safe/Cargo.toml --release --test security_regressions` exited 0.
- `cargo test --manifest-path safe/Cargo.toml --release --test link_contract` exited 0.
- `bash safe/scripts/link-original-tests.sh --all` exited 0.
- `bash safe/scripts/check-abi.sh --strict` exited 0.

Observed equivalence scope is therefore strong but still bounded. `safe/tests/golden_streams.rs:200-221` showed bit-for-bit agreement with the upstream `sample1`, `sample2`, and `sample3` fixtures across the stream API, buffer API, stdio wrappers, and `bzopen` wrappers. `safe/tests/original_port.rs:198-552` showed API-parity coverage for version reporting, stream compression/decompression, buffer wrappers, concatenated members, trailer bytes, stdio wrappers, `BZ2_bzflush`, `BZ2_bzerror`, and `BZ2_bzdopen`. `safe/tests/link_contract.rs:124-258` plus `safe/scripts/link-original-tests.sh:219-237` showed that the captured upstream `public_api_test.o`, `bzip2.o`, and `dlltest.o` fixtures still relink and execute against the staged safe shared object. `safe/tests/regression_mk251.rs:176-194` kept the MK251 / blocksort regressions aligned with the upstream `bzip2` binary, and `safe/tests/malformed_inputs.rs:33-102` kept distinct `BZ_DATA_ERROR_MAGIC`, `BZ_DATA_ERROR`, `BZ_UNEXPECTED_EOF`, and `BZ_OUTBUFF_FULL` paths visible. No non-bit-for-bit drift was observed in any rerun harness, but that evidence only covers the exercised fixtures, generated payloads, wrapper paths, and relink scenarios; it is not a proof for every possible `.bz2` stream.

Performance evidence in this refresh is grounded in an explicit release compat restage rather than the profile-ambiguous staged tree left behind by later relink tests. After the parity suite had already exercised `cargo test --manifest-path safe/Cargo.toml --release --test link_contract`, this refresh reran `bash safe/scripts/build-safe.sh --release` immediately before `env LIBBZ2_BENCH_CAPTURE_SECURITY_LOG=0 bash safe/scripts/benchmark-compare.sh` so the benchmark would use a freshly release-staged compat tree. That benchmark command exited 0 and rewrote `target/bench/summary.txt`, after also checking that safe and baseline compression SHA256 outputs matched for every benchmark case. Median compression times were `1.695x` baseline on `textual-16m`, `1.522x` on `mixed-24m`, and `0.889x` on `random-8m`; median decompression times were `1.039x`, `0.999x`, and `1.129x` respectively. That is informative performance evidence, not a release gate.

Packaging evidence is mixed but now current enough to cite:

- `target/package/out/package-manifest.txt` and `target/package/unpacked/*` already existed, so `bash safe/scripts/build-debs.sh` was not rerun in this documentation pass.
- The existing `target/security/07-build-debs.log` shows a successful Debian package build, but it also records `debugedit` warnings about unknown `DW_FORM_0x1f20` on the relinked `bzip2` binaries and `dpkg-shlibdeps` warnings about merged-usr diversions. Those are build-log caveats, not observed package failures.
- `bash safe/scripts/check-package-layout.sh` exited 0 against the current package tree.
- The older `target/security/09-run-debian-tests.log` is incomplete and cannot be treated as proof of a finished autopkgtest pass because it ends during package installation and there is no aggregate release-gate summary file under `target/security/`. To replace that gap, this refresh reran `bash safe/scripts/run-debian-tests.sh --tests link-with-shared bigfile bzexe-test compare compress grep`, and that fresh autopkgtest run exited 0.

Downstream coverage is still the weakest part of the evidence set. `dependents.json` records 13 curated Ubuntu 24.04 dependents: `libapt-pkg6.0t64`, `bzip2`, `libpython3.12-stdlib`, `php8.3-bz2`, `pike8.0-bzip2`, `libcompress-raw-bzip2-perl`, `mariadb-plugin-provider-bzip2`, `gpg`, `zip`, `unzip`, `libarchive13t64`, `libfreetype6`, and `gstreamer1.0-plugins-good`. `cargo test --manifest-path safe/Cargo.toml --release --test dependents` exited 0 and therefore revalidated the curated matrix contents plus the fact that `safe/scripts/run-full-suite.sh` still wires a full downstream `test-original.sh` pass and four representative `--only` smokes. However, no fresh `test-original.sh` container run was performed in this documentation refresh, so none of the 13 curated dependents were re-exercised here; downstream behavior still relies on the checked-in matrix and harness contract rather than a new runtime smoke.

`relevant_cves.json` keeps only two in-scope non-memory-corruption library CVE classes: `CVE-2005-1260` (algorithmic denial of service / infinite-loop handling) and `CVE-2010-0405` (integer overflow / bounds-validation). Both have direct fresh harness coverage in `safe/tests/security_regressions.rs:142-226`, which passed in release mode during this refresh. `all_cves.json` is still useful as the broader upstream dataset because it explains why CLI-only issues such as `CVE-2002-0759`, `CVE-2002-0760`, `CVE-2002-0761`, `CVE-2005-0953`, and `CVE-2011-4089` are excluded from the library-scope analysis, and why memory-corruption CVEs such as `CVE-2008-1372`, `CVE-2016-3189`, and `CVE-2019-12900` are not part of `relevant_cves.json`. In practice, `safe/tests/security_regressions.rs` still exercises `CVE-2008-1372` and `CVE-2019-12900` as regression context, but the remaining reasoning-only caveat is broader hostile-input resource control: the current tests prove termination and checked-arithmetic behavior on targeted corruptions, not a configurable global work or output cap for arbitrary decompression bombs.

5. **Dependencies and other libraries used.**

Direct Rust dependencies:

- `safe/Cargo.toml:1-14`, `cargo metadata --format-version 1 --no-deps --manifest-path safe/Cargo.toml`, and `cargo tree --manifest-path safe/Cargo.toml` all show that `libbz2-safe` has zero direct third-party Rust dependencies. There are therefore no external Rust crates to audit in this port.
- `cargo-geiger` was not available on `PATH` during this refresh, but the missing tool is low-impact here because there are no dependency crates for it to classify.

Runtime C-library surface linked by the crate:

- `malloc` and `free` in `safe/src/compress.rs:27-31` and `safe/src/decompress.rs:28-31`.
- `fprintf`, `fputs`, `exit`, and `stderr` in `safe/src/ffi.rs:11-16`.
- `fdopen`, `fclose`, `fflush`, `fread`, `fwrite`, `ferror`, `fgetc`, `ungetc`, `stdin`, `stdout`, and `fopen64` or `fopen` in `safe/src/stdio.rs:17-38`.
- These are meaningful unsafe surfaces because they cross into native libc / CRT code, but they are acceptable in this port because preserving the original `libbz2` C ABI and `FILE*` wrapper API requires that exact runtime contract.

Host build and verification tools invoked by the project scripts:

- `cargo`, `rustc`, and `gcc` are invoked by `safe/scripts/build-safe.sh:61-100`, `safe/debian/rules:29-41`, and `safe/scripts/build-original-cli-against-safe.sh:74-97`.
- `readelf`, `diff`, `awk`, `grep`, and `cmp` are part of ABI and relink verification in `safe/scripts/check-abi.sh:48-218` and `safe/scripts/link-original-tests.sh:100-237`.
- `docker`, `dpkg-buildpackage`, `dpkg-deb`, `python3`, and `tar` are invoked by `safe/scripts/build-debs.sh:63-187`, `safe/scripts/run-debian-tests.sh:64-234`, and `test-original.sh:42-160`.
- `make`, `docbook2x-texi`, and `makeinfo` are required by `safe/debian/rules:43-59` when the documentation package is built.
- These tools all carry native-code attack surface, but they run only in the host or containerized build/test environment; they are not linked into the shipped Rust crate.

Debian build dependencies and container prerequisites:

- `safe/debian/control:5-9` declares `debhelper-compat (= 13)`, `cargo`, `rustc`, `texinfo`, `docbook-xml`, `docbook2x`, and `xsltproc`.
- `safe/scripts/build-debs.sh:124-145` installs `build-essential`, `ca-certificates`, `cargo`, `debhelper`, `devscripts`, `docbook-xml`, `docbook2x`, `dpkg-dev`, `fakeroot`, `pkg-config`, `rustc`, `texinfo`, and `xsltproc` into the packaging container before calling `dpkg-buildpackage`.
- `safe/scripts/run-debian-tests.sh:193-234` and `safe/debian/tests/control:1-5` add `build-essential`, `@`, and `@builddeps@` for the `link-with-shared`, `bigfile`, `bzexe-test`, `compare`, `compress`, and `grep` autopkgtests.
- `test-original.sh:120-150` installs a larger downstream-smoke container image with `build-essential`, `ca-certificates`, `pkg-config`, language bindings, archive tools, MariaDB, GStreamer, and other package-specific consumers from the curated dependent matrix.

6. **How this document was produced.**

This refresh used the current `safe/` tree, the checked-in comparison inputs under `original/`, the prepared baseline and package artifacts under repo-root `target/`, and the curated downstream / CVE inputs `dependents.json`, `relevant_cves.json`, and `all_cves.json`.

Commands run for this refresh:

- `cargo metadata --format-version 1 --no-deps --manifest-path safe/Cargo.toml`
- `cargo tree --manifest-path safe/Cargo.toml`
- `rg -n '\bunsafe\b' safe -g '!safe/target/**' -g '!target/**'`
- `rg -n '\bunsafe\b' safe/src safe/tests`
- `rg -n '\b(TODO|FIXME|XXX|HACK)\b' safe original -g '!safe/target/**' -g '!target/**'`
- `cmp -s safe/include/bzlib.h original/bzlib.h`
- `test -f safe/target/release/libbz2.so`
- `bash safe/scripts/check-abi.sh --strict`
- `bash safe/scripts/build-safe.sh --release`
- `env LIBBZ2_BENCH_CAPTURE_SECURITY_LOG=0 bash safe/scripts/benchmark-compare.sh`
- `cargo test --manifest-path safe/Cargo.toml --release --test abi_contract`
- `cargo test --manifest-path safe/Cargo.toml --release --test golden_streams`
- `cargo test --manifest-path safe/Cargo.toml --release --test original_port`
- `cargo test --manifest-path safe/Cargo.toml --release --test regression_mk251`
- `cargo test --manifest-path safe/Cargo.toml --release --test malformed_inputs`
- `cargo test --manifest-path safe/Cargo.toml --release --test security_regressions`
- `cargo test --manifest-path safe/Cargo.toml --release --test link_contract`
- `cargo test --manifest-path safe/Cargo.toml --release --test dependents`
- `bash safe/scripts/link-original-tests.sh --all`
- `bash safe/scripts/check-package-layout.sh`
- `bash safe/scripts/run-debian-tests.sh --tests link-with-shared bigfile bzexe-test compare compress grep`
- `command -v cargo-geiger`

Key files consulted while drafting:

- Core crate and ABI surface: `safe/Cargo.toml`, `safe/build.rs`, `safe/src/lib.rs`, `safe/src/types.rs`, `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/ffi.rs`, `safe/src/stdio.rs`, `safe/abi/libbz2.map`, `safe/abi/libbz2.def`, `safe/abi/original.exports.txt`, `safe/abi/original.soname.txt`, `safe/include/bzlib.h`.
- Unsafe reconciliation: `safe/docs/unsafe-audit.md`, every live `unsafe` hit in `safe/src/*.rs` and `safe/tests/*.rs`.
- Parity and regression evidence: `safe/tests/abi_contract.rs`, `safe/tests/original_port.rs`, `safe/tests/golden_streams.rs`, `safe/tests/link_contract.rs`, `safe/tests/regression_mk251.rs`, `safe/tests/malformed_inputs.rs`, `safe/tests/security_regressions.rs`, and `safe/scripts/link-original-tests.sh`.
- Packaging and downstream evidence: `safe/debian/control`, `safe/debian/rules`, `safe/debian/tests/control`, `safe/scripts/build-debs.sh`, `safe/scripts/check-package-layout.sh`, `safe/scripts/run-debian-tests.sh`, `safe/scripts/run-full-suite.sh`, `test-original.sh`, `target/package/out/package-manifest.txt`, `target/package/unpacked/*`, `target/security/07-build-debs.log`, `target/security/09-run-debian-tests.log`, and `dependents.json`.
- CVE scope: `relevant_cves.json` as the authoritative in-scope list and `all_cves.json` as the broader upstream dataset explaining why other CVEs are excluded.
