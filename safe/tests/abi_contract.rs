use bz2::bz_stream;
use bz2::compress::{BZ2_bzCompressEnd, BZ2_bzCompressInit};
use bz2::constants::{BZ_OK, BZ_SEQUENCE_ERROR, BZ_STREAM_END};
use bz2::crc::BZ2_crc32Table;
use bz2::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use bz2::ffi::{BZ2_bz__AssertH__fail, BZ2_bzlibVersion};
use bz2::rand::BZ2_rNums;
use bz2::stdio::{
    BZ2_bzReadClose, BZ2_bzReadOpen, BZ2_bzWriteClose, BZ2_bzWriteClose64, BZ2_bzflush,
};
use bz2::types::{bz_stream as c_bz_stream, CFile};
use std::collections::HashSet;
use std::env;
use std::ffi::{c_void, CStr, CString};
use std::mem::{offset_of, size_of, MaybeUninit};
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::process::Command;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const SAMPLE1_BZ2: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample1.bz2"
));
const SAMPLE1_REF: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../original/sample1.ref"
));

static LIVE_ALLOCATIONS: LazyLock<Mutex<HashSet<usize>>> =
    LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Default)]
struct AllocStats {
    allocs: AtomicUsize,
    frees: AtomicUsize,
}

extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
    fn fclose(file: *mut CFile) -> c_int;
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}

unsafe extern "C" fn tracking_alloc(opaque: *mut c_void, items: c_int, size: c_int) -> *mut c_void {
    let stats = &*(opaque.cast::<AllocStats>());
    if items < 0 || size < 0 {
        return ptr::null_mut();
    }

    let Ok(items) = usize::try_from(items) else {
        return ptr::null_mut();
    };
    let Ok(size) = usize::try_from(size) else {
        return ptr::null_mut();
    };
    let Some(bytes) = items.checked_mul(size) else {
        return ptr::null_mut();
    };

    let raw = malloc(bytes);
    if !raw.is_null() {
        stats.allocs.fetch_add(1, Ordering::SeqCst);
        LIVE_ALLOCATIONS.lock().unwrap().insert(raw as usize);
    }
    raw
}

unsafe extern "C" fn tracking_free(opaque: *mut c_void, addr: *mut c_void) {
    if addr.is_null() {
        return;
    }

    let stats = &*(opaque.cast::<AllocStats>());
    let removed = LIVE_ALLOCATIONS.lock().unwrap().remove(&(addr as usize));
    assert!(removed, "allocator callback freed an unknown pointer");
    stats.frees.fetch_add(1, Ordering::SeqCst);
    free(addr);
}

fn zeroed_stream() -> bz_stream {
    unsafe { MaybeUninit::<bz_stream>::zeroed().assume_init() }
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

fn align_up(value: usize, align: usize) -> usize {
    value.next_multiple_of(align)
}

fn strip_c_block_comments(contents: &str) -> String {
    let mut cleaned = String::with_capacity(contents.len());
    let mut rest = contents;
    loop {
        let Some(start) = rest.find("/*") else {
            cleaned.push_str(rest);
            return cleaned;
        };
        cleaned.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("*/") else {
            return cleaned;
        };
        rest = &after_start[end + 2..];
    }
}

fn parse_c_array_u32(contents: &str) -> Vec<u32> {
    let cleaned = strip_c_block_comments(contents);
    let body = cleaned
        .split_once('{')
        .unwrap()
        .1
        .rsplit_once("};")
        .unwrap()
        .0;

    body.split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| {
            let token = token.trim_end_matches(';').trim_end_matches('L');
            if let Some(hex) = token.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).unwrap()
            } else {
                token.parse::<u32>().unwrap()
            }
        })
        .collect()
}

fn parse_c_array_i32(contents: &str) -> Vec<i32> {
    let cleaned = strip_c_block_comments(contents);
    let body = cleaned
        .split_once('{')
        .unwrap()
        .1
        .rsplit_once("};")
        .unwrap()
        .0;

    body.split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.trim_end_matches(';').parse::<i32>().unwrap())
        .collect()
}

