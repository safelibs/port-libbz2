use crate::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use crate::constants::{
    BZ_DATA_ERROR, BZ_DATA_ERROR_MAGIC, BZ_FINISH, BZ_FINISH_OK, BZ_IO_ERROR, BZ_MEM_ERROR, BZ_OK,
    BZ_OUTBUFF_FULL, BZ_PARAM_ERROR, BZ_RUN_OK, BZ_SEQUENCE_ERROR, BZ_STREAM_END,
    BZ_UNEXPECTED_EOF, BZ_VERSION_BYTES,
};
use crate::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use crate::types::bz_stream;
use core::mem::MaybeUninit;
use std::os::raw::{c_char, c_int, c_uint};
use std::process;

static BZ_OK_MSG: &[u8] = b"OK\0";
static BZ_SEQUENCE_ERROR_MSG: &[u8] = b"SEQUENCE_ERROR\0";
static BZ_PARAM_ERROR_MSG: &[u8] = b"PARAM_ERROR\0";
static BZ_MEM_ERROR_MSG: &[u8] = b"MEM_ERROR\0";
static BZ_DATA_ERROR_MSG: &[u8] = b"DATA_ERROR\0";
static BZ_DATA_ERROR_MAGIC_MSG: &[u8] = b"DATA_ERROR_MAGIC\0";
static BZ_IO_ERROR_MSG: &[u8] = b"IO_ERROR\0";
static BZ_UNEXPECTED_EOF_MSG: &[u8] = b"UNEXPECTED_EOF\0";
static BZ_OUTBUFF_FULL_MSG: &[u8] = b"OUTBUFF_FULL\0";
static BZ_CONFIG_ERROR_MSG: &[u8] = b"CONFIG_ERROR\0";
static BZ_STREAM_END_MSG: &[u8] = b"STREAM_END\0";
static BZ_RUN_OK_MSG: &[u8] = b"RUN_OK\0";
static BZ_FINISH_OK_MSG: &[u8] = b"FINISH_OK\0";

pub(crate) unsafe fn set_error_slot(slot: *mut c_int, code: c_int) {
    if !slot.is_null() {
        *slot = code;
    }
}

pub(crate) fn bzerror_message_ptr(code: c_int) -> *const c_char {
    match code {
        BZ_OK => BZ_OK_MSG.as_ptr().cast(),
        BZ_RUN_OK => BZ_RUN_OK_MSG.as_ptr().cast(),
        BZ_FINISH_OK => BZ_FINISH_OK_MSG.as_ptr().cast(),
        BZ_STREAM_END => BZ_STREAM_END_MSG.as_ptr().cast(),
        BZ_SEQUENCE_ERROR => BZ_SEQUENCE_ERROR_MSG.as_ptr().cast(),
        BZ_PARAM_ERROR => BZ_PARAM_ERROR_MSG.as_ptr().cast(),
        BZ_MEM_ERROR => BZ_MEM_ERROR_MSG.as_ptr().cast(),
        BZ_DATA_ERROR => BZ_DATA_ERROR_MSG.as_ptr().cast(),
        BZ_DATA_ERROR_MAGIC => BZ_DATA_ERROR_MAGIC_MSG.as_ptr().cast(),
        BZ_IO_ERROR => BZ_IO_ERROR_MSG.as_ptr().cast(),
        BZ_UNEXPECTED_EOF => BZ_UNEXPECTED_EOF_MSG.as_ptr().cast(),
        BZ_OUTBUFF_FULL => BZ_OUTBUFF_FULL_MSG.as_ptr().cast(),
        _ => BZ_CONFIG_ERROR_MSG.as_ptr().cast(),
    }
}

#[no_mangle]
pub extern "C" fn BZ2_bzlibVersion() -> *const c_char {
    BZ_VERSION_BYTES.as_ptr().cast()
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzBuffToBuffCompress(
    dest: *mut c_char,
    destLen: *mut c_uint,
    source: *mut c_char,
    sourceLen: c_uint,
    blockSize100k: c_int,
    verbosity: c_int,
    workFactor: c_int,
) -> c_int {
    if dest.is_null()
        || destLen.is_null()
        || source.is_null()
        || !(1..=9).contains(&blockSize100k)
        || !(0..=4).contains(&verbosity)
        || !(0..=250).contains(&workFactor)
    {
        return BZ_PARAM_ERROR;
    }

    let mut strm: bz_stream = MaybeUninit::zeroed().assume_init();
    strm.bzalloc = None;
    strm.bzfree = None;
    strm.opaque = core::ptr::null_mut();

    let ret = BZ2_bzCompressInit(&mut strm, blockSize100k, verbosity, workFactor);
    if ret != BZ_OK {
        return ret;
    }

    strm.next_in = source;
    strm.next_out = dest;
    strm.avail_in = sourceLen;
    strm.avail_out = *destLen;

    let ret = BZ2_bzCompress(&mut strm, BZ_FINISH);
    if ret == BZ_FINISH_OK {
        let _ = BZ2_bzCompressEnd(&mut strm);
        return BZ_OUTBUFF_FULL;
    }
    if ret != BZ_STREAM_END {
        let _ = BZ2_bzCompressEnd(&mut strm);
        return ret;
    }

    *destLen -= strm.avail_out;
    let _ = BZ2_bzCompressEnd(&mut strm);
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzBuffToBuffDecompress(
    dest: *mut c_char,
    destLen: *mut c_uint,
    source: *mut c_char,
    sourceLen: c_uint,
    small: c_int,
    verbosity: c_int,
) -> c_int {
    if dest.is_null()
        || destLen.is_null()
        || source.is_null()
        || (small != 0 && small != 1)
        || !(0..=4).contains(&verbosity)
    {
        return BZ_PARAM_ERROR;
    }

    let mut strm: bz_stream = MaybeUninit::zeroed().assume_init();
    let requested_len = *destLen;
    let ret = BZ2_bzDecompressInit(&mut strm, verbosity, small);
    if ret != BZ_OK {
        return ret;
    }

    strm.next_in = source;
    strm.next_out = dest;
    strm.avail_in = sourceLen;
    strm.avail_out = requested_len;

    let ret = BZ2_bzDecompress(&mut strm);
    if ret == BZ_STREAM_END {
        *destLen = requested_len - strm.avail_out;
        let _ = BZ2_bzDecompressEnd(&mut strm);
        BZ_OK
    } else if ret == BZ_OK {
        let _ = BZ2_bzDecompressEnd(&mut strm);
        if strm.avail_out > 0 {
            BZ_UNEXPECTED_EOF
        } else {
            BZ_OUTBUFF_FULL
        }
    } else {
        let _ = BZ2_bzDecompressEnd(&mut strm);
        ret
    }
}

#[no_mangle]
pub extern "C" fn BZ2_bz__AssertH__fail(errcode: c_int) {
    eprintln!("libbz2-safe internal assertion failure: {}", errcode);
    process::abort();
}
