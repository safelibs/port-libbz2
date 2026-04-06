use crate::constants::BZ_MEM_ERROR;
use crate::types::{bz_alloc_func, bz_free_func, bz_stream};
use core::ffi::c_void;
use core::mem::{size_of, MaybeUninit};
use core::ptr;
use std::os::raw::{c_char, c_int, c_short};

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

#[inline]
pub(crate) fn bz_config_ok() -> bool {
    size_of::<c_int>() == 4 && size_of::<c_short>() == 2 && size_of::<c_char>() == 1
}

#[inline]
pub(crate) unsafe fn ensure_default_allocators(
    strm: *mut bz_stream,
    default_bzalloc: bz_alloc_func,
    default_bzfree: bz_free_func,
) {
    if (*strm).bzalloc.is_none() {
        (*strm).bzalloc = default_bzalloc;
    }
    if (*strm).bzfree.is_none() {
        (*strm).bzfree = default_bzfree;
    }
}

#[inline]
pub(crate) unsafe fn free_with_bzfree<T>(strm: *mut bz_stream, ptr: *mut T) {
    if ptr.is_null() {
        return;
    }
    if let Some(bzfree) = (*strm).bzfree {
        bzfree((*strm).opaque, ptr.cast());
    }
}

pub(crate) unsafe fn alloc_zeroed_with_bzalloc<T>(strm: *mut bz_stream) -> Result<*mut T, c_int> {
    alloc_zeroed_slice_with_bzalloc::<T>(strm, 1)
}

pub(crate) unsafe fn alloc_zeroed_slice_with_bzalloc<T>(
    strm: *mut bz_stream,
    len: usize,
) -> Result<*mut T, c_int> {
    let Some(bytes) = len.checked_mul(size_of::<T>()) else {
        return Err(BZ_MEM_ERROR);
    };
    let items = c_int::try_from(bytes).map_err(|_| BZ_MEM_ERROR)?;
    let Some(bzalloc) = (*strm).bzalloc else {
        return Err(BZ_MEM_ERROR);
    };
    let raw = bzalloc((*strm).opaque, items, 1);
    if raw.is_null() {
        return Err(BZ_MEM_ERROR);
    }
    if bytes != 0 {
        ptr::write_bytes(raw, 0, bytes);
    }
    Ok(raw.cast())
}