#[test]
fn bz_stream_layout_matches_original_header() {
    let ptr = size_of::<*mut c_void>();
    let uint = size_of::<u32>();
    let fn_ptr = size_of::<unsafe extern "C" fn()>();

    let mut expected = 0usize;
    assert_eq!(offset_of!(c_bz_stream, next_in), expected);
    expected += ptr;
    assert_eq!(offset_of!(c_bz_stream, avail_in), expected);
    expected += uint;
    assert_eq!(offset_of!(c_bz_stream, total_in_lo32), expected);
    expected += uint;
    assert_eq!(offset_of!(c_bz_stream, total_in_hi32), expected);
    expected += uint;

    expected = align_up(expected, ptr);
    assert_eq!(offset_of!(c_bz_stream, next_out), expected);
    expected += ptr;
    assert_eq!(offset_of!(c_bz_stream, avail_out), expected);
    expected += uint;
    assert_eq!(offset_of!(c_bz_stream, total_out_lo32), expected);
    expected += uint;
    assert_eq!(offset_of!(c_bz_stream, total_out_hi32), expected);
    expected += uint;

    expected = align_up(expected, ptr);
    assert_eq!(offset_of!(c_bz_stream, state), expected);
    expected += ptr;
    assert_eq!(offset_of!(c_bz_stream, bzalloc), expected);
    expected += fn_ptr;
    assert_eq!(offset_of!(c_bz_stream, bzfree), expected);
    expected += fn_ptr;
    assert_eq!(offset_of!(c_bz_stream, opaque), expected);
    expected += ptr;

    assert_eq!(size_of::<c_bz_stream>(), align_up(expected, ptr));
    assert_eq!(size_of::<c_int>(), 4);
    assert_eq!(size_of::<i16>(), 2);
    assert_eq!(size_of::<c_char>(), 1);
}

#[test]
fn exported_tables_match_original_c_fixtures() {
    let expected_crc = parse_c_array_u32(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../original/crctable.c"
    )));
    let expected_rand = parse_c_array_i32(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../original/randtable.c"
    )));

    let actual_crc = unsafe { std::ptr::addr_of!(BZ2_crc32Table).read().to_vec() };
    let actual_rand = unsafe { std::ptr::addr_of!(BZ2_rNums).read().to_vec() };

    assert_eq!(actual_crc.len(), 256);
    assert_eq!(actual_crc, expected_crc);
    assert_eq!(actual_rand.len(), 512);
    assert_eq!(actual_rand, expected_rand);
}

#[test]
fn custom_bzalloc_and_bzfree_back_the_core_stream_state() {
    LIVE_ALLOCATIONS.lock().unwrap().clear();
    let stats = AllocStats::default();
    let mut compress = zeroed_stream();
    compress.bzalloc = Some(tracking_alloc);
    compress.bzfree = Some(tracking_free);
    compress.opaque = (&stats as *const AllocStats).cast_mut().cast::<c_void>();

    unsafe {
        assert_eq!(BZ2_bzCompressInit(&mut compress, 1, 0, 30), BZ_OK);
        assert_eq!(stats.allocs.load(Ordering::SeqCst), 4);
        assert_eq!(BZ2_bzCompressEnd(&mut compress), BZ_OK);
        assert_eq!(stats.frees.load(Ordering::SeqCst), 4);
    }

    let stats = AllocStats::default();
    let mut decompress = zeroed_stream();
    decompress.bzalloc = Some(tracking_alloc);
    decompress.bzfree = Some(tracking_free);
    decompress.opaque = (&stats as *const AllocStats).cast_mut().cast::<c_void>();

    unsafe {
        assert_eq!(BZ2_bzDecompressInit(&mut decompress, 0, 0), BZ_OK);
        decompress.next_in = SAMPLE1_BZ2.as_ptr().cast_mut().cast::<c_char>();
        decompress.avail_in = SAMPLE1_BZ2.len() as u32;

        let mut output = vec![0u8; SAMPLE1_REF.len()];
        decompress.next_out = output.as_mut_ptr().cast::<c_char>();
        decompress.avail_out = output.len() as u32;

        assert_eq!(BZ2_bzDecompress(&mut decompress), BZ_STREAM_END);
        assert_eq!(output, SAMPLE1_REF);
        assert!(stats.allocs.load(Ordering::SeqCst) >= 2);
        assert_eq!(BZ2_bzDecompressEnd(&mut decompress), BZ_OK);
        assert_eq!(
            stats.allocs.load(Ordering::SeqCst),
            stats.frees.load(Ordering::SeqCst)
        );
    }
}

