use crate::types::bz_stream;
use core::ffi::c_void;
use core::mem::MaybeUninit;

pub unsafe fn zeroed_box<T>() -> *mut T {
    Box::into_raw(Box::new(MaybeUninit::<T>::zeroed().assume_init()))
}

pub unsafe fn drop_box<T>(ptr: *mut c_void) {
    if !ptr.is_null() {
        drop(Box::from_raw(ptr.cast::<T>()));
    }
}

pub unsafe fn reset_stream_totals(strm: *mut bz_stream) {
    if strm.is_null() {
        return;
    }

    (*strm).total_in_lo32 = 0;
    (*strm).total_in_hi32 = 0;
    (*strm).total_out_lo32 = 0;
    (*strm).total_out_hi32 = 0;
}
