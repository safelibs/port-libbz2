use bz2::bz_stream;
use bz2::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use bz2::constants::{BZ_FINISH, BZ_FINISH_OK, BZ_OK, BZ_RUN, BZ_RUN_OK, BZ_STREAM_END};
use bz2::ffi::{BZ2_bzBuffToBuffCompress, BZ2_bzBuffToBuffDecompress};
use bz2::stdio::{
    BZ2_bzWrite, BZ2_bzWriteClose64, BZ2_bzWriteOpen, BZ2_bzclose, BZ2_bzflush, BZ2_bzopen,
    BZ2_bzwrite,
};
use bz2::types::CFile;
use std::ffi::CString;
use std::fs;
use std::mem::MaybeUninit;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    fn fclose(file: *mut CFile) -> c_int;
}

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
}

fn fill_payload(dst: &mut [u8], seed: u32) {
    const ALPHABET: &[u8] = b"The quick brown fox jumps over the lazy dog. 0123456789\n";

    for (i, byte) in dst.iter_mut().enumerate() {
        let mix = i as u32 + (seed * 17) + (i as u32 / 29) + (i as u32 / 251);
        *byte = ALPHABET[(mix as usize) % (ALPHABET.len() - 1)];
        if (i % 97) >= 64 {
            *byte = b'A' + (mix % 23) as u8;
        }
    }
}

fn compress_bound(source_len: usize) -> usize {
    source_len + (source_len / 100) + 601
}

fn stream_compress(source: &[u8], block_size_100k: i32) -> Vec<u8> {
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

        assert_eq!(strm.total_in_lo32, source.len() as u32);
        assert_eq!(strm.total_in_hi32, 0);
        assert_eq!(strm.total_out_lo32, dest_off as u32);
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

fn write_via_bzopen(payload: &[u8], mode: &str) -> Vec<u8> {
    let wrapper_path = temp_path("bzopen");
    let wrapper_path_c = CString::new(wrapper_path.to_string_lossy().as_bytes()).unwrap();
    let mode = CString::new(mode).unwrap();
    let handle = unsafe { BZ2_bzopen(wrapper_path_c.as_ptr(), mode.as_ptr()) };
    assert!(!handle.is_null());

    let mut offset = 0usize;
    while offset < payload.len() {
        let chunk = (payload.len() - offset).min(701);
        let written = unsafe {
            BZ2_bzwrite(
                handle,
                payload.as_ptr().add(offset).cast_mut().cast(),
                chunk as c_int,
            )
        };
        assert_eq!(written, chunk as c_int);
        offset += chunk;
    }
    assert_eq!(unsafe { BZ2_bzflush(handle) }, 0);
    unsafe { BZ2_bzclose(handle) };

    let wrapped = fs::read(&wrapper_path).unwrap();
    fs::remove_file(&wrapper_path).unwrap();
    wrapped
}

#[test]
fn stream_and_buffer_compress_roundtrip_generated_payload() {
    let mut payload = vec![0u8; 180_000];
    fill_payload(&mut payload, 1);

    let compressed = stream_compress(&payload, 1);
    assert_eq!(decompress_all(&compressed, payload.len()), payload);

    let mut dest = vec![0u8; compress_bound(payload.len())];
    let mut dest_len = dest.len() as u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffCompress(
            dest.as_mut_ptr().cast::<c_char>(),
            &mut dest_len,
            payload.as_ptr().cast_mut().cast::<c_char>(),
            payload.len() as u32,
            1,
            0,
            0,
        )
    };
    assert_eq!(ret, BZ_OK);
    dest.truncate(dest_len as usize);
    assert_eq!(decompress_all(&dest, payload.len()), payload);
}