#[test]
fn bzwriteclose_preserves_output_pointers_on_early_return() {
    unsafe {
        let mut bzerr = 1234;
        let mut in_lo = 0x1111_1111u32;
        let mut in_hi = 0x2222_2222u32;
        let mut out_lo = 0x3333_3333u32;
        let mut out_hi = 0x4444_4444u32;

        BZ2_bzWriteClose64(
            &mut bzerr,
            ptr::null_mut(),
            0,
            &mut in_lo,
            &mut in_hi,
            &mut out_lo,
            &mut out_hi,
        );
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(in_lo, 0x1111_1111);
        assert_eq!(in_hi, 0x2222_2222);
        assert_eq!(out_lo, 0x3333_3333);
        assert_eq!(out_hi, 0x4444_4444);

        let mut short_in = 0x5555_5555u32;
        let mut short_out = 0x6666_6666u32;
        BZ2_bzWriteClose(
            &mut bzerr,
            ptr::null_mut(),
            0,
            &mut short_in,
            &mut short_out,
        );
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(short_in, 0x5555_5555);
        assert_eq!(short_out, 0x6666_6666);
    }

    let path = temp_path("bzwriteclose-sequence");
    std::fs::write(&path, SAMPLE1_BZ2).unwrap();
    let path_c = CString::new(path.to_string_lossy().as_bytes()).unwrap();
    let rb = CString::new("rb").unwrap();

    unsafe {
        let fp = fopen(path_c.as_ptr(), rb.as_ptr());
        assert!(!fp.is_null());

        let mut bzerr = BZ_OK;
        let bzf = BZ2_bzReadOpen(&mut bzerr, fp, 0, 0, ptr::null_mut(), 0);
        assert_eq!(bzerr, BZ_OK);
        assert!(!bzf.is_null());

        let mut in_lo = 7u32;
        let mut in_hi = 8u32;
        let mut out_lo = 9u32;
        let mut out_hi = 10u32;
        BZ2_bzWriteClose64(
            &mut bzerr,
            bzf,
            0,
            &mut in_lo,
            &mut in_hi,
            &mut out_lo,
            &mut out_hi,
        );
        assert_eq!(bzerr, BZ_SEQUENCE_ERROR);
        assert_eq!(in_lo, 7);
        assert_eq!(in_hi, 8);
        assert_eq!(out_lo, 9);
        assert_eq!(out_hi, 10);

        BZ2_bzReadClose(&mut bzerr, bzf);
        assert_eq!(bzerr, BZ_OK);
        assert_eq!(fclose(fp), 0);
    }
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn bzflush_and_version_stay_in_wrapper_compat_mode() {
    let version = unsafe { CStr::from_ptr(BZ2_bzlibVersion()) }
        .to_str()
        .unwrap();
    assert_eq!(version, "1.0.8, 13-Jul-2019");
    assert_eq!(unsafe { BZ2_bzflush(ptr::null_mut()) }, 0);
}

#[test]
fn assert_fail_emits_original_text_and_exits_with_code_three() {
    let output = Command::new(env::current_exe().unwrap())
        .arg("--exact")
        .arg("assert_fail_child_process")
        .arg("--nocapture")
        .env("LIBBZ2_ASSERT_CHILD", "1")
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("bzip2/libbzip2: internal error number 1007."));
    assert!(stderr.contains("Please report it to: bzip2-devel@sourceware.org."));
    assert!(stderr.contains("*** A special note about internal error number 1007 ***"));
}

#[test]
fn assert_fail_child_process() {
    if env::var_os("LIBBZ2_ASSERT_CHILD").is_some() {
        BZ2_bz__AssertH__fail(1007);
    }
}
