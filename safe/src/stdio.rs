use crate::compress::{BZ2_bzCompress, BZ2_bzCompressEnd, BZ2_bzCompressInit};
use crate::constants::{
    BZ_IO_ERROR, BZ_MAX_UNUSED, BZ_OK, BZ_PARAM_ERROR, BZ_RUN, BZ_RUN_OK, BZ_SEQUENCE_ERROR,
    BZ_STREAM_END, BZ_UNEXPECTED_EOF,
};
use crate::decompress::{BZ2_bzDecompress, BZ2_bzDecompressEnd, BZ2_bzDecompressInit};
use crate::ffi::{bzerror_message_ptr, set_error_slot};
use crate::types::{bzfile_from_handle, BzFileState, CFile};
use core::ffi::c_void;
use core::mem::MaybeUninit;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::ptr;

const EOF_VALUE: c_int = -1;

extern "C" {
    fn fdopen(fd: c_int, mode: *const c_char) -> *mut CFile;
    fn fclose(file: *mut CFile) -> c_int;
    fn fflush(file: *mut CFile) -> c_int;
    fn fread(ptr: *mut c_void, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut CFile) -> usize;
    fn ferror(stream: *mut CFile) -> c_int;
    fn fgetc(stream: *mut CFile) -> c_int;
    fn ungetc(c: c_int, stream: *mut CFile) -> c_int;
    static mut stdin: *mut CFile;
    static mut stdout: *mut CFile;
}

#[cfg(any(target_os = "linux", target_os = "android"))]
extern "C" {
    fn fopen64(path: *const c_char, mode: *const c_char) -> *mut CFile;
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut CFile;
}

unsafe fn zeroed_bzfile() -> Box<BzFileState> {
    Box::new(MaybeUninit::<BzFileState>::zeroed().assume_init())
}

unsafe fn state_from_handle(handle: *mut c_void) -> *mut BzFileState {
    bzfile_from_handle(handle)
}

unsafe fn set_bzerror(bzerror: *mut c_int, bzf: *mut BzFileState, code: c_int) {
    set_error_slot(bzerror, code);
    if !bzf.is_null() {
        (*bzf).lastErr = code;
    }
}

unsafe fn make_handle(handle: *mut CFile, writing: bool) -> *mut c_void {
    let mut state = zeroed_bzfile();
    state.handle = handle;
    state.writing = if writing { 1 } else { 0 };
    state.lastErr = BZ_OK;
    Box::into_raw(state).cast()
}

unsafe fn myfeof(file: *mut CFile) -> bool {
    let ch = fgetc(file);
    if ch == EOF_VALUE {
        return true;
    }
    let _ = ungetc(ch, file);
    false
}

unsafe fn should_close_handle(handle: *mut CFile) -> bool {
    handle != stdin && handle != stdout
}

unsafe fn parse_open_mode(mode: *const c_char) -> Option<(bool, c_int, bool)> {
    if mode.is_null() {
        return None;
    }
    let mode = CStr::from_ptr(mode).to_bytes();
    let mut writing = false;
    let mut block_size_100k = 9;
    let mut small_mode = false;

    for &byte in mode {
        match byte {
            b'r' => writing = false,
            b'w' => writing = true,
            b's' => small_mode = true,
            b'0'..=b'9' => block_size_100k = c_int::from(byte - b'0'),
            _ => {}
        }
    }

    Some((writing, block_size_100k, small_mode))
}

unsafe fn open_path_binary(path: *const c_char, mode: *const c_char) -> *mut CFile {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        fopen64(path, mode)
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        fopen(path, mode)
    }
}

