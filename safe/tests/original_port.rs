use bz2::bz_stream;
use bz2::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use bz2::constants::{
    BZ_FINISH, BZ_FINISH_OK, BZ_OK, BZ_OUTBUFF_FULL, BZ_RUN, BZ_RUN_OK, BZ_SEQUENCE_ERROR,
    BZ_STREAM_END,
};
use bz2::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use bz2::ffi::{BZ2_bzBuffToBuffCompress, BZ2_bzBuffToBuffDecompress, BZ2_bzlibVersion};
use bz2::stdio::{
    BZ2_bzRead, BZ2_bzReadClose, BZ2_bzReadGetUnused, BZ2_bzReadOpen, BZ2_bzWrite,
    BZ2_bzWriteClose64, BZ2_bzWriteOpen, BZ2_bzclose, BZ2_bzdopen, BZ2_bzerror, BZ2_bzflush,
    BZ2_bzopen, BZ2_bzread, BZ2_bzwrite,
};
use bz2::types::CFile;
use std::ffi::{c_void, CStr, CString};
use std::fs;
use std::mem::MaybeUninit;
use std::os::fd::IntoRawFd;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::ptr;
use std::slice;
use std::time::{SystemTime, UNIX_EPOCH};

const STREAM_PAYLOAD_LEN: usize = 180_000;
const BUFFER_PAYLOAD_LEN: usize = 9_000;
const FILE_PAYLOAD_LEN: usize = 64_000;
const WRAPPER_PAYLOAD_LEN: usize = 48_000;

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

fn compressed_bound(source_len: usize) -> usize {
    source_len + (source_len / 100) + 601
}

