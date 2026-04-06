use bz2::bz_stream;
use bz2::constants::{BZ_OK, BZ_OUTBUFF_FULL, BZ_SEQUENCE_ERROR, BZ_STREAM_END, BZ_UNEXPECTED_EOF};
use bz2::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use bz2::ffi::BZ2_bzBuffToBuffDecompress;
use bz2::stdio::{
    BZ2_bzRead, BZ2_bzReadClose, BZ2_bzReadGetUnused, BZ2_bzReadOpen, BZ2_bzclose, BZ2_bzdopen,
    BZ2_bzerror, BZ2_bzopen, BZ2_bzread,
};
use bz2::types::CFile;
use std::ffi::CStr;
use std::ffi::{c_void, CString};
use std::fs::{self, File};
use std::mem::MaybeUninit;
use std::os::fd::IntoRawFd;
use std::os::raw::{c_char, c_int};
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;
use std::time::{SystemTime, UNIX_EPOCH};

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
const SAMPLE3_RANDOMIZED_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/sample3.randomized.bz2"
));

extern "C" {
    fn fdopen(fd: c_int, mode: *const c_char) -> *mut CFile;
    fn fclose(file: *mut CFile) -> c_int;
}

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
}

fn chunked_stream_decompress(input: &[u8], expected: &[u8], small: c_int) -> Vec<u8> {
    let mut strm = zeroed_stream();
    let mut output = vec![0u8; expected.len()];
    let mut source_off = 0usize;
    let mut output_off = 0usize;
    let mut steps = 0usize;

    unsafe {
        assert_eq!(BZ2_bzDecompressInit(&mut strm, 0, small), BZ_OK);
        loop {
            if strm.avail_in == 0 && source_off < input.len() {
                let chunk = (input.len() - source_off).min(811);
                strm.next_in = input.as_ptr().add(source_off).cast_mut().cast::<c_char>();
                strm.avail_in = chunk as u32;
                source_off += chunk;
            }

            let out_chunk = (output.len() - output_off).min(997);
            strm.next_out = output.as_mut_ptr().add(output_off).cast::<c_char>();
            strm.avail_out = out_chunk as u32;
            let ret = BZ2_bzDecompress(&mut strm);
            output_off += out_chunk - strm.avail_out as usize;

            if ret == BZ_STREAM_END {
                break;
            }
            assert_eq!(ret, BZ_OK, "stream decode returned {ret}");
            steps += 1;
            assert!(steps < 10_000, "stream decode stopped making progress");
        }
        assert_eq!(BZ2_bzDecompressEnd(&mut strm), BZ_OK);
    }

    output.truncate(output_off);
    output
}

