use crate::constants::{BZ_DATA_ERROR, BZ_MAX_ALPHA_SIZE, BZ_MAX_CODE_LEN};
use crate::types::{Int32, UChar};
use std::os::raw::c_int;
use std::slice;

fn build_decode_tables(
    limit: &mut [Int32],
    base: &mut [Int32],
    perm: &mut [Int32],
    length: &[UChar],
    min_len: Int32,
    max_len: Int32,
    alpha_size: Int32,
) -> Result<(), c_int> {
    let alpha_size = usize::try_from(alpha_size).map_err(|_| BZ_DATA_ERROR)?;
    let min_len = usize::try_from(min_len).map_err(|_| BZ_DATA_ERROR)?;
    let max_len = usize::try_from(max_len).map_err(|_| BZ_DATA_ERROR)?;

    if alpha_size == 0
        || alpha_size > BZ_MAX_ALPHA_SIZE
        || min_len >= BZ_MAX_CODE_LEN as usize
        || max_len >= BZ_MAX_CODE_LEN as usize
        || min_len > max_len
    {
        return Err(BZ_DATA_ERROR);
    }

    let length = length.get(..alpha_size).ok_or(BZ_DATA_ERROR)?;
    let perm = perm.get_mut(..alpha_size).ok_or(BZ_DATA_ERROR)?;
    let limit = limit.get_mut(..BZ_MAX_ALPHA_SIZE).ok_or(BZ_DATA_ERROR)?;
    let base = base.get_mut(..BZ_MAX_ALPHA_SIZE).ok_or(BZ_DATA_ERROR)?;

    let mut pp = 0usize;
    for i in min_len..=max_len {
        for (j, &code_len) in length.iter().enumerate() {
            if usize::from(code_len) == i {
                perm[pp] = j as Int32;
                pp += 1;
            }
        }
    }

    base.fill(0);
    for &code_len in length {
        let next = usize::from(code_len)
            .checked_add(1)
            .filter(|idx| *idx < BZ_MAX_CODE_LEN as usize)
            .ok_or(BZ_DATA_ERROR)?;
        base[next] = base[next].checked_add(1).ok_or(BZ_DATA_ERROR)?;
    }

    for i in 1..BZ_MAX_CODE_LEN as usize {
        base[i] = base[i].checked_add(base[i - 1]).ok_or(BZ_DATA_ERROR)?;
    }

    limit.fill(0);
    let mut vec = 0i32;
    for i in min_len..=max_len {
        vec = vec
            .checked_add(base[i + 1] - base[i])
            .ok_or(BZ_DATA_ERROR)?;
        limit[i] = vec.checked_sub(1).ok_or(BZ_DATA_ERROR)?;
        vec = vec.checked_shl(1).ok_or(BZ_DATA_ERROR)?;
    }

    for i in (min_len + 1)..=max_len {
        base[i] = ((limit[i - 1] + 1) << 1)
            .checked_sub(base[i])
            .ok_or(BZ_DATA_ERROR)?;
    }

    Ok(())
}

pub(crate) fn hb_create_decode_tables_checked(
    limit: &mut [Int32],
    base: &mut [Int32],
    perm: &mut [Int32],
    length: &[UChar],
    min_len: Int32,
    max_len: Int32,
    alpha_size: Int32,
) -> Result<(), c_int> {
    build_decode_tables(limit, base, perm, length, min_len, max_len, alpha_size)
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_hbCreateDecodeTables(
    limit: *mut Int32,
    base: *mut Int32,
    perm: *mut Int32,
    length: *mut UChar,
    minLen: Int32,
    maxLen: Int32,
    alphaSize: Int32,
) {
    if limit.is_null() || base.is_null() || perm.is_null() || length.is_null() {
        return;
    }

    let limit = slice::from_raw_parts_mut(limit, BZ_MAX_ALPHA_SIZE);
    let base = slice::from_raw_parts_mut(base, BZ_MAX_ALPHA_SIZE);
    let perm = slice::from_raw_parts_mut(perm, BZ_MAX_ALPHA_SIZE);
    let length = slice::from_raw_parts(length, BZ_MAX_ALPHA_SIZE);
    let _ = build_decode_tables(limit, base, perm, length, minLen, maxLen, alphaSize);
}