fn stream_compress(source: &[u8], dest_cap: usize) -> Vec<u8> {
    let mut strm = zeroed_stream();
    let mut dest = vec![0u8; dest_cap];
    let mut source_off = 0usize;
    let mut dest_off = 0usize;

    unsafe {
        assert_eq!(BZ2_bzCompressInit(&mut strm, 1, 0, 30), BZ_OK);

        while source_off < source.len() {
            let chunk = (source.len() - source_off).min(4093);
            strm.next_in = source.as_ptr().add(source_off).cast_mut().cast::<c_char>();
            strm.avail_in = chunk as u32;

            while strm.avail_in > 0 {
                let out_chunk = (dest.len() - dest_off).min(1537);
                assert!(out_chunk > 0, "stream_compress overflow");
                strm.next_out = dest.as_mut_ptr().add(dest_off).cast::<c_char>();
                strm.avail_out = out_chunk as u32;
                assert_eq!(BZ2_bzCompress(&mut strm, BZ_RUN), BZ_RUN_OK);
                dest_off += out_chunk - strm.avail_out as usize;
            }

            source_off += chunk;
        }

        loop {
            let out_chunk = (dest.len() - dest_off).min(1537);
            assert!(out_chunk > 0, "stream_compress finish overflow");
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

fn stream_decompress(source: &[u8], dest_cap: usize, small: c_int) -> Vec<u8> {
    let mut strm = zeroed_stream();
    let mut dest = vec![0u8; dest_cap];
    let mut source_off = 0usize;
    let mut dest_off = 0usize;

    unsafe {
        assert_eq!(BZ2_bzDecompressInit(&mut strm, 0, small), BZ_OK);

        loop {
            if strm.avail_in == 0 && source_off < source.len() {
                let chunk = (source.len() - source_off).min(811);
                strm.next_in = source.as_ptr().add(source_off).cast_mut().cast::<c_char>();
                strm.avail_in = chunk as u32;
                source_off += chunk;
            }

            let out_chunk = (dest.len() - dest_off).min(997);
            assert!(out_chunk > 0, "stream_decompress overflow");
            strm.next_out = dest.as_mut_ptr().add(dest_off).cast::<c_char>();
            strm.avail_out = out_chunk as u32;

            let ret = BZ2_bzDecompress(&mut strm);
            dest_off += out_chunk - strm.avail_out as usize;
            if ret == BZ_STREAM_END {
                break;
            }
            assert_eq!(ret, BZ_OK);
            assert!(
                !(source_off == source.len() && strm.avail_in == 0 && strm.avail_out > 0),
                "stream_decompress ended before stream end"
            );
        }

        assert_eq!(strm.total_in_lo32, source.len() as u32);
        assert_eq!(strm.total_in_hi32, 0);
        assert_eq!(strm.total_out_lo32, dest_off as u32);
        assert_eq!(BZ2_bzDecompressEnd(&mut strm), BZ_OK);
    }

    dest.truncate(dest_off);
    dest
}

fn read_bz_stream(bzf: *mut c_void, dest_cap: usize) -> Vec<u8> {
    let mut total = Vec::with_capacity(dest_cap);
    let mut bzerr = BZ_OK;
    let mut buf = [0u8; 257];
    let mut extra = 0u8;

    unsafe {
        loop {
            let chunk = (dest_cap.saturating_sub(total.len())).min(buf.len());
            if chunk == 0 {
                let nread = BZ2_bzRead(&mut bzerr, bzf, (&mut extra as *mut u8).cast(), 1);
                if bzerr == BZ_STREAM_END && nread == 0 {
                    break;
                }
                panic!("read_bz_stream overflow");
            }

            let nread = BZ2_bzRead(
                &mut bzerr,
                bzf,
                buf.as_mut_ptr().cast::<c_void>(),
                chunk as c_int,
            );
            assert!(nread >= 0);
            total.extend_from_slice(&buf[..nread as usize]);

            if bzerr == BZ_OK {
                continue;
            }
            if bzerr == BZ_STREAM_END {
                break;
            }
            panic!("BZ2_bzRead failed with {bzerr}");
        }
    }

    total
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

#[test]
fn ported_version_string_matches_upstream() {
    let version = unsafe { CStr::from_ptr(BZ2_bzlibVersion()) }
        .to_str()
        .unwrap();
    assert_eq!(version, "1.0.8, 13-Jul-2019");
}

#[test]
fn ported_core_stream_api_matches_upstream() {
    let mut source = vec![0u8; STREAM_PAYLOAD_LEN];
    fill_payload(&mut source, 1);

    let compressed = stream_compress(&source, compressed_bound(STREAM_PAYLOAD_LEN));
    let restored = stream_decompress(&compressed, STREAM_PAYLOAD_LEN, 1);
    assert_eq!(restored.len(), STREAM_PAYLOAD_LEN);
    assert_eq!(restored, source);
}

#[test]
fn ported_buffer_api_matches_upstream() {
    let mut source = vec![0u8; BUFFER_PAYLOAD_LEN];
    let mut compressed = vec![0u8; compressed_bound(BUFFER_PAYLOAD_LEN)];
    let mut restored = vec![0u8; BUFFER_PAYLOAD_LEN];
    fill_payload(&mut source, 3);

    let mut compressed_len = compressed.len() as u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffCompress(
            compressed.as_mut_ptr().cast::<c_char>(),
            &mut compressed_len,
            source.as_ptr().cast_mut().cast::<c_char>(),
            source.len() as u32,
            9,
            0,
            0,
        )
    };
    assert_eq!(ret, BZ_OK);
    compressed.truncate(compressed_len as usize);

    let mut restored_len = restored.len() as u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            restored.as_mut_ptr().cast::<c_char>(),
            &mut restored_len,
            compressed.as_ptr().cast_mut().cast::<c_char>(),
            compressed.len() as u32,
            0,
            0,
        )
    };
    assert_eq!(ret, BZ_OK);
    assert_eq!(restored_len as usize, BUFFER_PAYLOAD_LEN);
    assert_eq!(&restored[..BUFFER_PAYLOAD_LEN], &source);

    restored_len = restored.len() as u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            restored.as_mut_ptr().cast::<c_char>(),
            &mut restored_len,
            compressed.as_ptr().cast_mut().cast::<c_char>(),
            compressed.len() as u32,
            1,
            0,
        )
    };
    assert_eq!(ret, BZ_OK);
    assert_eq!(&restored[..BUFFER_PAYLOAD_LEN], &source);

    let mut tiny_len = 32u32;
    let ret = unsafe {
        BZ2_bzBuffToBuffDecompress(
            restored.as_mut_ptr().cast::<c_char>(),
            &mut tiny_len,
            compressed.as_ptr().cast_mut().cast::<c_char>(),
            compressed.len() as u32,
            0,
            0,
        )
    };
    assert_eq!(ret, BZ_OUTBUFF_FULL);
}

