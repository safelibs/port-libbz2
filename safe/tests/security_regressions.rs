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
const SAMPLE3_RANDOMIZED_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/sample3.randomized.bz2"
));
const SAMPLE3_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample3.ref"
));

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
}

fn helper_decompress_code(input: &[u8], output_cap: usize, small: c_int) -> c_int {
    let mut output = vec![0u8; output_cap];
    let mut dest_len = output_cap as u32;
    unsafe {
        BZ2_bzBuffToBuffDecompress(
            output.as_mut_ptr().cast::<c_char>(),
            &mut dest_len,
            input.as_ptr().cast_mut().cast::<c_char>(),
            input.len() as u32,
            small,
            0,
        )
    }
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

fn is_terminal_decode_code(code: c_int) -> bool {
    matches!(
        code,
        BZ_STREAM_END | BZ_DATA_ERROR | BZ_DATA_ERROR_MAGIC | BZ_UNEXPECTED_EOF | BZ_OUTBUFF_FULL
    )
}

fn overwrite_bits_msb_first(buffer: &mut [u8], start_bit: usize, width: usize, value: u32) {
    assert!(width <= 32);
    for bit_index in 0..width {
        let absolute_bit = start_bit + bit_index;
        let byte_index = absolute_bit / 8;
        let bit_in_byte = 7 - (absolute_bit % 8);
        let source_bit = (value >> (width - 1 - bit_index)) & 1;
        if source_bit == 1 {
            buffer[byte_index] |= 1 << bit_in_byte;
        } else {
            buffer[byte_index] &= !(1 << bit_in_byte);
        }
    }
}

fn sample_fixtures() -> [(&'static str, &'static [u8], &'static [u8]); 4] {
    [
        ("sample1", SAMPLE1_BZ2, SAMPLE1_REF),
        ("sample2", SAMPLE2_BZ2, SAMPLE2_REF),
        ("sample3", SAMPLE3_BZ2, SAMPLE3_REF),
        ("sample3-randomized", SAMPLE3_RANDOMIZED_BZ2, SAMPLE3_REF),
    ]
}

#[test]
fn cve_2005_1260_loop_and_progress_termination_for_small_stream_bitflips() {
    // CVE-2005-1260: keep loop and progress termination explicit by forcing every one-bit
    // corruption of a tiny stream to either consume input, emit bounded output, or terminate.
    for &small in &[0, 1] {
        for bit in 0..(SAMPLE3_BZ2.len() * 8) {
            let mut corrupted = SAMPLE3_BZ2.to_vec();
            corrupted[bit / 8] ^= 1 << (bit % 8);
            let code = terminal_stream_code(&corrupted, SAMPLE3_REF.len() * 2, small, 10_000);
            assert!(
                is_terminal_decode_code(code),
                "small={small} bit {bit} produced unexpected code {code}"
            );
        }
    }
}

#[test]
fn cve_2010_0405_checked_arithmetic_plus_impossible_state_rejects_invalid_origptr() {
    // CVE-2010-0405: origPtr is a 24-bit block-local index. Forcing it beyond the decoded
    // block length must fail as checked arithmetic plus impossible-state rejection, not crash.
    let mut corrupted = SAMPLE3_BZ2.to_vec();
    overwrite_bits_msb_first(&mut corrupted, 32 + 48 + 32 + 1, 24, 0x00ff_ffff);

    assert_eq!(
        helper_decompress_code(&corrupted, SAMPLE3_REF.len() * 2, 0),
        BZ_DATA_ERROR
    );
    assert_eq!(
        helper_decompress_code(&corrupted, SAMPLE3_REF.len() * 2, 1),
        BZ_DATA_ERROR
    );
}

#[test]
fn cve_2010_0405_checked_arithmetic_paths_terminate_on_bounded_corruptions() {
    // CVE-2010-0405: checked arithmetic around selector counts, RUNA/RUNB growth, and
    // inverse-BWT bookkeeping must still end in decode errors instead of overflows or loops.
    for &(name, compressed, expected) in &sample_fixtures() {
        let stride = (compressed.len() / 32).max(1);
        for idx in (0..compressed.len()).step_by(stride).take(48) {
            for bit in [0u8, 2, 5, 7] {
                let mut corrupted = compressed.to_vec();
                corrupted[idx] ^= 1 << bit;
                for &small in &[0, 1] {
                    let code = terminal_stream_code(&corrupted, expected.len() * 2, small, 20_000);
                    assert!(
                        is_terminal_decode_code(code),
                        "{name} small={small} mutation at byte {idx} bit {bit} produced unexpected code {code}"
                    );
                }
            }
        }
    }
}

#[test]
fn cve_2008_1372_and_cve_2019_12900_checked_indexing_and_safe_container_access_reject_corruptions()
{
    // CVE-2008-1372 / CVE-2019-12900: checked indexing and safe container access now replace the
    // historical over-read/out-of-bounds write classes in selector decoding and BWT rebuild paths.
    for &(name, compressed, expected) in &sample_fixtures() {
        let mut interesting_offsets = vec![
            0usize,
            compressed.len() / 8,
            compressed.len() / 4,
            compressed.len() / 2,
            compressed.len().saturating_sub(8),
            compressed.len().saturating_sub(1),
        ];
        interesting_offsets.sort_unstable();
        interesting_offsets.dedup();

        for idx in interesting_offsets {
            for bit in [1u8, 3, 6] {
                let mut corrupted = compressed.to_vec();
                corrupted[idx] ^= 1 << bit;
                let code = terminal_stream_code(&corrupted, expected.len() * 2, 1, 20_000);
                assert!(
                    is_terminal_decode_code(code),
                    "{name} checked-indexing mutation at byte {idx} bit {bit} produced unexpected code {code}"
                );
            }
        }
    }
}
