use crate::constants::{
    BZ_CONFIG_ERROR, BZ_DATA_ERROR, BZ_DATA_ERROR_MAGIC, BZ_FINISH_OK, BZ_IO_ERROR, BZ_MEM_ERROR,
    BZ_OK, BZ_OUTBUFF_FULL, BZ_PARAM_ERROR, BZ_RUN_OK, BZ_SEQUENCE_ERROR, BZ_STREAM_END,
    BZ_UNEXPECTED_EOF, BZ_VERSION_BYTES,
};
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
    _dest: *mut c_char,
    destLen: *mut c_uint,
    _source: *mut c_char,
    _sourceLen: c_uint,
    _blockSize100k: c_int,
    _verbosity: c_int,
    _workFactor: c_int,
) -> c_int {
    if !destLen.is_null() {
        *destLen = 0;
    }
    BZ_CONFIG_ERROR
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzBuffToBuffDecompress(
    _dest: *mut c_char,
    destLen: *mut c_uint,
    _source: *mut c_char,
    _sourceLen: c_uint,
    _small: c_int,
    _verbosity: c_int,
) -> c_int {
    if !destLen.is_null() {
        *destLen = 0;
    }
    BZ_CONFIG_ERROR
}

#[no_mangle]
pub extern "C" fn BZ2_bz__AssertH__fail(errcode: c_int) {
    eprintln!("libbz2-safe internal assertion failure: {}", errcode);
    process::abort();
}
