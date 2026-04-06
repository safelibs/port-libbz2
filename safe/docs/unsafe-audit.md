# Unsafe Audit

Release-gate refresh for `impl_06_final_hardening_and_release_gate` on 2026-04-06:

- revalidated the current tree with `rg -n '\bunsafe\b' safe/src safe/tests`
- reran the full release gate, including the direct `dlltest.o` relink-and-execute step, Debian autopkgtests, and the full downstream matrix
- confirmed that no new `unsafe` buckets were introduced in this final hardening pass

This phase keeps `unsafe` in four buckets only:

1. C ABI boundary
2. Custom allocator callback bridge
3. `FILE*` and fd wrapper interop
4. Raw pointer reconstruction for the opaque state handle

## C ABI boundary

- `safe/src/compress.rs`, `safe/src/decompress.rs`, `safe/src/huffman.rs`, `safe/src/ffi.rs`, and `safe/src/stdio.rs` export the original `libbz2` C ABI as `pub unsafe extern "C" fn`.
- These functions remain `unsafe` because C callers control nullability, buffer lengths, callback pointers, and lifetime rules that Rust cannot prove.
- `safe/src/crc.rs` and `safe/src/rand.rs` expose the original mutable data symbols. Internal reads from `BZ2_crc32Table` and `BZ2_rNums` stay `unsafe` only because the ABI requires those tables to remain exported as mutable C globals.
- `safe/tests/*` use `unsafe` only to invoke those exported C ABI entry points or to zero-initialize `bz_stream` the way the C contract expects.

## Custom Allocator Callback Bridge

- `safe/src/compress.rs::default_bzalloc/default_bzfree`
- `safe/src/decompress.rs::default_bzalloc/default_bzfree`
- `safe/src/alloc.rs::{ensure_default_allocators, alloc_zeroed_with_bzalloc, alloc_zeroed_slice_with_bzalloc, free_with_bzfree}`

These sites bridge between Rust-owned allocations and the caller-provided `bzalloc` / `bzfree` callbacks from the original ABI. The unsafe operations are limited to:

- calling foreign function pointers supplied by the C caller
- zeroing freshly allocated foreign memory
- returning foreign allocations back through the matching `bzfree`

All size calculations are checked before allocation so allocator misuse becomes a deterministic `BZ_MEM_ERROR` instead of unchecked arithmetic.

## `FILE*` And Fd Wrapper Interop

- `safe/src/stdio.rs`

The stdio wrapper surface is inherently foreign-function interop:

- `fdopen`, `fopen64` / `fopen`, `fclose`, `fflush`, `fread`, `fwrite`, `ferror`, `fgetc`, `ungetc`
- ownership decisions around inherited `stdin` / `stdout`
- boxing and unboxing the opaque `BZFILE`-style handle that the C wrapper API returns

The unsafe blocks here are justified because Rust cannot model `FILE*` ownership or libc stream state directly. The code keeps the unsafety narrow by:

- rejecting invalid wrapper parameters up front
- storing wrapper state in a boxed Rust struct
- closing only handles the wrapper actually owns
- copying unused trailer bytes into fixed-size internal buffers before decode begins

## Raw Pointer Reconstruction For The Opaque State Handle

- `safe/src/types.rs::{stream_state, bzfile_from_handle}`
- `safe/src/compress.rs`
- `safe/src/decompress.rs`
- `safe/src/blocksort.rs`

The core compressor and decompressor preserve the original ABI layout by hanging opaque state off `bz_stream.state`. The remaining unsafe code reconstructs Rust references and slices from that opaque storage:

- `EState` aliases over `arr1`, `arr2`, `ptr`, `block`, `mtfv`, and `zbits`
- `DState` aliases over `tt`, `ll16`, `ll4`, and saved Huffman-group pointers
- blocksort work arrays derived from the `EState` storage owned by the stream

This is the one place where raw pointers are still unavoidable, but the code immediately converts them into checked Rust views and then keeps all format-derived indexing behind explicit validation:

- capacity math uses `checked_mul`, `checked_add`, and integer conversions that return errors on overflow
- slice access uses `get` / `get_mut` or prevalidated indices
- impossible state transitions return `BZ_DATA_ERROR` instead of relying on undefined behavior
- the CVE regressions in `safe/tests/security_regressions.rs` keep the checked-arithmetic and checked-indexing paths visible

## Summary

There is no remaining `unsafe` in shell scripts, and the source-side unsafety is limited to ABI glue plus reconstruction of C-visible opaque state. All algorithmic memory safety now comes from checked arithmetic, checked indexing, and safe container access in the Rust implementation.
