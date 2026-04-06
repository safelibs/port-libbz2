use crate::constants::{BZ_DATA_ERROR, BZ_MAX_ALPHA_SIZE, BZ_MAX_CODE_LEN};
use crate::types::{Int32, UChar};
use std::os::raw::c_int;
use std::slice;

unsafe extern "C" {
    fn BZ2_bz__AssertH__fail(errcode: c_int);
}

const MAX_HEAP: usize = BZ_MAX_ALPHA_SIZE + 2;
const MAX_NODES: usize = BZ_MAX_ALPHA_SIZE * 2;

#[inline]
fn weight_of(value: Int32) -> Int32 {
    value & 0xffff_ff00u32 as Int32
}

#[inline]
fn depth_of(value: Int32) -> Int32 {
    value & 0x0000_00ff
}

#[inline]
fn add_weights(lhs: Int32, rhs: Int32) -> Int32 {
    weight_of(lhs) + weight_of(rhs) | (1 + depth_of(lhs).max(depth_of(rhs)))
}

fn upheap(heap: &mut [Int32], weight: &[Int32], start: usize) {
    let mut zz = start;
    let tmp = heap[zz];
    while weight[tmp as usize] < weight[heap[zz >> 1] as usize] {
        heap[zz] = heap[zz >> 1];
        zz >>= 1;
    }
    heap[zz] = tmp;
}

fn downheap(heap: &mut [Int32], weight: &[Int32], n_heap: usize, start: usize) {
    let mut zz = start;
    let tmp = heap[zz];
    loop {
        let mut yy = zz << 1;
        if yy > n_heap {
            break;
        }
        if yy < n_heap && weight[heap[yy + 1] as usize] < weight[heap[yy] as usize] {
            yy += 1;
        }
        if weight[tmp as usize] < weight[heap[yy] as usize] {
            break;
        }
        heap[zz] = heap[yy];
        zz = yy;
    }
    heap[zz] = tmp;
}

fn make_code_lengths(length: &mut [UChar], freq: &[Int32], alpha_size: usize, max_len: Int32) {
    let mut heap = [0; MAX_HEAP];
    let mut weight = [0; MAX_NODES];
    let mut parent = [0; MAX_NODES];

    for i in 0..alpha_size {
        let base = if freq[i] == 0 { 1 } else { freq[i] };
        weight[i + 1] = base << 8;
    }

    loop {
        let mut n_nodes = alpha_size;
        let mut n_heap = 0usize;
        heap[0] = 0;
        weight[0] = 0;
        parent[0] = -2;

        for i in 1..=alpha_size {
            parent[i] = -1;
            n_heap += 1;
            heap[n_heap] = i as Int32;
            upheap(&mut heap, &weight, n_heap);
        }

        if n_heap >= MAX_HEAP {
            unsafe { BZ2_bz__AssertH__fail(2001) };
        }

        while n_heap > 1 {
            let n1 = heap[1] as usize;
            heap[1] = heap[n_heap];
            n_heap -= 1;
            downheap(&mut heap, &weight, n_heap, 1);

            let n2 = heap[1] as usize;
            heap[1] = heap[n_heap];
            n_heap -= 1;
            downheap(&mut heap, &weight, n_heap, 1);

            n_nodes += 1;
            parent[n1] = n_nodes as Int32;
            parent[n2] = n_nodes as Int32;
            weight[n_nodes] = add_weights(weight[n1], weight[n2]);
            parent[n_nodes] = -1;
            n_heap += 1;
            heap[n_heap] = n_nodes as Int32;
            upheap(&mut heap, &weight, n_heap);
        }

        if n_nodes >= MAX_NODES {
            unsafe { BZ2_bz__AssertH__fail(2002) };
        }

        let mut too_long = false;
        for i in 1..=alpha_size {
            let mut depth = 0;
            let mut k = i;
            while parent[k] >= 0 {
                k = parent[k] as usize;
                depth += 1;
            }
            length[i - 1] = depth as UChar;
            if depth > max_len {
                too_long = true;
            }
        }

        if !too_long {
            break;
        }

        for value in weight.iter_mut().take(alpha_size + 1).skip(1) {
            let j = *value >> 8;
            *value = (1 + (j / 2)) << 8;
        }
    }
}

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

#[no_mangle]
pub unsafe extern "C" fn BZ2_hbMakeCodeLengths(
    length: *mut UChar,
    freq: *mut Int32,
    alphaSize: Int32,
    maxLen: Int32,
) {
    if length.is_null() || freq.is_null() || alphaSize <= 0 {
        return;
    }
    let Ok(alpha_size) = usize::try_from(alphaSize) else {
        return;
    };
    if alpha_size > BZ_MAX_ALPHA_SIZE {
        return;
    }
    let length = slice::from_raw_parts_mut(length, alpha_size);
    let freq = slice::from_raw_parts(freq, alpha_size);
    make_code_lengths(length, freq, alpha_size, maxLen);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_hbAssignCodes(
    code: *mut Int32,
    length: *mut UChar,
    minLen: Int32,
    maxLen: Int32,
    alphaSize: Int32,
) {
    if code.is_null() || length.is_null() || alphaSize <= 0 {
        return;
    }
    let Ok(alpha_size) = usize::try_from(alphaSize) else {
        return;
    };
    if alpha_size > BZ_MAX_ALPHA_SIZE {
        return;
    }

    let code = slice::from_raw_parts_mut(code, alpha_size);
    let length = slice::from_raw_parts(length, alpha_size);
    let mut vec = 0;
    for n in minLen..=maxLen {
        for i in 0..alpha_size {
            if length[i] as Int32 == n {
                code[i] = vec;
                vec += 1;
            }
        }
        vec <<= 1;
    }
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
