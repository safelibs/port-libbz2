use crate::alloc::{drop_box, reset_stream_totals, zeroed_box};
use crate::constants::{BZ_CONFIG_ERROR, BZ_OK, BZ_PARAM_ERROR, BZ_SEQUENCE_ERROR, BZ_X_IDLE};
use crate::types::{bz_stream, DState, Int32, UChar};
use std::os::raw::c_int;
use std::ptr;

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzDecompressInit(
    strm: *mut bz_stream,
    verbosity: c_int,
    small: c_int,
) -> c_int {
    if strm.is_null() || verbosity < 0 || (small != 0 && small != 1) {
        return BZ_PARAM_ERROR;
    }
    if !(*strm).state.is_null() {
        return BZ_SEQUENCE_ERROR;
    }

    reset_stream_totals(strm);
    let state = zeroed_box::<DState>();
    (*state).strm = strm;
    (*state).state = BZ_X_IDLE;
    (*state).verbosity = verbosity;
    (*state).smallDecompress = small as UChar;
    (*strm).state = state.cast();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzDecompress(strm: *mut bz_stream) -> c_int {
    if strm.is_null() || (*strm).state.is_null() {
        return BZ_SEQUENCE_ERROR;
    }
    BZ_CONFIG_ERROR
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzDecompressEnd(strm: *mut bz_stream) -> c_int {
    if strm.is_null() {
        return BZ_PARAM_ERROR;
    }
    if (*strm).state.is_null() {
        return BZ_SEQUENCE_ERROR;
    }

    drop_box::<DState>((*strm).state);
    (*strm).state = ptr::null_mut();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_indexIntoF(_index: Int32, _cftab: *mut Int32) -> Int32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_decompress(_state: *mut DState) -> Int32 {
    BZ_CONFIG_ERROR
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_hbCreateDecodeTables(
    _limit: *mut Int32,
    _base: *mut Int32,
    _perm: *mut Int32,
    _length: *mut UChar,
    _minLen: Int32,
    _maxLen: Int32,
    _alphaSize: Int32,
) {
}
