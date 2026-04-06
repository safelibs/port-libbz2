use bz2::bz_stream;
use bz2::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use bz2::constants::{BZ_FINISH, BZ_FINISH_OK, BZ_OK, BZ_RUN, BZ_RUN_OK, BZ_STREAM_END};
use bz2::ffi::BZ2_bzBuffToBuffCompress;
use bz2::stdio::{
    BZ2_bzWrite, BZ2_bzWriteClose, BZ2_bzWriteOpen, BZ2_bzclose, BZ2_bzflush, BZ2_bzopen,
    BZ2_bzwrite,
};
use bz2::types::CFile;
use std::ffi::CString;
use std::fs;
use std::mem::MaybeUninit;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    fn fclose(file: *mut CFile) -> c_int;
}

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
}

fn compress_bound(source_len: usize) -> usize {
    source_len + (source_len / 100) + 601
}

fn temp_path(label: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "libbz2-safe-{label}-{stamp}-{}",
        std::process::id()
    ))
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

fn compress_via_stdio(source: &[u8], block_size_100k: i32) -> Vec<u8> {
    let path = temp_path("golden-stdio");
    let path_c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let mode = CString::new("wb").unwrap();
    let file = unsafe { fopen(path_c.as_ptr(), mode.as_ptr()) };
    assert!(!file.is_null());

    unsafe {
        let mut bzerr = BZ_OK;
        let handle = BZ2_bzWriteOpen(&mut bzerr, file, block_size_100k, 0, 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!handle.is_null());

        let mut offset = 0usize;
        while offset < source.len() {
            let chunk = (source.len() - offset).min(997);
            BZ2_bzWrite(
                &mut bzerr,
                handle,
                source.as_ptr().add(offset).cast_mut().cast(),
                chunk as c_int,
            );
            assert_eq!(bzerr, BZ_OK);
            offset += chunk;
        }

        let mut total_in = 0u32;
        let mut total_out = 0u32;
        BZ2_bzWriteClose(&mut bzerr, handle, 0, &mut total_in, &mut total_out);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(total_in, source.len() as u32);
        assert!(total_out > 0);
        assert_eq!(fclose(file), 0);
    }

    let compressed = fs::read(&path).unwrap();
    fs::remove_file(path).unwrap();
    compressed
}

fn compress_via_bzopen(source: &[u8], block_size_100k: i32) -> Vec<u8> {
    let path = temp_path("golden-bzopen");
    let path_c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let mode = CString::new(format!("w{block_size_100k}")).unwrap();
    let handle = unsafe { BZ2_bzopen(path_c.as_ptr(), mode.as_ptr()) };
    assert!(!handle.is_null());

    let mut offset = 0usize;
    while offset < source.len() {
        let chunk = (source.len() - offset).min(701);
        let written = unsafe {
            BZ2_bzwrite(
                handle,
                source.as_ptr().add(offset).cast_mut().cast(),
                chunk as c_int,
            )
        };
        assert_eq!(written, chunk as c_int);
        offset += chunk;
    }

    assert_eq!(unsafe { BZ2_bzflush(handle) }, 0);
    unsafe { BZ2_bzclose(handle) };

    let compressed = fs::read(&path).unwrap();
    fs::remove_file(path).unwrap();
    compressed
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

#[test]
fn tracked_upstream_golden_streams_match_write_wrappers_bit_for_bit() {
    for (block_size, source, expected) in [
        (1, SAMPLE1_REF, SAMPLE1_BZ2),
        (2, SAMPLE2_REF, SAMPLE2_BZ2),
        (3, SAMPLE3_REF, SAMPLE3_BZ2),
    ] {
        assert_eq!(compress_via_stdio(source, block_size), expected);
        assert_eq!(compress_via_bzopen(source, block_size), expected);
    }
}