#[test]
fn write_side_stdio_and_wrapper_paths_roundtrip_generated_payload() {
    let mut payload = vec![0u8; 64_000];
    fill_payload(&mut payload, 5);

    let direct_path = temp_path("write-open");
    let direct_path_c = CString::new(direct_path.to_string_lossy().as_bytes()).unwrap();
    let wb = CString::new("wb").unwrap();
    let file = unsafe { fopen(direct_path_c.as_ptr(), wb.as_ptr()) };
    assert!(!file.is_null());

    unsafe {
        let mut bzerr = BZ_OK;
        let handle = BZ2_bzWriteOpen(&mut bzerr, file, 3, 0, 0);
        assert!(!handle.is_null());
        assert_eq!(bzerr, BZ_OK);

        let mut offset = 0usize;
        while offset < payload.len() {
            let chunk = (payload.len() - offset).min(1531);
            BZ2_bzWrite(
                &mut bzerr,
                handle,
                payload.as_ptr().add(offset).cast_mut().cast(),
                chunk as c_int,
            );
            assert_eq!(bzerr, BZ_OK);
            offset += chunk;
        }

        let mut in_lo = 0u32;
        let mut in_hi = 0u32;
        let mut out_lo = 0u32;
        let mut out_hi = 0u32;
        BZ2_bzWriteClose64(
            &mut bzerr,
            handle,
            0,
            &mut in_lo,
            &mut in_hi,
            &mut out_lo,
            &mut out_hi,
        );
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(in_lo, payload.len() as u32);
        assert_eq!(in_hi, 0);
        assert!(out_lo > 0);
        assert_eq!(out_hi, 0);

        assert_eq!(fclose(file), 0);
    }

    let compressed = fs::read(&direct_path).unwrap();
    assert_eq!(decompress_all(&compressed, payload.len()), payload);
    fs::remove_file(&direct_path).unwrap();

    let wrapped = write_via_bzopen(&payload, "w7");
    assert_eq!(decompress_all(&wrapped, payload.len()), payload);
}

#[test]
fn write_close_abandon_reports_counters_without_finishing_stream() {
    let mut payload = vec![0u8; 48_000];
    fill_payload(&mut payload, 9);

    let path = temp_path("write-abandon");
    let path_c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let wb = CString::new("wb").unwrap();
    let file = unsafe { fopen(path_c.as_ptr(), wb.as_ptr()) };
    assert!(!file.is_null());
    let mut out_lo = 0u32;

    unsafe {
        let mut bzerr = BZ_OK;
        let handle = BZ2_bzWriteOpen(&mut bzerr, file, 5, 0, 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!handle.is_null());

        let mut offset = 0usize;
        while offset < payload.len() {
            let chunk = (payload.len() - offset).min(1021);
            BZ2_bzWrite(
                &mut bzerr,
                handle,
                payload.as_ptr().add(offset).cast_mut().cast(),
                chunk as c_int,
            );
            assert_eq!(bzerr, BZ_OK);
            offset += chunk;
        }

        let mut in_lo = 0u32;
        let mut in_hi = 0u32;
        let mut out_hi = 0u32;
        BZ2_bzWriteClose64(
            &mut bzerr,
            handle,
            1,
            &mut in_lo,
            &mut in_hi,
            &mut out_lo,
            &mut out_hi,
        );
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(in_lo, payload.len() as u32);
        assert_eq!(in_hi, 0);
        assert_eq!(out_hi, 0);
        assert_eq!(fclose(file), 0);
    }

    let partial = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(partial.len() as u32, out_lo);

    let mut restored = vec![0u8; payload.len()];
    let mut restored_len = restored.len() as u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            restored.as_mut_ptr().cast::<c_char>(),
            &mut restored_len,
            partial.as_ptr().cast_mut().cast::<c_char>(),
            partial.len() as u32,
            0,
            0,
        )
    };
    assert_ne!(ret, BZ_OK);
}

#[test]
fn bzopen_write_mode_zero_clamps_to_block_size_one() {
    let mut payload = vec![0u8; 32_000];
    fill_payload(&mut payload, 11);

    let clamped = write_via_bzopen(&payload, "w0");
    let explicit = write_via_bzopen(&payload, "w1");

    assert_eq!(&clamped[..4], b"BZh1");
    assert_eq!(clamped, explicit);
    assert_eq!(decompress_all(&clamped, payload.len()), payload);
}