#[test]
fn ported_high_level_api_matches_upstream() {
    let mut source = vec![0u8; FILE_PAYLOAD_LEN];
    let mut restored = vec![0u8; FILE_PAYLOAD_LEN];
    fill_payload(&mut source, 5);

    let path = temp_path("public-api-high-level");
    let path_c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let wb = CString::new("wb").unwrap();
    let rb = CString::new("rb").unwrap();

    unsafe {
        let fp = fopen(path_c.as_ptr(), wb.as_ptr());
        assert!(!fp.is_null());

        let mut bzerr = BZ_OK;
        let bzf = BZ2_bzWriteOpen(&mut bzerr, fp, 3, 0, 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!bzf.is_null());

        let mut offset = 0usize;
        while offset < source.len() {
            let chunk = (source.len() - offset).min(1531);
            BZ2_bzWrite(
                &mut bzerr,
                bzf,
                source.as_ptr().add(offset).cast_mut().cast::<c_void>(),
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
            bzf,
            0,
            &mut in_lo,
            &mut in_hi,
            &mut out_lo,
            &mut out_hi,
        );
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(in_lo, FILE_PAYLOAD_LEN as u32);
        assert_eq!(in_hi, 0);
        assert!(out_lo > 0);
        assert_eq!(out_hi, 0);
        assert_eq!(fclose(fp), 0);

        let fp = fopen(path_c.as_ptr(), rb.as_ptr());
        assert!(!fp.is_null());
        let bzf = BZ2_bzReadOpen(&mut bzerr, fp, 0, 0, ptr::null_mut(), 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!bzf.is_null());

        let output = read_bz_stream(bzf, FILE_PAYLOAD_LEN);
        restored[..output.len()].copy_from_slice(&output);
        assert_eq!(output.len(), FILE_PAYLOAD_LEN);
        assert_eq!(output, source);

        BZ2_bzReadClose(&mut bzerr, bzf);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(fclose(fp), 0);
    }
    fs::remove_file(&path).unwrap();

    let trailer = b"TAIL";
    let mut first_payload = vec![0u8; 731];
    let mut second_payload = vec![0u8; 887];
    fill_payload(&mut first_payload, 7);
    fill_payload(&mut second_payload, 11);

    let mut member1 = vec![0u8; compressed_bound(first_payload.len())];
    let mut member2 = vec![0u8; compressed_bound(second_payload.len())];
    let mut member1_len = member1.len() as u32;
    let mut member2_len = member2.len() as u32;
    unsafe {
        assert_eq!(
            BZ2_bzBuffToBuffCompress(
                member1.as_mut_ptr().cast::<c_char>(),
                &mut member1_len,
                first_payload.as_ptr().cast_mut().cast::<c_char>(),
                first_payload.len() as u32,
                1,
                0,
                0,
            ),
            BZ_OK
        );
        assert_eq!(
            BZ2_bzBuffToBuffCompress(
                member2.as_mut_ptr().cast::<c_char>(),
                &mut member2_len,
                second_payload.as_ptr().cast_mut().cast::<c_char>(),
                second_payload.len() as u32,
                1,
                0,
                0,
            ),
            BZ_OK
        );
    }
    member1.truncate(member1_len as usize);
    member2.truncate(member2_len as usize);

    let concat_path = temp_path("public-api-concat");
    let mut combined = Vec::new();
    combined.extend_from_slice(&member1);
    combined.extend_from_slice(&member2);
    combined.extend_from_slice(trailer);
    fs::write(&concat_path, &combined).unwrap();

    let concat_path_c = CString::new(concat_path.to_string_lossy().as_bytes()).unwrap();
    unsafe {
        let fp = fopen(concat_path_c.as_ptr(), rb.as_ptr());
        assert!(!fp.is_null());

        let mut bzerr = BZ_OK;
        let first = BZ2_bzReadOpen(&mut bzerr, fp, 0, 0, ptr::null_mut(), 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!first.is_null());

        assert_eq!(read_bz_stream(first, first_payload.len()), first_payload);

        let mut unused = ptr::null_mut();
        let mut n_unused = 0;
        BZ2_bzReadGetUnused(&mut bzerr, first, &mut unused, &mut n_unused);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(n_unused as usize, member2.len() + trailer.len());
        let unused_copy = slice::from_raw_parts(unused.cast::<u8>(), n_unused as usize).to_vec();

        BZ2_bzReadClose(&mut bzerr, first);
        assert_eq!(bzerr, BZ_OK);

        let second = BZ2_bzReadOpen(
            &mut bzerr,
            fp,
            0,
            1,
            unused_copy.as_ptr().cast::<c_void>().cast_mut(),
            unused_copy.len() as c_int,
        );
        assert_eq!(bzerr, BZ_OK);
        assert!(!second.is_null());
        assert_eq!(read_bz_stream(second, second_payload.len()), second_payload);

        BZ2_bzReadGetUnused(&mut bzerr, second, &mut unused, &mut n_unused);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(n_unused as usize, trailer.len());
        assert_eq!(
            slice::from_raw_parts(unused.cast::<u8>(), n_unused as usize),
            trailer
        );

        BZ2_bzReadClose(&mut bzerr, second);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(fclose(fp), 0);
    }
    fs::remove_file(&concat_path).unwrap();
}

#[test]
fn ported_stdio_wrappers_match_upstream() {
    let mut source = vec![0u8; WRAPPER_PAYLOAD_LEN];
    fill_payload(&mut source, 13);

    let data_path = temp_path("public-api-wrapper");
    let data_path_c = CString::new(data_path.to_string_lossy().as_bytes()).unwrap();
    let write_mode = CString::new("w7").unwrap();
    let read_mode = CString::new("rs").unwrap();

    unsafe {
        let bzf = BZ2_bzopen(data_path_c.as_ptr(), write_mode.as_ptr());
        assert!(!bzf.is_null());

        let mut total = 0usize;
        while total < source.len() {
            let chunk = (source.len() - total).min(701);
            assert_eq!(
                BZ2_bzwrite(
                    bzf,
                    source.as_ptr().add(total).cast_mut().cast::<c_void>(),
                    chunk as c_int,
                ),
                chunk as c_int
            );
            total += chunk;
        }
        assert_eq!(BZ2_bzflush(bzf), 0);
        BZ2_bzclose(bzf);
    }

    let wrapped_member = fs::read(&data_path).unwrap();
    let mut expected_member = vec![0u8; compressed_bound(source.len())];
    let mut expected_member_len = expected_member.len() as u32;
    unsafe {
        assert_eq!(
            BZ2_bzBuffToBuffCompress(
                expected_member.as_mut_ptr().cast::<c_char>(),
                &mut expected_member_len,
                source.as_ptr().cast_mut().cast::<c_char>(),
                source.len() as u32,
                7,
                0,
                0,
            ),
            BZ_OK
        );
    }
    expected_member.truncate(expected_member_len as usize);
    assert_eq!(wrapped_member, expected_member);

    let fd = std::fs::File::open(&data_path).unwrap().into_raw_fd();
    let bzf = unsafe { BZ2_bzdopen(fd, read_mode.as_ptr()) };
    assert!(!bzf.is_null());

    let mut restored = Vec::with_capacity(WRAPPER_PAYLOAD_LEN);
    let mut scratch = [0u8; 32];
    let mut buf = [0u8; 389];
    loop {
        let nread =
            unsafe { BZ2_bzread(bzf, buf.as_mut_ptr().cast::<c_void>(), buf.len() as c_int) };
        assert!(nread >= 0);
        if nread == 0 {
            break;
        }
        restored.extend_from_slice(&buf[..nread as usize]);
    }
    assert_eq!(restored.len(), WRAPPER_PAYLOAD_LEN);
    assert_eq!(restored, source);
    assert_eq!(
        unsafe {
            BZ2_bzread(
                bzf,
                scratch.as_mut_ptr().cast::<c_void>(),
                scratch.len() as c_int,
            )
        },
        0
    );
    unsafe { BZ2_bzclose(bzf) };
    fs::remove_file(&data_path).unwrap();

    let error_path = temp_path("public-api-wrapper-error");
    let error_path_c = CString::new(error_path.to_string_lossy().as_bytes()).unwrap();
    let error_mode = CString::new("w1").unwrap();
    unsafe {
        let bzf = BZ2_bzopen(error_path_c.as_ptr(), error_mode.as_ptr());
        assert!(!bzf.is_null());
        assert_eq!(
            BZ2_bzread(
                bzf,
                scratch.as_mut_ptr().cast::<c_void>(),
                scratch.len() as c_int
            ),
            -1
        );

        let mut errnum = 0;
        let errstr = CStr::from_ptr(BZ2_bzerror(bzf, &mut errnum))
            .to_str()
            .unwrap();
        assert_eq!(errnum, BZ_SEQUENCE_ERROR);
        assert_eq!(errstr, "SEQUENCE_ERROR");
        BZ2_bzclose(bzf);
    }
    fs::remove_file(&error_path).unwrap();
}
