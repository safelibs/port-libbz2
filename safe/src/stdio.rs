use crate::constants::{
    BZFILE_MODE_READ, BZFILE_MODE_WRITE, BZ_CONFIG_ERROR, BZ_OK, BZ_PARAM_ERROR,
};
use crate::ffi::{bzerror_message_ptr, set_error_slot};
use crate::types::{BzFileState, CFile};
use core::ffi::c_void;
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;

unsafe fn make_handle(
    mode: c_int,
    file: *mut CFile,
    verbosity: c_int,
    small: c_int,
    block_size_100k: c_int,
    work_factor: c_int,
) -> *mut c_void {
    Box::into_raw(Box::new(BzFileState {
        mode,
        last_err: BZ_OK,
        file,
        small,
        verbosity,
        block_size_100k,
        work_factor,
    }))
    .cast()
}

unsafe fn state_from_handle(handle: *mut c_void) -> *mut BzFileState {
    handle.cast()
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzReadOpen(
    bzerror: *mut c_int,
    f: *mut CFile,
    verbosity: c_int,
    small: c_int,
    _unused: *mut c_void,
    nUnused: c_int,
) -> *mut c_void {
    if f.is_null() || verbosity < 0 || (small != 0 && small != 1) || nUnused < 0 {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return ptr::null_mut();
    }
    set_error_slot(bzerror, BZ_OK);
    make_handle(BZFILE_MODE_READ, f, verbosity, small, 0, 0)
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzReadClose(bzerror: *mut c_int, b: *mut c_void) {
    if b.is_null() {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return;
    }
    drop(Box::from_raw(state_from_handle(b)));
    set_error_slot(bzerror, BZ_OK);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzReadGetUnused(
    bzerror: *mut c_int,
    b: *mut c_void,
    unused: *mut *mut c_void,
    nUnused: *mut c_int,
) {
    if b.is_null() {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return;
    }
    if !unused.is_null() {
        *unused = ptr::null_mut();
    }
    if !nUnused.is_null() {
        *nUnused = 0;
    }
    (*state_from_handle(b)).last_err = BZ_CONFIG_ERROR;
    set_error_slot(bzerror, BZ_CONFIG_ERROR);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzRead(
    bzerror: *mut c_int,
    b: *mut c_void,
    _buf: *mut c_void,
    len: c_int,
) -> c_int {
    if b.is_null() || len < 0 {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return 0;
    }
    (*state_from_handle(b)).last_err = BZ_CONFIG_ERROR;
    set_error_slot(bzerror, BZ_CONFIG_ERROR);
    0
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWriteOpen(
    bzerror: *mut c_int,
    f: *mut CFile,
    blockSize100k: c_int,
    verbosity: c_int,
    workFactor: c_int,
) -> *mut c_void {
    if f.is_null() || !(1..=9).contains(&blockSize100k) || verbosity < 0 || workFactor < 0 {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return ptr::null_mut();
    }
    set_error_slot(bzerror, BZ_OK);
    make_handle(
        BZFILE_MODE_WRITE,
        f,
        verbosity,
        0,
        blockSize100k,
        workFactor,
    )
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWrite(
    bzerror: *mut c_int,
    b: *mut c_void,
    _buf: *mut c_void,
    len: c_int,
) {
    if b.is_null() || len < 0 {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return;
    }
    (*state_from_handle(b)).last_err = BZ_CONFIG_ERROR;
    set_error_slot(bzerror, BZ_CONFIG_ERROR);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWriteClose(
    bzerror: *mut c_int,
    b: *mut c_void,
    abandon: c_int,
    nbytes_in: *mut c_uint,
    nbytes_out: *mut c_uint,
) {
    let mut in_lo32 = 0;
    let mut in_hi32 = 0;
    let mut out_lo32 = 0;
    let mut out_hi32 = 0;

    BZ2_bzWriteClose64(
        bzerror,
        b,
        abandon,
        &mut in_lo32,
        &mut in_hi32,
        &mut out_lo32,
        &mut out_hi32,
    );

    if !nbytes_in.is_null() {
        *nbytes_in = in_lo32;
    }
    if !nbytes_out.is_null() {
        *nbytes_out = out_lo32;
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzWriteClose64(
    bzerror: *mut c_int,
    b: *mut c_void,
    _abandon: c_int,
    nbytes_in_lo32: *mut c_uint,
    nbytes_in_hi32: *mut c_uint,
    nbytes_out_lo32: *mut c_uint,
    nbytes_out_hi32: *mut c_uint,
) {
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

    if b.is_null() {
        set_error_slot(bzerror, BZ_PARAM_ERROR);
        return;
    }
    drop(Box::from_raw(state_from_handle(b)));
    set_error_slot(bzerror, BZ_OK);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzopen(_path: *const c_char, _mode: *const c_char) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzdopen(_fd: c_int, _mode: *const c_char) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzread(b: *mut c_void, _buf: *mut c_void, _len: c_int) -> c_int {
    if b.is_null() {
        return -1;
    }
    (*state_from_handle(b)).last_err = BZ_CONFIG_ERROR;
    0
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzwrite(b: *mut c_void, _buf: *mut c_void, _len: c_int) -> c_int {
    if b.is_null() {
        return -1;
    }
    (*state_from_handle(b)).last_err = BZ_CONFIG_ERROR;
    0
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzflush(b: *mut c_void) -> c_int {
    if b.is_null() {
        return BZ_PARAM_ERROR;
    }
    (*state_from_handle(b)).last_err = BZ_CONFIG_ERROR;
    BZ_CONFIG_ERROR
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzclose(b: *mut c_void) {
    if b.is_null() {
        return;
    }
    drop(Box::from_raw(state_from_handle(b)));
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzerror(b: *mut c_void, errnum: *mut c_int) -> *const c_char {
    if b.is_null() {
        set_error_slot(errnum, BZ_PARAM_ERROR);
        return bzerror_message_ptr(BZ_PARAM_ERROR);
    }
    let code = (*state_from_handle(b)).last_err;
    set_error_slot(errnum, code);
    bzerror_message_ptr(code)
}
