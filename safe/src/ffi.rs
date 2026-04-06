use crate::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use crate::constants::{
    BZ_FINISH, BZ_FINISH_OK, BZ_OK, BZ_OUTBUFF_FULL, BZ_PARAM_ERROR, BZ_STREAM_END,
    BZ_UNEXPECTED_EOF, BZ_VERSION_BYTES,
};
use crate::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use crate::types::{bz_stream, CFile};
use core::mem::MaybeUninit;
use std::os::raw::{c_char, c_int, c_uint};

unsafe extern "C" {
    fn fprintf(stream: *mut CFile, format: *const c_char, ...) -> c_int;
    fn fputs(s: *const c_char, stream: *mut CFile) -> c_int;
    fn exit(status: c_int) -> !;
    static mut stderr: *mut CFile;
}

static bzerrorstrings: [&[u8]; 16] = [
    b"OK\0",
    b"SEQUENCE_ERROR\0",
    b"PARAM_ERROR\0",
    b"MEM_ERROR\0",
    b"DATA_ERROR\0",
    b"DATA_ERROR_MAGIC\0",
    b"IO_ERROR\0",
    b"UNEXPECTED_EOF\0",
    b"OUTBUFF_FULL\0",
    b"CONFIG_ERROR\0",
    b"???\0",
    b"???\0",
    b"???\0",
    b"???\0",
    b"???\0",
    b"???\0",
];

static ASSERT_H_FORMAT: &[u8] = b"\n\nbzip2/libbzip2: internal error number %d.\nThis is a bug in bzip2/libbzip2, %s.\nPlease report it to: bzip2-devel@sourceware.org.  If this happened\nwhen you were using some program which uses libbzip2 as a\ncomponent, you should also report this bug to the author(s)\nof that program.  Please make an effort to report this bug;\ntimely and accurate bug reports eventually lead to higher\nquality software.  Thanks.\n\n\0";
static ASSERT_H_1007_NOTE: &[u8] = b"\n*** A special note about internal error number 1007 ***\n\nExperience suggests that a common cause of i.e. 1007\nis unreliable memory or other hardware.  The 1007 assertion\njust happens to cross-check the results of huge numbers of\nmemory reads/writes, and so acts (unintendedly) as a stress\ntest of your memory system.\n\nI suggest the following: try compressing the file again,\npossibly monitoring progress in detail with the -vv flag.\n\n* If the error cannot be reproduced, and/or happens at different\n  points in compression, you may have a flaky memory system.\n  Try a memory-test program.  I have used Memtest86\n  (www.memtest86.com).  At the time of writing it is free (GPLd).\n  Memtest86 tests memory much more thorougly than your BIOSs\n  power-on test, and may find failures that the BIOS doesn't.\n\n* If the error can be repeatably reproduced, this is a bug in\n  bzip2, and I would very much like to hear about it.  Please\n  let me know, and, ideally, save a copy of the file causing the\n  problem -- without which I will be unable to investigate it.\n\n\0";

pub(crate) unsafe fn set_error_slot(slot: *mut c_int, code: c_int) {
    if !slot.is_null() {
        *slot = code;
    }
}

pub(crate) fn bzerror_message_ptr(code: c_int) -> *const c_char {
    let index = if code > 0 {
        0usize
    } else {
        usize::try_from(code.wrapping_neg())
            .ok()
            .filter(|index| *index < bzerrorstrings.len())
            .unwrap_or(bzerrorstrings.len() - 1)
    };
    bzerrorstrings[index].as_ptr().cast()
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
    unsafe {
        let _ = fprintf(
            stderr,
            ASSERT_H_FORMAT.as_ptr().cast(),
            errcode,
            BZ2_bzlibVersion(),
        );
        if errcode == 1007 {
            let _ = fputs(ASSERT_H_1007_NOTE.as_ptr().cast(), stderr);
        }
        exit(3);
    }
}
