use crate::alloc::{drop_box, reset_stream_totals, zeroed_box};
use crate::constants::{
    BZ_CONFIG_ERROR, BZ_FINISH, BZ_FLUSH, BZ_M_RUNNING, BZ_OK, BZ_PARAM_ERROR, BZ_RUN,
    BZ_SEQUENCE_ERROR, BZ_S_INPUT,
};
use crate::types::{bz_stream, Bool, EState, Int32, UChar};
use std::os::raw::c_int;
use std::ptr;

fn valid_block_size(block_size_100k: c_int) -> bool {
    (1..=9).contains(&block_size_100k)
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzCompressInit(
    strm: *mut bz_stream,
    blockSize100k: c_int,
    verbosity: c_int,
    workFactor: c_int,
) -> c_int {
    if strm.is_null() || !valid_block_size(blockSize100k) || verbosity < 0 || workFactor < 0 {
        return BZ_PARAM_ERROR;
    }
    if !(*strm).state.is_null() {
        return BZ_SEQUENCE_ERROR;
    }

    reset_stream_totals(strm);
    let state = zeroed_box::<EState>();
    (*state).strm = strm;
    (*state).mode = BZ_M_RUNNING;
    (*state).state = BZ_S_INPUT;
    (*state).blockSize100k = blockSize100k;
    (*state).verbosity = verbosity;
    (*state).workFactor = workFactor;
    (*strm).state = state.cast();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzCompress(strm: *mut bz_stream, action: c_int) -> c_int {
    if strm.is_null() || (*strm).state.is_null() {
        return BZ_SEQUENCE_ERROR;
    }
    if action != BZ_RUN && action != BZ_FLUSH && action != BZ_FINISH {
        return BZ_PARAM_ERROR;
    }
    BZ_CONFIG_ERROR
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzCompressEnd(strm: *mut bz_stream) -> c_int {
    if strm.is_null() {
        return BZ_PARAM_ERROR;
    }
    if (*strm).state.is_null() {
        return BZ_SEQUENCE_ERROR;
    }

    drop_box::<EState>((*strm).state);
    (*strm).state = ptr::null_mut();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_blockSort(_state: *mut EState) {}

#[no_mangle]
pub unsafe extern "C" fn BZ2_compressBlock(_state: *mut EState, _is_last_block: Bool) {}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bsInitWrite(state: *mut EState) {
    if state.is_null() {
        return;
    }
    (*state).bsBuff = 0;
    (*state).bsLive = 0;
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_hbAssignCodes(
    _code: *mut Int32,
    _length: *mut UChar,
    _minLen: Int32,
    _maxLen: Int32,
    _alphaSize: Int32,
) {
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_hbMakeCodeLengths(
    _length: *mut UChar,
    _freq: *mut Int32,
    _alphaSize: Int32,
    _maxLen: Int32,
) {
}