fn buff_to_buff(input: &[u8], output_cap: usize, small: c_int) -> (c_int, usize, Vec<u8>) {
    let mut output = vec![0u8; output_cap];
    let mut dest_len = output_cap as u32;
    let code = unsafe {
        BZ2_bzBuffToBuffDecompress(
            output.as_mut_ptr().cast::<c_char>(),
            &mut dest_len,
            input.as_ptr().cast_mut().cast::<c_char>(),
            input.len() as u32,
            small,
            0,
        )
    };
    output.truncate(dest_len as usize);
    (code, dest_len as usize, output)
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

fn bzerror_state(handle: *mut c_void) -> (c_int, String) {
    let mut err = i32::MIN;
    let msg = unsafe { CStr::from_ptr(BZ2_bzerror(handle, &mut err)) };
    (err, msg.to_str().unwrap().to_owned())
}

fn read_all_via_bzopen(path: &Path, mode: &str) -> Vec<u8> {
    let path_c = CString::new(path.as_os_str().to_string_lossy().into_owned()).unwrap();
    let mode_c = CString::new(mode).unwrap();
    let handle = unsafe { BZ2_bzopen(path_c.as_ptr(), mode_c.as_ptr()) };
    assert!(!handle.is_null());

    let mut output = Vec::new();
    let mut buf = [0u8; 389];
    loop {
        let n = unsafe {
            BZ2_bzread(
                handle,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            )
        };
        assert!(n >= 0, "BZ2_bzread failed with {n}");
        if n == 0 {
            break;
        }
        output.extend_from_slice(&buf[..n as usize]);
    }
    unsafe { BZ2_bzclose(handle) };
    output
}

fn read_all_via_bzdopen(path: &Path, mode: &str) -> Vec<u8> {
    let file = File::open(path).unwrap();
    let fd = file.into_raw_fd();
    let mode_c = CString::new(mode).unwrap();
    let handle = unsafe { BZ2_bzdopen(fd, mode_c.as_ptr()) };
    assert!(!handle.is_null());

    let mut output = Vec::new();
    let mut buf = [0u8; 257];
    loop {
        let n = unsafe {
            BZ2_bzread(
                handle,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            )
        };
        assert!(n >= 0, "BZ2_bzread failed with {n}");
        if n == 0 {
            break;
        }
        output.extend_from_slice(&buf[..n as usize]);
    }
    unsafe { BZ2_bzclose(handle) };
    output
}

#[test]
fn stream_api_decompresses_upstream_samples_in_both_modes() {
    for &(compressed, expected) in &[
        (SAMPLE1_BZ2, SAMPLE1_REF),
        (SAMPLE2_BZ2, SAMPLE2_REF),
        (SAMPLE3_BZ2, SAMPLE3_REF),
        (SAMPLE3_RANDOMIZED_BZ2, SAMPLE3_REF),
    ] {
        assert_eq!(chunked_stream_decompress(compressed, expected, 0), expected);
        assert_eq!(chunked_stream_decompress(compressed, expected, 1), expected);
    }
}

#[test]
fn buffer_api_matches_upstream_small_mode_and_outbuff_full_behavior() {
    for &(compressed, expected) in &[
        (SAMPLE1_BZ2, SAMPLE1_REF),
        (SAMPLE2_BZ2, SAMPLE2_REF),
        (SAMPLE3_BZ2, SAMPLE3_REF),
    ] {
        let (code, len, output) = buff_to_buff(compressed, expected.len(), 0);
        assert_eq!(code, BZ_OK);
        assert_eq!(len, expected.len());
        assert_eq!(output, expected);

        let (code, len, output) = buff_to_buff(compressed, expected.len(), 1);
        assert_eq!(code, BZ_OK);
        assert_eq!(len, expected.len());
        assert_eq!(output, expected);
    }

    let (code, _, _) = buff_to_buff(SAMPLE3_BZ2, 32, 0);
    assert_eq!(code, BZ_OUTBUFF_FULL);
}

#[test]
fn read_api_preserves_concatenated_member_and_unused_trailer_contract() {
    let trailer = b"TAIL";
    let path = temp_path("concat");
    let mut file_bytes = Vec::new();
    file_bytes.extend_from_slice(SAMPLE3_BZ2);
    file_bytes.extend_from_slice(SAMPLE3_RANDOMIZED_BZ2);
    file_bytes.extend_from_slice(trailer);
    fs::write(&path, &file_bytes).unwrap();

    let fd = File::open(&path).unwrap().into_raw_fd();
    let mode_c = CString::new("rb").unwrap();
    let file = unsafe { fdopen(fd, mode_c.as_ptr()) };
    assert!(!file.is_null());

    unsafe {
        let mut bzerr = BZ_OK;
        let first = BZ2_bzReadOpen(&mut bzerr, file, 0, 0, ptr::null_mut(), 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!first.is_null());

        let mut first_out = Vec::new();
        let mut buf = [0u8; 257];
        loop {
            let n = BZ2_bzRead(
                &mut bzerr,
                first,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            );
            assert!(n >= 0);
            first_out.extend_from_slice(&buf[..n as usize]);
            if bzerr == BZ_STREAM_END {
                break;
            }
            assert_eq!(bzerr, BZ_OK);
        }
        assert_eq!(first_out, SAMPLE3_REF);

        let mut unused_ptr = ptr::null_mut();
        let mut unused_len = 0;
        BZ2_bzReadGetUnused(&mut bzerr, first, &mut unused_ptr, &mut unused_len);
        assert_eq!(bzerr, BZ_OK);
        let unused_copy =
            slice::from_raw_parts(unused_ptr.cast::<u8>(), unused_len as usize).to_vec();
        assert_eq!(
            &unused_copy[..SAMPLE3_RANDOMIZED_BZ2.len()],
            SAMPLE3_RANDOMIZED_BZ2
        );
        assert_eq!(&unused_copy[SAMPLE3_RANDOMIZED_BZ2.len()..], trailer);

        BZ2_bzReadClose(&mut bzerr, first);
        assert_eq!(bzerr, BZ_OK);

        let second = BZ2_bzReadOpen(
            &mut bzerr,
            file,
            0,
            1,
            unused_copy.as_ptr().cast::<c_void>().cast_mut(),
            unused_copy.len() as c_int,
        );
        assert_eq!(bzerr, BZ_OK);
        assert!(!second.is_null());

        let mut second_out = Vec::new();
        loop {
            let n = BZ2_bzRead(
                &mut bzerr,
                second,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            );
            assert!(n >= 0);
            second_out.extend_from_slice(&buf[..n as usize]);
            if bzerr == BZ_STREAM_END {
                break;
            }
            assert_eq!(bzerr, BZ_OK);
        }
        assert_eq!(second_out, SAMPLE3_REF);

        BZ2_bzReadGetUnused(&mut bzerr, second, &mut unused_ptr, &mut unused_len);
        assert_eq!(bzerr, BZ_OK);
        let trailer_unused = slice::from_raw_parts(unused_ptr.cast::<u8>(), unused_len as usize);
        assert_eq!(trailer_unused, trailer);

        BZ2_bzReadClose(&mut bzerr, second);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(fclose(file), 0);
    }

    fs::remove_file(path).unwrap();
}

#[test]
fn bzopen_and_bzdopen_read_wrappers_decompress_samples() {
    let bzopen_path = temp_path("bzopen");
    let bzdopen_path = temp_path("bzdopen");
    fs::write(&bzopen_path, SAMPLE3_RANDOMIZED_BZ2).unwrap();
    fs::write(&bzdopen_path, SAMPLE1_BZ2).unwrap();

    assert_eq!(read_all_via_bzopen(&bzopen_path, "rs"), SAMPLE3_REF);
    assert_eq!(read_all_via_bzdopen(&bzdopen_path, "r"), SAMPLE1_REF);

    fs::remove_file(bzopen_path).unwrap();
    fs::remove_file(bzdopen_path).unwrap();
}

#[test]
fn bzread_and_bzerror_preserve_read_side_wrapper_semantics() {
    let ok_path = temp_path("bzread-ok");
    let truncated_path = temp_path("bzread-truncated");
    fs::write(&ok_path, SAMPLE3_RANDOMIZED_BZ2).unwrap();
    fs::write(&truncated_path, &SAMPLE3_BZ2[..SAMPLE3_BZ2.len() - 1]).unwrap();

    let ok_path_c = CString::new(ok_path.as_os_str().to_string_lossy().into_owned()).unwrap();
    let truncated_path_c =
        CString::new(truncated_path.as_os_str().to_string_lossy().into_owned()).unwrap();
    let read_mode = CString::new("rs").unwrap();

    let ok_handle = unsafe { BZ2_bzopen(ok_path_c.as_ptr(), read_mode.as_ptr()) };
    assert!(!ok_handle.is_null());
    let mut ok_output = Vec::new();
    let mut buf = [0u8; 211];
    loop {
        let n = unsafe {
            BZ2_bzread(
                ok_handle,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            )
        };
        assert!(n >= 0, "unexpected bzread failure on valid stream");
        if n == 0 {
            break;
        }
        ok_output.extend_from_slice(&buf[..n as usize]);
    }
    assert_eq!(ok_output, SAMPLE3_REF);
    assert_eq!(
        unsafe {
            BZ2_bzread(
                ok_handle,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            )
        },
        0
    );
    assert_eq!(bzerror_state(ok_handle), (BZ_OK, "OK".to_owned()));
    unsafe { BZ2_bzclose(ok_handle) };

    let truncated_handle = unsafe { BZ2_bzopen(truncated_path_c.as_ptr(), read_mode.as_ptr()) };
    assert!(!truncated_handle.is_null());
    loop {
        let n = unsafe {
            BZ2_bzread(
                truncated_handle,
                buf.as_mut_ptr().cast::<c_void>(),
                buf.len() as c_int,
            )
        };
        if n == -1 {
            break;
        }
        assert!(
            n >= 0,
            "truncated stream produced unexpected read result {n}"
        );
    }
    assert_eq!(
        bzerror_state(truncated_handle),
        (BZ_UNEXPECTED_EOF, "UNEXPECTED_EOF".to_owned())
    );
    unsafe { BZ2_bzclose(truncated_handle) };

    fs::remove_file(ok_path).unwrap();
    fs::remove_file(truncated_path).unwrap();
}

#[test]
fn read_get_unused_requires_stream_end_before_exposing_trailer_bytes() {
    let path = temp_path("read-unused-seq");
    fs::write(&path, SAMPLE1_BZ2).unwrap();

    let fd = File::open(&path).unwrap().into_raw_fd();
    let mode_c = CString::new("rb").unwrap();
    let file = unsafe { fdopen(fd, mode_c.as_ptr()) };
    assert!(!file.is_null());

    unsafe {
        let mut bzerr = BZ_OK;
        let handle = BZ2_bzReadOpen(&mut bzerr, file, 0, 0, ptr::null_mut(), 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!handle.is_null());

        let mut unused_ptr = ptr::null_mut();
        let mut unused_len = 0;
        BZ2_bzReadGetUnused(&mut bzerr, handle, &mut unused_ptr, &mut unused_len);
        assert_eq!(bzerr, BZ_SEQUENCE_ERROR);

        BZ2_bzReadClose(&mut bzerr, handle);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(fclose(file), 0);
    }

    fs::remove_file(path).unwrap();
}
