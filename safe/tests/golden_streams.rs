use bz2::bz_stream;
use bz2::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use bz2::constants::{BZ_FINISH, BZ_FINISH_OK, BZ_OK, BZ_RUN, BZ_RUN_OK, BZ_STREAM_END};
use bz2::ffi::BZ2_bzBuffToBuffCompress;
use std::mem::MaybeUninit;
use std::os::raw::c_char;

const SAMPLE1_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample1.ref"
));
const SAMPLE1_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample1.bz2"
));
const SAMPLE2_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample2.ref"
));
const SAMPLE2_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample2.bz2"
));
const SAMPLE3_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample3.ref"
));
const SAMPLE3_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample3.bz2"
));

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
}

fn compress_bound(source_len: usize) -> usize {
    source_len + (source_len / 100) + 601
}

fn compress_via_stream(source: &[u8], block_size_100k: i32) -> Vec<u8> {
    let mut strm = zeroed_stream();
    let mut dest = vec![0u8; compress_bound(source.len())];
    let mut source_off = 0usize;
    let mut dest_off = 0usize;

    unsafe {
        assert_eq!(BZ2_bzCompressInit(&mut strm, block_size_100k, 0, 0), BZ_OK);

        while source_off < source.len() {
            let chunk = (source.len() - source_off).min(4093);
            strm.next_in = source.as_ptr().add(source_off).cast_mut().cast::<c_char>();
            strm.avail_in = chunk as u32;

            while strm.avail_in > 0 {
                let out_chunk = (dest.len() - dest_off).min(1537);
                strm.next_out = dest.as_mut_ptr().add(dest_off).cast::<c_char>();
                strm.avail_out = out_chunk as u32;
                let ret = BZ2_bzCompress(&mut strm, BZ_RUN);
                assert_eq!(ret, BZ_RUN_OK);
                dest_off += out_chunk - strm.avail_out as usize;
            }

            source_off += chunk;
        }

        loop {
            let out_chunk = (dest.len() - dest_off).min(1537);
            strm.next_out = dest.as_mut_ptr().add(dest_off).cast::<c_char>();
            strm.avail_out = out_chunk as u32;
            let ret = BZ2_bzCompress(&mut strm, BZ_FINISH);
            dest_off += out_chunk - strm.avail_out as usize;
            if ret == BZ_STREAM_END {
                break;
            }
            assert_eq!(ret, BZ_FINISH_OK);
        }

        assert_eq!(BZ2_bzCompressEnd(&mut strm), BZ_OK);
    }

    dest.truncate(dest_off);
    dest
}

fn compress_via_buffer_api(source: &[u8], block_size_100k: i32) -> Vec<u8> {
    let mut dest = vec![0u8; compress_bound(source.len())];
    let mut dest_len = dest.len() as u32;

    let ret = unsafe {
        BZ2_bzBuffToBuffCompress(
            dest.as_mut_ptr().cast::<c_char>(),
            &mut dest_len,
            source.as_ptr().cast_mut().cast::<c_char>(),
            source.len() as u32,
            block_size_100k,
            0,
            0,
        )
    };
    assert_eq!(ret, BZ_OK);
    dest.truncate(dest_len as usize);
    dest
}

#[test]
fn tracked_upstream_golden_streams_match_bit_for_bit() {
    for (block_size, source, expected) in [
        (1, SAMPLE1_REF, SAMPLE1_BZ2),
        (2, SAMPLE2_REF, SAMPLE2_BZ2),
        (3, SAMPLE3_REF, SAMPLE3_BZ2),
    ] {
        assert_eq!(compress_via_stream(source, block_size), expected);
        assert_eq!(compress_via_buffer_api(source, block_size), expected);
    }
}