unsafe fn bzopen_or_bzdopen(
    path: *const c_char,
    fd: c_int,
    mode: *const c_char,
    by_fd: bool,
) -> *mut c_void {
    let (writing, mut block_size_100k, small_mode) = match parse_open_mode(mode) {
        Some(parsed) => parsed,
        None => return ptr::null_mut(),
    };
    if writing {
        block_size_100k = block_size_100k.clamp(1, 9);
    }

    let mode2 = match CString::new(if writing { "wb" } else { "rb" }) {
        Ok(value) => value,
        Err(_) => return ptr::null_mut(),
    };
    let file = if by_fd {
        fdopen(fd, mode2.as_ptr())
    } else if path.is_null() || *path == 0 {
        if writing {
            stdout
        } else {
            stdin
        }
    } else {
        open_path_binary(path, mode2.as_ptr())
    };

    if file.is_null() {
        return ptr::null_mut();
    }

    let mut bzerr = BZ_OK;
    let handle = if writing {
        BZ2_bzWriteOpen(&mut bzerr, file, block_size_100k, 0, 30)
    } else {
        BZ2_bzReadOpen(
            &mut bzerr,
            file,
            0,
            if small_mode { 1 } else { 0 },
            ptr::null_mut(),
            0,
        )
    };

    if handle.is_null() {
        if should_close_handle(file) {
            let _ = fclose(file);
        }
        return ptr::null_mut();
    }
    handle
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzReadOpen(
    bzerror: *mut c_int,
    f: *mut CFile,
    verbosity: c_int,
    small: c_int,
    unused: *mut c_void,
    nUnused: c_int,
) -> *mut c_void {
    if f.is_null()
        || (small != 0 && small != 1)
        || !(0..=4).contains(&verbosity)
        || (unused.is_null() && nUnused != 0)
        || (!unused.is_null() && !(0..=BZ_MAX_UNUSED).contains(&nUnused))
    {
        set_bzerror(bzerror, ptr::null_mut(), BZ_PARAM_ERROR);
        return ptr::null_mut();
    }
    if ferror(f) != 0 {
        set_bzerror(bzerror, ptr::null_mut(), crate::constants::BZ_IO_ERROR);
        return ptr::null_mut();
    }

    let handle = make_handle(f, false);
    let bzf = state_from_handle(handle);
    (*bzf).initialisedOk = 0;
    (*bzf).bufN = 0;

    if nUnused > 0 {
        ptr::copy_nonoverlapping(
            unused.cast::<u8>(),
            (*bzf).buf.as_mut_ptr().cast::<u8>(),
            nUnused as usize,
        );
        (*bzf).bufN = nUnused;
    }

    let ret = BZ2_bzDecompressInit(&mut (*bzf).strm, verbosity, small);
    if ret != BZ_OK {
        set_bzerror(bzerror, bzf, ret);
        drop(Box::from_raw(bzf));
        return ptr::null_mut();
    }

    (*bzf).strm.avail_in = (*bzf).bufN as u32;
    (*bzf).strm.next_in = (*bzf).buf.as_mut_ptr();
    (*bzf).initialisedOk = 1;
    set_bzerror(bzerror, bzf, BZ_OK);
    handle
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzReadClose(bzerror: *mut c_int, b: *mut c_void) {
    if b.is_null() {
        set_error_slot(bzerror, BZ_OK);
        return;
    }
    let bzf = state_from_handle(b);
    if (*bzf).writing != 0 {
        set_bzerror(bzerror, bzf, BZ_SEQUENCE_ERROR);
        return;
    }
    if (*bzf).initialisedOk != 0 {
        let _ = BZ2_bzDecompressEnd(&mut (*bzf).strm);
    }
    set_error_slot(bzerror, BZ_OK);
    drop(Box::from_raw(bzf));
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzRead(
    bzerror: *mut c_int,
    b: *mut c_void,
    buf: *mut c_void,
    len: c_int,
) -> c_int {
    if b.is_null() || buf.is_null() || len < 0 {
        set_bzerror(bzerror, ptr::null_mut(), BZ_PARAM_ERROR);
        return 0;
    }

    let bzf = state_from_handle(b);
    if (*bzf).writing != 0 {
        set_bzerror(bzerror, bzf, BZ_SEQUENCE_ERROR);
        return 0;
    }
    if len == 0 {
        set_bzerror(bzerror, bzf, BZ_OK);
        return 0;
    }

    (*bzf).strm.avail_out = len as u32;
    (*bzf).strm.next_out = buf.cast();

    loop {
        if ferror((*bzf).handle) != 0 {
            set_bzerror(bzerror, bzf, crate::constants::BZ_IO_ERROR);
            return 0;
        }

        if (*bzf).strm.avail_in == 0 && !myfeof((*bzf).handle) {
            let n = fread(
                (*bzf).buf.as_mut_ptr().cast(),
                1,
                BZ_MAX_UNUSED as usize,
                (*bzf).handle,
            );
            if ferror((*bzf).handle) != 0 {
                set_bzerror(bzerror, bzf, crate::constants::BZ_IO_ERROR);
                return 0;
            }
            (*bzf).bufN = n as c_int;
            (*bzf).strm.avail_in = n as u32;
            (*bzf).strm.next_in = (*bzf).buf.as_mut_ptr();
        }

        let ret = BZ2_bzDecompress(&mut (*bzf).strm);
        if ret != BZ_OK && ret != BZ_STREAM_END {
            set_bzerror(bzerror, bzf, ret);
            return 0;
        }

        if ret == BZ_OK
            && myfeof((*bzf).handle)
            && (*bzf).strm.avail_in == 0
            && (*bzf).strm.avail_out > 0
        {
            set_bzerror(bzerror, bzf, BZ_UNEXPECTED_EOF);
            return 0;
        }

        if ret == BZ_STREAM_END {
            set_bzerror(bzerror, bzf, BZ_STREAM_END);
            return len - (*bzf).strm.avail_out as c_int;
        }
        if (*bzf).strm.avail_out == 0 {
            set_bzerror(bzerror, bzf, BZ_OK);
            return len;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzReadGetUnused(
    bzerror: *mut c_int,
    b: *mut c_void,
    unused: *mut *mut c_void,
    nUnused: *mut c_int,
) {
    if b.is_null() {
        set_bzerror(bzerror, ptr::null_mut(), BZ_PARAM_ERROR);
        return;
    }
    let bzf = state_from_handle(b);
    if (*bzf).lastErr != BZ_STREAM_END {
        set_bzerror(bzerror, bzf, BZ_SEQUENCE_ERROR);
        return;
    }
    if unused.is_null() || nUnused.is_null() {
        set_bzerror(bzerror, bzf, BZ_PARAM_ERROR);
        return;
    }

    *nUnused = (*bzf).strm.avail_in as c_int;
    *unused = (*bzf).strm.next_in.cast();
    set_bzerror(bzerror, bzf, BZ_OK);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWriteOpen(
    bzerror: *mut c_int,
    f: *mut CFile,
    blockSize100k: c_int,
    verbosity: c_int,
    workFactor: c_int,
) -> *mut c_void {
    if f.is_null()
        || !(1..=9).contains(&blockSize100k)
        || !(0..=4).contains(&verbosity)
        || !(0..=250).contains(&workFactor)
    {
        set_bzerror(bzerror, ptr::null_mut(), BZ_PARAM_ERROR);
        return ptr::null_mut();
    }

    if ferror(f) != 0 {
        set_bzerror(bzerror, ptr::null_mut(), BZ_IO_ERROR);
        return ptr::null_mut();
    }

    let handle = make_handle(f, true);
    let bzf = state_from_handle(handle);
    (*bzf).initialisedOk = 0;
    (*bzf).bufN = 0;
    (*bzf).strm.bzalloc = None;
    (*bzf).strm.bzfree = None;
    (*bzf).strm.opaque = ptr::null_mut();

    let mut work_factor = workFactor;
    if work_factor == 0 {
        work_factor = 30;
    }
    let ret = BZ2_bzCompressInit(&mut (*bzf).strm, blockSize100k, verbosity, work_factor);
    if ret != BZ_OK {
        set_bzerror(bzerror, bzf, ret);
        drop(Box::from_raw(bzf));
        return ptr::null_mut();
    }

    (*bzf).strm.avail_in = 0;
    (*bzf).initialisedOk = 1;
    set_bzerror(bzerror, bzf, BZ_OK);
    handle
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWrite(
    bzerror: *mut c_int,
    b: *mut c_void,
    buf: *mut c_void,
    len: c_int,
) {
    if b.is_null() || buf.is_null() || len < 0 {
        set_bzerror(bzerror, ptr::null_mut(), BZ_PARAM_ERROR);
        return;
    }

    let bzf = state_from_handle(b);
    if (*bzf).writing == 0 {
        set_bzerror(bzerror, bzf, BZ_SEQUENCE_ERROR);
        return;
    }
    if ferror((*bzf).handle) != 0 {
        set_bzerror(bzerror, bzf, BZ_IO_ERROR);
        return;
    }
    if len == 0 {
        set_bzerror(bzerror, bzf, BZ_OK);
        return;
    }

    (*bzf).strm.avail_in = len as u32;
    (*bzf).strm.next_in = buf.cast();

    loop {
        (*bzf).strm.avail_out = BZ_MAX_UNUSED as u32;
        (*bzf).strm.next_out = (*bzf).buf.as_mut_ptr();
        let ret = BZ2_bzCompress(&mut (*bzf).strm, BZ_RUN);
        if ret != BZ_RUN_OK {
            set_bzerror(bzerror, bzf, ret);
            return;
        }

        if (*bzf).strm.avail_out < BZ_MAX_UNUSED as u32 {
            let n = BZ_MAX_UNUSED as usize - (*bzf).strm.avail_out as usize;
            let written = fwrite((*bzf).buf.as_ptr().cast::<c_void>(), 1, n, (*bzf).handle);
            if written != n || ferror((*bzf).handle) != 0 {
                set_bzerror(bzerror, bzf, BZ_IO_ERROR);
                return;
            }
        }

        if (*bzf).strm.avail_in == 0 {
            set_bzerror(bzerror, bzf, BZ_OK);
            return;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWriteClose(
    bzerror: *mut c_int,
    b: *mut c_void,
    abandon: c_int,
    nbytes_in: *mut u32,
    nbytes_out: *mut u32,
) {
    BZ2_bzWriteClose64(
        bzerror,
        b,
        abandon,
        nbytes_in,
        ptr::null_mut(),
        nbytes_out,
        ptr::null_mut(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWriteClose64(
    bzerror: *mut c_int,
    b: *mut c_void,
    abandon: c_int,
    nbytes_in_lo32: *mut u32,
    nbytes_in_hi32: *mut u32,
    nbytes_out_lo32: *mut u32,
    nbytes_out_hi32: *mut u32,
) {
    if b.is_null() {
        set_error_slot(bzerror, BZ_OK);
        return;
    }
    let bzf = state_from_handle(b);
    if (*bzf).writing == 0 {
        set_bzerror(bzerror, bzf, BZ_SEQUENCE_ERROR);
        return;
    }
    if ferror((*bzf).handle) != 0 {
        set_bzerror(bzerror, bzf, BZ_IO_ERROR);
        return;
    }

    if !nbytes_in_lo32.is_null() {
        *nbytes_in_lo32 = 0;
    }
    if !nbytes_in_hi32.is_null() {
        *nbytes_in_hi32 = 0;
    }
    if !nbytes_out_lo32.is_null() {
        *nbytes_out_lo32 = 0;
    }
    if !nbytes_out_hi32.is_null() {
        *nbytes_out_hi32 = 0;
    }

    if abandon == 0 && (*bzf).lastErr == BZ_OK {
        loop {
            (*bzf).strm.avail_out = BZ_MAX_UNUSED as u32;
            (*bzf).strm.next_out = (*bzf).buf.as_mut_ptr();
            let ret = BZ2_bzCompress(&mut (*bzf).strm, crate::constants::BZ_FINISH);
            if ret != crate::constants::BZ_FINISH_OK && ret != BZ_STREAM_END {
                set_bzerror(bzerror, bzf, ret);
                return;
            }

            if (*bzf).strm.avail_out < BZ_MAX_UNUSED as u32 {
                let n = BZ_MAX_UNUSED as usize - (*bzf).strm.avail_out as usize;
                let written = fwrite((*bzf).buf.as_ptr().cast::<c_void>(), 1, n, (*bzf).handle);
                if written != n || ferror((*bzf).handle) != 0 {
                    set_bzerror(bzerror, bzf, BZ_IO_ERROR);
                    return;
                }
            }

            if ret == BZ_STREAM_END {
                break;
            }
        }
    }

    if abandon == 0 && ferror((*bzf).handle) == 0 {
        let _ = fflush((*bzf).handle);
        if ferror((*bzf).handle) != 0 {
            set_bzerror(bzerror, bzf, BZ_IO_ERROR);
            return;
        }
    }

    if !nbytes_in_lo32.is_null() {
        *nbytes_in_lo32 = (*bzf).strm.total_in_lo32;
    }
    if !nbytes_in_hi32.is_null() {
        *nbytes_in_hi32 = (*bzf).strm.total_in_hi32;
    }
    if !nbytes_out_lo32.is_null() {
        *nbytes_out_lo32 = (*bzf).strm.total_out_lo32;
    }
    if !nbytes_out_hi32.is_null() {
        *nbytes_out_hi32 = (*bzf).strm.total_out_hi32;
    }

    set_bzerror(bzerror, bzf, BZ_OK);
    let _ = BZ2_bzCompressEnd(&mut (*bzf).strm);
    drop(Box::from_raw(bzf));
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzopen(path: *const c_char, mode: *const c_char) -> *mut c_void {
    bzopen_or_bzdopen(path, -1, mode, false)
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzdopen(fd: c_int, mode: *const c_char) -> *mut c_void {
    bzopen_or_bzdopen(ptr::null(), fd, mode, true)
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzread(b: *mut c_void, buf: *mut c_void, len: c_int) -> c_int {
    if b.is_null() {
        return -1;
    }
    let bzf = state_from_handle(b);
    if (*bzf).lastErr == BZ_STREAM_END {
        return 0;
    }
    let mut bzerr = BZ_OK;
    let nread = BZ2_bzRead(&mut bzerr, b, buf, len);
    if bzerr == BZ_OK || bzerr == BZ_STREAM_END {
        nread
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzwrite(b: *mut c_void, _buf: *mut c_void, len: c_int) -> c_int {
    if b.is_null() {
        return -1;
    }
    let mut bzerr = BZ_OK;
    BZ2_bzWrite(&mut bzerr, b, _buf, len);
    if bzerr == BZ_OK {
        len
    } else {
        -1
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzflush(_b: *mut c_void) -> c_int {
    0
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzclose(b: *mut c_void) {
    if b.is_null() {
        return;
    }
    let bzf = state_from_handle(b);
    let handle = (*bzf).handle;
    if (*bzf).writing != 0 {
        let mut bzerr = BZ_OK;
        BZ2_bzWriteClose(&mut bzerr, b, 0, ptr::null_mut(), ptr::null_mut());
        if bzerr != BZ_OK {
            BZ2_bzWriteClose(ptr::null_mut(), b, 1, ptr::null_mut(), ptr::null_mut());
        }
    } else {
        let mut bzerr = BZ_OK;
        BZ2_bzReadClose(&mut bzerr, b);
    }
    if should_close_handle(handle) {
        let _ = fclose(handle);
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzerror(b: *mut c_void, errnum: *mut c_int) -> *const c_char {
    if b.is_null() {
        set_error_slot(errnum, BZ_PARAM_ERROR);
        return bzerror_message_ptr(BZ_PARAM_ERROR);
    }
    let bzf = state_from_handle(b);
    let mut code = (*bzf).lastErr;
    if code > 0 {
        code = BZ_OK;
    }
    set_error_slot(errnum, code);
    bzerror_message_ptr(code)
}
