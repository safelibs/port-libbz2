use bz2::bz_stream;
use bz2::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use bz2::constants::{BZ_FINISH, BZ_FINISH_OK, BZ_OK, BZ_RUN, BZ_RUN_OK, BZ_STREAM_END};
use bz2::ffi::BZ2_bzBuffToBuffDecompress;
use std::fs;
use std::io::Write;
use std::mem::MaybeUninit;
use std::os::raw::c_char;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
        assert_eq!(BZ2_bzCompressInit(&mut strm, block_size_100k, 0, 30), BZ_OK);

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

fn decompress_all(source: &[u8], expected_len: usize) -> Vec<u8> {
    let mut dest = vec![0u8; expected_len];
    let mut dest_len = expected_len as u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            dest.as_mut_ptr().cast::<c_char>(),
            &mut dest_len,
            source.as_ptr().cast_mut().cast::<c_char>(),
            source.len() as u32,
            0,
            0,
        )
    };
    assert_eq!(ret, BZ_OK);
    dest.truncate(dest_len as usize);
    dest
}

#[test]
fn mk251_regression_preserves_the_blocksort_1007_fix() {
    let payload = vec![251u8; 1_250_000];
    let compressed = compress_via_stream(&payload, 9);

    assert_eq!(decompress_all(&compressed, payload.len()), payload);

    let input_path = temp_path("mk251");
    fs::File::create(&input_path)
        .unwrap()
        .write_all(&payload)
        .unwrap();

    let original_bzip2 = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../original/bzip2")
        .canonicalize()
        .unwrap();
    let output = Command::new(original_bzip2)
        .arg("-9c")
        .arg(&input_path)
        .output()
        .expect("run upstream bzip2 for mk251 oracle");
    assert!(
        output.status.success(),
        "upstream mk251 oracle failed: {:?}",
        output.status
    );

    fs::remove_file(input_path).unwrap();
    assert_eq!(
        compressed, output.stdout,
        "mk251 compressor drifted from upstream"
    );
}
