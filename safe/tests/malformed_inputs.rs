use bz2::constants::{BZ_DATA_ERROR, BZ_DATA_ERROR_MAGIC, BZ_OUTBUFF_FULL, BZ_UNEXPECTED_EOF};
use bz2::ffi::BZ2_bzBuffToBuffDecompress;
use std::os::raw::{c_char, c_int};

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

#[test]
fn malformed_headers_and_truncation_return_distinct_errors() {
    let mut bad_magic = SAMPLE3_BZ2.to_vec();
    bad_magic[0] ^= 0x01;
    assert_eq!(
        helper_decompress_code(&bad_magic, SAMPLE3_REF.len() * 2, 0),
        BZ_DATA_ERROR_MAGIC
    );
    assert_eq!(
        helper_decompress_code(&bad_magic, SAMPLE3_REF.len() * 2, 1),
        BZ_DATA_ERROR_MAGIC
    );

    let mut bad_structure = SAMPLE3_BZ2.to_vec();
    bad_structure[10] ^= 0x80;
    assert_eq!(
        helper_decompress_code(&bad_structure, SAMPLE3_REF.len() * 2, 0),
        BZ_DATA_ERROR
    );
    assert_eq!(
        helper_decompress_code(&bad_structure, SAMPLE3_REF.len() * 2, 1),
        BZ_DATA_ERROR
    );

    assert_eq!(
        helper_decompress_code(
            &SAMPLE3_BZ2[..SAMPLE3_BZ2.len() - 1],
            SAMPLE3_REF.len() * 2,
            0
        ),
        BZ_UNEXPECTED_EOF
    );
    assert_eq!(
        helper_decompress_code(
            &SAMPLE3_BZ2[..SAMPLE3_BZ2.len() - 1],
            SAMPLE3_REF.len() * 2,
            1
        ),
        BZ_UNEXPECTED_EOF
    );

    assert_eq!(helper_decompress_code(SAMPLE3_BZ2, 32, 0), BZ_OUTBUFF_FULL);
    assert_eq!(helper_decompress_code(SAMPLE3_BZ2, 32, 1), BZ_OUTBUFF_FULL);

    assert_eq!(
        helper_decompress_code(
            &SAMPLE3_RANDOMIZED_BZ2[..SAMPLE3_RANDOMIZED_BZ2.len() - 1],
            SAMPLE3_REF.len() * 2,
            0
        ),
        BZ_UNEXPECTED_EOF
    );
    assert_eq!(
        helper_decompress_code(
            &SAMPLE3_RANDOMIZED_BZ2[..SAMPLE3_RANDOMIZED_BZ2.len() - 1],
            SAMPLE3_REF.len() * 2,
            1
        ),
        BZ_UNEXPECTED_EOF
    );

    assert_eq!(
        helper_decompress_code(SAMPLE3_RANDOMIZED_BZ2, 32, 0),
        BZ_OUTBUFF_FULL
    );
    assert_eq!(
        helper_decompress_code(SAMPLE3_RANDOMIZED_BZ2, 32, 1),
        BZ_OUTBUFF_FULL
    );
}
