use bz2::bz_stream;
use bz2::constants::{
    BZ_DATA_ERROR, BZ_DATA_ERROR_MAGIC, BZ_OK, BZ_OUTBUFF_FULL, BZ_STREAM_END, BZ_UNEXPECTED_EOF,
};
use bz2::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use bz2::ffi::BZ2_bzBuffToBuffDecompress;
use std::mem::MaybeUninit;
use std::os::raw::{c_char, c_int};

const SAMPLE1_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample1.bz2"
));
const SAMPLE1_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample1.ref"
));
const SAMPLE2_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample2.bz2"
));
const SAMPLE2_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample2.ref"
));
const SAMPLE3_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample3.bz2"
));
const SAMPLE3_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample3.ref"
));

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
}

fn terminal_stream_code(input: &[u8], output_cap: usize, small: c_int, step_limit: usize) -> c_int {
    let mut strm = zeroed_stream();
    let mut output = vec![0u8; output_cap];
    let mut source_off = 0usize;
    let mut output_off = 0usize;
    let mut steps = 0usize;

    unsafe {
        assert_eq!(BZ2_bzDecompressInit(&mut strm, 0, small), BZ_OK);
        loop {
            assert!(
                steps < step_limit,
                "CVE-2005-1260 regression: malformed decode exceeded step limit"
            );

            if strm.avail_in == 0 && source_off < input.len() {
                let chunk = (input.len() - source_off).min(73);
                strm.next_in = input.as_ptr().add(source_off).cast_mut().cast::<c_char>();
                strm.avail_in = chunk as u32;
                source_off += chunk;
            }

            let out_chunk = (output.len() - output_off).min(113);
            if out_chunk > 0 {
                strm.next_out = output.as_mut_ptr().add(output_off).cast::<c_char>();
            }
            strm.avail_out = out_chunk as u32;

            let ret = BZ2_bzDecompress(&mut strm);
            output_off += out_chunk - strm.avail_out as usize;
            if ret != BZ_OK {
                let _ = BZ2_bzDecompressEnd(&mut strm);
                return ret;
            }
            if out_chunk == 0 {
                let _ = BZ2_bzDecompressEnd(&mut strm);
                return BZ_OUTBUFF_FULL;
            }
            if source_off == input.len() && strm.avail_in == 0 {
                let code = if strm.avail_out > 0 {
                    BZ_UNEXPECTED_EOF
                } else {
                    BZ_OUTBUFF_FULL
                };
                let _ = BZ2_bzDecompressEnd(&mut strm);
                return code;
            }

            steps += 1;
        }
    }
}

fn helper_decompress_code(input: &[u8], output_cap: usize) -> c_int {
    let mut output = vec![0u8; output_cap];
    let mut dest_len = output_cap as u32;
    unsafe {
        BZ2_bzBuffToBuffDecompress(
            output.as_mut_ptr().cast::<c_char>(),
            &mut dest_len,
            input.as_ptr().cast_mut().cast::<c_char>(),
            input.len() as u32,
            0,
            0,
        )
    }
}

fn is_terminal_decode_code(code: c_int) -> bool {
    matches!(
        code,
        BZ_STREAM_END | BZ_DATA_ERROR | BZ_DATA_ERROR_MAGIC | BZ_UNEXPECTED_EOF | BZ_OUTBUFF_FULL
    )
}

#[test]
fn malformed_headers_and_truncation_return_distinct_errors() {
    let mut bad_magic = SAMPLE3_BZ2.to_vec();
    bad_magic[0] ^= 0x01;
    assert_eq!(
        helper_decompress_code(&bad_magic, SAMPLE3_REF.len() * 2),
        BZ_DATA_ERROR_MAGIC
    );

    let mut bad_structure = SAMPLE3_BZ2.to_vec();
    bad_structure[10] ^= 0x80;
    assert_eq!(
        helper_decompress_code(&bad_structure, SAMPLE3_REF.len() * 2),
        BZ_DATA_ERROR
    );

    assert_eq!(
        helper_decompress_code(&SAMPLE3_BZ2[..SAMPLE3_BZ2.len() - 1], SAMPLE3_REF.len() * 2),
        BZ_UNEXPECTED_EOF
    );

    assert_eq!(helper_decompress_code(SAMPLE3_BZ2, 32), BZ_OUTBUFF_FULL);
}

#[test]
fn exhaustive_bit_flips_on_small_fixture_terminate() {
    // CVE-2005-1260: every one-bit corruption of a very small stream must terminate.
    for bit in 0..(SAMPLE3_BZ2.len() * 8) {
        let mut corrupted = SAMPLE3_BZ2.to_vec();
        corrupted[bit / 8] ^= 1 << (bit % 8);
        let code = terminal_stream_code(&corrupted, SAMPLE3_REF.len() * 2, 0, 10_000);
        assert!(
            is_terminal_decode_code(code),
            "unexpected code {code} at bit {bit}"
        );
    }
}

#[test]
fn bounded_corruption_sweeps_cover_large_samples_without_panics() {
    // CVE-2008-1372 / CVE-2019-12900: malformed selectors, origPtr values, inverse-BWT state,
    // and checked arithmetic paths must end in decode errors instead of memory corruption.
    for &(name, compressed, expected) in &[
        ("sample1", SAMPLE1_BZ2, SAMPLE1_REF),
        ("sample2", SAMPLE2_BZ2, SAMPLE2_REF),
        ("sample3", SAMPLE3_BZ2, SAMPLE3_REF),
    ] {
        let stride = (compressed.len() / 32).max(1);
        for idx in (0..compressed.len()).step_by(stride).take(48) {
            for bit in [0u8, 2, 5, 7] {
                let mut corrupted = compressed.to_vec();
                corrupted[idx] ^= 1 << bit;
                let code = terminal_stream_code(&corrupted, expected.len() * 2, 1, 20_000);
                assert!(
                    is_terminal_decode_code(code),
                    "{name} mutation at byte {idx} bit {bit} produced unexpected code {code}"
                );
            }
        }
    }
}
