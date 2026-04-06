use crate::alloc::{
    alloc_zeroed_slice_with_bzalloc, alloc_zeroed_with_bzalloc, bz_config_ok,
    ensure_default_allocators, free_with_bzfree, reset_stream_totals,
};
use crate::constants::{
    BZ_CONFIG_ERROR, BZ_DATA_ERROR, BZ_DATA_ERROR_MAGIC, BZ_G_SIZE, BZ_MAX_ALPHA_SIZE,
    BZ_MAX_SELECTORS, BZ_OK, BZ_PARAM_ERROR, BZ_RUNA, BZ_RUNB, BZ_SEQUENCE_ERROR, BZ_STREAM_END,
    BZ_X_BCRC_1, BZ_X_BLKHDR_1, BZ_X_CCRC_1, BZ_X_ENDHDR_2, BZ_X_IDLE, BZ_X_MAGIC_1,
    BZ_X_MAPPING_1, BZ_X_OUTPUT,
};
use crate::crc::{bz_crc_finalize, bz_crc_init, bz_crc_update};
use crate::huffman::hb_create_decode_tables_checked;
use crate::rand::{rand_init, rand_mask, rand_update_mask};
use crate::types::{bz_stream, stream_state, DState, Int32, UChar, UInt16, UInt32};
use std::os::raw::c_int;
use std::ptr;
use std::slice;

const BZ_HDR_B: u8 = 0x42;
const BZ_HDR_Z: u8 = 0x5a;
const BZ_HDR_h: u8 = 0x68;
const BZ_HDR_0: Int32 = 0x30;
const MAX_HUFFMAN_LEN: Int32 = 20;
const MAX_RUN_ACCUMULATOR: Int32 = 2 * 1024 * 1024;
const MAX_VERBOSITY: c_int = 4;
const FIRST_MTF_GROUP_LABEL: DecodeState = DecodeState::Mtf1;

unsafe extern "C" {
    fn malloc(size: usize) -> *mut core::ffi::c_void;
    fn free(ptr: *mut core::ffi::c_void);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
enum DecodeState {
    Idle = BZ_X_IDLE,
    Output = BZ_X_OUTPUT,
    Magic1 = BZ_X_MAGIC_1,
    Magic2 = BZ_X_MAGIC_1 + 1,
    Magic3 = BZ_X_MAGIC_1 + 2,
    Magic4 = BZ_X_MAGIC_1 + 3,
    BlkHdr1 = BZ_X_BLKHDR_1,
    BlkHdr2 = BZ_X_BLKHDR_1 + 1,
    BlkHdr3 = BZ_X_BLKHDR_1 + 2,
    BlkHdr4 = BZ_X_BLKHDR_1 + 3,
    BlkHdr5 = BZ_X_BLKHDR_1 + 4,
    BlkHdr6 = BZ_X_BLKHDR_1 + 5,
    Bcrc1 = BZ_X_BCRC_1,
    Bcrc2 = BZ_X_BCRC_1 + 1,
    Bcrc3 = BZ_X_BCRC_1 + 2,
    Bcrc4 = BZ_X_BCRC_1 + 3,
    RandBit = BZ_X_BCRC_1 + 4,
    OrigPtr1 = BZ_X_BCRC_1 + 5,
    OrigPtr2 = BZ_X_BCRC_1 + 6,
    OrigPtr3 = BZ_X_BCRC_1 + 7,
    Mapping1 = BZ_X_MAPPING_1,
    Mapping2 = BZ_X_MAPPING_1 + 1,
    Selector1 = BZ_X_MAPPING_1 + 2,
    Selector2 = BZ_X_MAPPING_1 + 3,
    Selector3 = BZ_X_MAPPING_1 + 4,
    Coding1 = BZ_X_MAPPING_1 + 5,
    Coding2 = BZ_X_MAPPING_1 + 6,
    Coding3 = BZ_X_MAPPING_1 + 7,
    Mtf1 = BZ_X_MAPPING_1 + 8,
    Mtf2 = BZ_X_MAPPING_1 + 9,
    Mtf3 = BZ_X_MAPPING_1 + 10,
    Mtf4 = BZ_X_MAPPING_1 + 11,
    Mtf5 = BZ_X_MAPPING_1 + 12,
    Mtf6 = BZ_X_MAPPING_1 + 13,
    EndHdr2 = BZ_X_ENDHDR_2,
    EndHdr3 = BZ_X_ENDHDR_2 + 1,
    EndHdr4 = BZ_X_ENDHDR_2 + 2,
    EndHdr5 = BZ_X_ENDHDR_2 + 3,
    EndHdr6 = BZ_X_ENDHDR_2 + 4,
    Ccrc1 = BZ_X_CCRC_1,
    Ccrc2 = BZ_X_CCRC_1 + 1,
    Ccrc3 = BZ_X_CCRC_1 + 2,
    Ccrc4 = BZ_X_CCRC_1 + 3,
}

impl DecodeState {
    fn from_raw(raw: Int32) -> Option<Self> {
        match raw {
            BZ_X_IDLE => Some(Self::Idle),
            BZ_X_OUTPUT => Some(Self::Output),
            10 => Some(Self::Magic1),
            11 => Some(Self::Magic2),
            12 => Some(Self::Magic3),
            13 => Some(Self::Magic4),
            14 => Some(Self::BlkHdr1),
            15 => Some(Self::BlkHdr2),
            16 => Some(Self::BlkHdr3),
            17 => Some(Self::BlkHdr4),
            18 => Some(Self::BlkHdr5),
            19 => Some(Self::BlkHdr6),
            20 => Some(Self::Bcrc1),
            21 => Some(Self::Bcrc2),
            22 => Some(Self::Bcrc3),
            23 => Some(Self::Bcrc4),
            24 => Some(Self::RandBit),
            25 => Some(Self::OrigPtr1),
            26 => Some(Self::OrigPtr2),
            27 => Some(Self::OrigPtr3),
            28 => Some(Self::Mapping1),
            29 => Some(Self::Mapping2),
            30 => Some(Self::Selector1),
            31 => Some(Self::Selector2),
            32 => Some(Self::Selector3),
            33 => Some(Self::Coding1),
            34 => Some(Self::Coding2),
            35 => Some(Self::Coding3),
            36 => Some(Self::Mtf1),
            37 => Some(Self::Mtf2),
            38 => Some(Self::Mtf3),
            39 => Some(Self::Mtf4),
            40 => Some(Self::Mtf5),
            41 => Some(Self::Mtf6),
            42 => Some(Self::EndHdr2),
            43 => Some(Self::EndHdr3),
            44 => Some(Self::EndHdr4),
            45 => Some(Self::EndHdr5),
            46 => Some(Self::EndHdr6),
            47 => Some(Self::Ccrc1),
            48 => Some(Self::Ccrc2),
            49 => Some(Self::Ccrc3),
            50 => Some(Self::Ccrc4),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct ParserLocals {
    i: Int32,
    j: Int32,
    t: Int32,
    alphaSize: Int32,
    nGroups: Int32,
    nSelectors: Int32,
    EOB: Int32,
    groupNo: Int32,
    groupPos: Int32,
    nextSym: Int32,
    nblockMAX: Int32,
    nblock: Int32,
    es: Int32,
    N: Int32,
    curr: Int32,
    zt: Int32,
    zn: Int32,
    zvec: Int32,
    zj: Int32,
    gSel: Int32,
    gMinlen: Int32,
}

macro_rules! or_return_code {
    ($expr:expr, $state:expr, $locals:expr) => {
        match $expr {
            Ok(value) => value,
            Err(code) => return return_with_code($state, $locals, code),
        }
    };
}

fn usize_from_i32(value: Int32) -> Result<usize, c_int> {
    usize::try_from(value).map_err(|_| BZ_DATA_ERROR)
}

fn uchar_from_i32(value: Int32) -> Result<UChar, c_int> {
    UChar::try_from(value).map_err(|_| BZ_DATA_ERROR)
}

unsafe extern "C" fn default_bzalloc(
    _opaque: *mut core::ffi::c_void,
    items: c_int,
    size: c_int,
) -> *mut core::ffi::c_void {
    if items < 0 || size < 0 {
        return ptr::null_mut();
    }
    let items = usize::try_from(items).ok();
    let size = usize::try_from(size).ok();
    let bytes = items.and_then(|items| size.and_then(|size| items.checked_mul(size)));
    let Some(bytes) = bytes else {
        return ptr::null_mut();
    };
    malloc(bytes)
}

unsafe extern "C" fn default_bzfree(_opaque: *mut core::ffi::c_void, addr: *mut core::ffi::c_void) {
    if !addr.is_null() {
        free(addr);
    }
}

fn block_capacity(block_size_100k: Int32) -> Option<usize> {
    usize::try_from(block_size_100k).ok()?.checked_mul(100_000)
}

fn ll4_capacity(block_capacity: usize) -> Option<usize> {
    block_capacity.checked_add(1).map(|n| n >> 1)
}

unsafe fn release_block_storage(state: *mut DState) {
    let s = &mut *state;
    if block_capacity(s.blockSize100k).is_some() {
        if !s.tt.is_null() {
            free_with_bzfree(s.strm, s.tt);
            s.tt = ptr::null_mut();
        }
        if !s.ll16.is_null() {
            free_with_bzfree(s.strm, s.ll16);
            s.ll16 = ptr::null_mut();
        }
        if !s.ll4.is_null() {
            free_with_bzfree(s.strm, s.ll4);
            s.ll4 = ptr::null_mut();
        }
    } else {
        s.tt = ptr::null_mut();
        s.ll16 = ptr::null_mut();
        s.ll4 = ptr::null_mut();
    }
}

unsafe fn allocate_block_storage(state: *mut DState) -> c_int {
    let s = &mut *state;
    let Some(capacity) = block_capacity(s.blockSize100k) else {
        return BZ_DATA_ERROR;
    };

    if s.smallDecompress != 0 {
        let Some(ll4_cap) = ll4_capacity(capacity) else {
            return BZ_DATA_ERROR;
        };
        match alloc_zeroed_slice_with_bzalloc::<UInt16>(s.strm, capacity) {
            Ok(ptr) => s.ll16 = ptr,
            Err(code) => return code,
        }
        match alloc_zeroed_slice_with_bzalloc::<UChar>(s.strm, ll4_cap) {
            Ok(ptr) => s.ll4 = ptr,
            Err(code) => {
                release_block_storage(state);
                return code;
            }
        }
    } else {
        match alloc_zeroed_slice_with_bzalloc::<UInt32>(s.strm, capacity) {
            Ok(ptr) => s.tt = ptr,
            Err(code) => return code,
        }
    }

    BZ_OK
}

unsafe fn tt_slice_mut(s: &mut DState) -> Result<&mut [UInt32], c_int> {
    let len = block_capacity(s.blockSize100k).ok_or(BZ_DATA_ERROR)?;
    if s.tt.is_null() {
        return Err(BZ_DATA_ERROR);
    }
    Ok(slice::from_raw_parts_mut(s.tt, len))
}

unsafe fn ll16_slice_mut(s: &mut DState) -> Result<&mut [UInt16], c_int> {
    let len = block_capacity(s.blockSize100k).ok_or(BZ_DATA_ERROR)?;
    if s.ll16.is_null() {
        return Err(BZ_DATA_ERROR);
    }
    Ok(slice::from_raw_parts_mut(s.ll16, len))
}

unsafe fn set_ll(s: &mut DState, index: usize, value: UInt32) -> Result<(), c_int> {
    let ll16_len = block_capacity(s.blockSize100k).ok_or(BZ_DATA_ERROR)?;
    let ll4_len = ll4_capacity(ll16_len).ok_or(BZ_DATA_ERROR)?;
    if s.ll16.is_null() || s.ll4.is_null() {
        return Err(BZ_DATA_ERROR);
    }
    let ll16 = slice::from_raw_parts_mut(s.ll16, ll16_len);
    let ll4 = slice::from_raw_parts_mut(s.ll4, ll4_len);
    let nibble_index = index >> 1;
    let ll16_entry = ll16.get_mut(index).ok_or(BZ_DATA_ERROR)?;
    let ll4_entry = ll4.get_mut(nibble_index).ok_or(BZ_DATA_ERROR)?;
    *ll16_entry = (value & 0x0000_ffff) as UInt16;
    let hi = ((value >> 16) & 0x0f) as UChar;
    if (index & 1) == 0 {
        *ll4_entry = (*ll4_entry & 0xf0) | hi;
    } else {
        *ll4_entry = (*ll4_entry & 0x0f) | (hi << 4);
    }
    Ok(())
}

unsafe fn get_ll(s: &mut DState, index: usize) -> Result<UInt32, c_int> {
    let ll16_len = block_capacity(s.blockSize100k).ok_or(BZ_DATA_ERROR)?;
    let ll4_len = ll4_capacity(ll16_len).ok_or(BZ_DATA_ERROR)?;
    if s.ll16.is_null() || s.ll4.is_null() {
        return Err(BZ_DATA_ERROR);
    }
    let ll16 = slice::from_raw_parts_mut(s.ll16, ll16_len);
    let ll4 = slice::from_raw_parts_mut(s.ll4, ll4_len);
    let lo = UInt32::from(*ll16.get(index).ok_or(BZ_DATA_ERROR)?);
    let hi_nibble = UInt32::from(
        (*ll4.get(index >> 1).ok_or(BZ_DATA_ERROR)? >> (((index << 2) & 0x4) as u8)) & 0x0f,
    );
    Ok(lo | (hi_nibble << 16))
}

unsafe fn get_fast_value(s: &mut DState) -> Result<u8, c_int> {
    let len = block_capacity(s.blockSize100k).ok_or(BZ_DATA_ERROR)?;
    let t_pos = usize::try_from(s.tPos).map_err(|_| BZ_DATA_ERROR)?;
    if t_pos >= len {
        return Err(BZ_DATA_ERROR);
    }
    let tt = tt_slice_mut(s)?;
    let value = *tt.get(t_pos).ok_or(BZ_DATA_ERROR)?;
    s.tPos = value >> 8;
    Ok((value & 0xff) as u8)
}

unsafe fn get_small_value(s: &mut DState) -> Result<u8, c_int> {
    let len = block_capacity(s.blockSize100k).ok_or(BZ_DATA_ERROR)?;
    let t_pos = usize::try_from(s.tPos).map_err(|_| BZ_DATA_ERROR)?;
    if t_pos >= len {
        return Err(BZ_DATA_ERROR);
    }
    let symbol = BZ2_indexIntoF(s.tPos as Int32, s.cftab.as_mut_ptr());
    s.tPos = get_ll(s, t_pos)?;
    u8::try_from(symbol).map_err(|_| BZ_DATA_ERROR)
}

unsafe fn increment_total_in(strm: &mut bz_stream) {
    strm.total_in_lo32 = strm.total_in_lo32.wrapping_add(1);
    if strm.total_in_lo32 == 0 {
        strm.total_in_hi32 = strm.total_in_hi32.wrapping_add(1);
    }
}

unsafe fn increment_total_out(strm: &mut bz_stream) {
    strm.total_out_lo32 = strm.total_out_lo32.wrapping_add(1);
    if strm.total_out_lo32 == 0 {
        strm.total_out_hi32 = strm.total_out_hi32.wrapping_add(1);
    }
}

unsafe fn write_output_byte(s: &mut DState, ch: u8) -> Result<(), c_int> {
    let strm = &mut *s.strm;
    if strm.avail_out == 0 {
        return Err(BZ_OK);
    }
    if strm.next_out.is_null() {
        return Err(BZ_PARAM_ERROR);
    }
    *strm.next_out.cast::<u8>() = ch;
    strm.next_out = strm.next_out.add(1);
    strm.avail_out -= 1;
    increment_total_out(strm);
    s.calculatedBlockCRC = bz_crc_update(s.calculatedBlockCRC, ch);
    Ok(())
}

unsafe fn finish_pending_run(s: &mut DState) -> Result<bool, c_int> {
    while s.state_out_len > 0 {
        match write_output_byte(s, s.state_out_ch) {
            Ok(()) => {
                s.state_out_len -= 1;
            }
            Err(BZ_OK) => return Ok(false),
            Err(code) => return Err(code),
        }
    }
    Ok(true)
}

unsafe fn output_fast_nonrandomized(s: &mut DState) -> Result<bool, c_int> {
    let save_nblock_pp = s.save_nblock.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    loop {
        if !finish_pending_run(s)? {
            return Ok(false);
        }
        if s.nblock_used == save_nblock_pp {
            return Ok(false);
        }
        if s.nblock_used > save_nblock_pp {
            return Ok(true);
        }

        s.state_out_len = 1;
        s.state_out_ch = s.k0 as UChar;
        let k1 = Int32::from(get_fast_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if k1 != s.k0 {
            s.k0 = k1;
            continue;
        }

        s.state_out_len = 2;
        let k1 = Int32::from(get_fast_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if k1 != s.k0 {
            s.k0 = k1;
            continue;
        }

        s.state_out_len = 3;
        let k1 = Int32::from(get_fast_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if k1 != s.k0 {
            s.k0 = k1;
            continue;
        }

        let k1 = Int32::from(get_fast_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        s.state_out_len = k1.checked_add(4).ok_or(BZ_DATA_ERROR)?;
        s.k0 = Int32::from(get_fast_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    }
}

unsafe fn output_fast_randomized(s: &mut DState) -> Result<bool, c_int> {
    let save_nblock_pp = s.save_nblock.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    loop {
        if !finish_pending_run(s)? {
            return Ok(false);
        }
        if s.nblock_used == save_nblock_pp {
            return Ok(false);
        }
        if s.nblock_used > save_nblock_pp {
            return Ok(true);
        }

        s.state_out_len = 1;
        s.state_out_ch = s.k0 as UChar;
        let mut k1 = get_fast_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if Int32::from(k1) != s.k0 {
            s.k0 = Int32::from(k1);
            continue;
        }

        s.state_out_len = 2;
        let mut k1 = get_fast_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if Int32::from(k1) != s.k0 {
            s.k0 = Int32::from(k1);
            continue;
        }

        s.state_out_len = 3;
        let mut k1 = get_fast_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if Int32::from(k1) != s.k0 {
            s.k0 = Int32::from(k1);
            continue;
        }

        let mut k1 = get_fast_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        s.state_out_len = Int32::from(k1).checked_add(4).ok_or(BZ_DATA_ERROR)?;
        let mut next_k0 = get_fast_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        next_k0 ^= rand_mask(s.rNToGo);
        s.k0 = Int32::from(next_k0);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    }
}

unsafe fn output_small_nonrandomized(s: &mut DState) -> Result<bool, c_int> {
    let save_nblock_pp = s.save_nblock.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    loop {
        if !finish_pending_run(s)? {
            return Ok(false);
        }
        if s.nblock_used == save_nblock_pp {
            return Ok(false);
        }
        if s.nblock_used > save_nblock_pp {
            return Ok(true);
        }

        s.state_out_len = 1;
        s.state_out_ch = s.k0 as UChar;
        let k1 = Int32::from(get_small_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if k1 != s.k0 {
            s.k0 = k1;
            continue;
        }

        s.state_out_len = 2;
        let k1 = Int32::from(get_small_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if k1 != s.k0 {
            s.k0 = k1;
            continue;
        }

        s.state_out_len = 3;
        let k1 = Int32::from(get_small_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if k1 != s.k0 {
            s.k0 = k1;
            continue;
        }

        let k1 = Int32::from(get_small_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        s.state_out_len = k1.checked_add(4).ok_or(BZ_DATA_ERROR)?;
        s.k0 = Int32::from(get_small_value(s)?);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    }
}

unsafe fn output_small_randomized(s: &mut DState) -> Result<bool, c_int> {
    let save_nblock_pp = s.save_nblock.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    loop {
        if !finish_pending_run(s)? {
            return Ok(false);
        }
        if s.nblock_used == save_nblock_pp {
            return Ok(false);
        }
        if s.nblock_used > save_nblock_pp {
            return Ok(true);
        }

        s.state_out_len = 1;
        s.state_out_ch = s.k0 as UChar;
        let mut k1 = get_small_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if Int32::from(k1) != s.k0 {
            s.k0 = Int32::from(k1);
            continue;
        }

        s.state_out_len = 2;
        let mut k1 = get_small_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if Int32::from(k1) != s.k0 {
            s.k0 = Int32::from(k1);
            continue;
        }

        s.state_out_len = 3;
        let mut k1 = get_small_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        if s.nblock_used == save_nblock_pp {
            continue;
        }
        if Int32::from(k1) != s.k0 {
            s.k0 = Int32::from(k1);
            continue;
        }

        let mut k1 = get_small_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        k1 ^= rand_mask(s.rNToGo);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        s.state_out_len = Int32::from(k1).checked_add(4).ok_or(BZ_DATA_ERROR)?;
        let mut next_k0 = get_small_value(s)?;
        rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
        next_k0 ^= rand_mask(s.rNToGo);
        s.k0 = Int32::from(next_k0);
        s.nblock_used = s.nblock_used.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    }
}

fn restore_locals(s: &DState) -> ParserLocals {
    ParserLocals {
        i: s.save_i,
        j: s.save_j,
        t: s.save_t,
        alphaSize: s.save_alphaSize,
        nGroups: s.save_nGroups,
        nSelectors: s.save_nSelectors,
        EOB: s.save_EOB,
        groupNo: s.save_groupNo,
        groupPos: s.save_groupPos,
        nextSym: s.save_nextSym,
        nblockMAX: s.save_nblockMAX,
        nblock: s.save_nblock,
        es: s.save_es,
        N: s.save_N,
        curr: s.save_curr,
        zt: s.save_zt,
        zn: s.save_zn,
        zvec: s.save_zvec,
        zj: s.save_zj,
        gSel: s.save_gSel,
        gMinlen: s.save_gMinlen,
    }
}

unsafe fn sync_saved_group_ptrs(s: &mut DState, locals: &ParserLocals) {
    if let Ok(group) = usize::try_from(locals.gSel) {
        if group < s.limit.len() {
            s.save_gLimit = s.limit[group].as_mut_ptr();
            s.save_gBase = s.base[group].as_mut_ptr();
            s.save_gPerm = s.perm[group].as_mut_ptr();
            return;
        }
    }
    s.save_gLimit = ptr::null_mut();
    s.save_gBase = ptr::null_mut();
    s.save_gPerm = ptr::null_mut();
}

unsafe fn save_locals(s: &mut DState, locals: &ParserLocals) {
    s.save_i = locals.i;
    s.save_j = locals.j;
    s.save_t = locals.t;
    s.save_alphaSize = locals.alphaSize;
    s.save_nGroups = locals.nGroups;
    s.save_nSelectors = locals.nSelectors;
    s.save_EOB = locals.EOB;
    s.save_groupNo = locals.groupNo;
    s.save_groupPos = locals.groupPos;
    s.save_nextSym = locals.nextSym;
    s.save_nblockMAX = locals.nblockMAX;
    s.save_nblock = locals.nblock;
    s.save_es = locals.es;
    s.save_N = locals.N;
    s.save_curr = locals.curr;
    s.save_zt = locals.zt;
    s.save_zn = locals.zn;
    s.save_zvec = locals.zvec;
    s.save_zj = locals.zj;
    s.save_gSel = locals.gSel;
    s.save_gMinlen = locals.gMinlen;
    sync_saved_group_ptrs(s, locals);
}

unsafe fn return_with_code(s: &mut DState, locals: &ParserLocals, code: c_int) -> c_int {
    save_locals(s, locals);
    code
}

unsafe fn make_maps_d(s: &mut DState) {
    s.nInUse = 0;
    for (idx, in_use) in s.inUse.iter().enumerate() {
        if *in_use != 0 {
            s.seqToUnseq[s.nInUse as usize] = idx as UChar;
            s.nInUse += 1;
        }
    }
}

unsafe fn get_bits(s: &mut DState, label: DecodeState, bits: Int32) -> Option<UInt32> {
    s.state = label as Int32;
    let bits_u32 = u32::try_from(bits).ok()?;
    let mask = (1u32 << bits_u32) - 1;

    while s.bsLive < bits {
        let strm = &mut *s.strm;
        if strm.avail_in == 0 {
            return None;
        }
        if strm.next_in.is_null() {
            return None;
        }
        let byte = *strm.next_in.cast::<u8>();
        s.bsBuff = (s.bsBuff << 8) | UInt32::from(byte);
        s.bsLive += 8;
        strm.next_in = strm.next_in.add(1);
        strm.avail_in -= 1;
        increment_total_in(strm);
    }

    let shift = (s.bsLive - bits) as u32;
    let value = (s.bsBuff >> shift) & mask;
    s.bsLive -= bits;
    Some(value)
}

unsafe fn get_bit(s: &mut DState, label: DecodeState) -> Option<UChar> {
    get_bits(s, label, 1).map(|value| value as UChar)
}

unsafe fn get_uchar(s: &mut DState, label: DecodeState) -> Option<UChar> {
    get_bits(s, label, 8).map(|value| value as UChar)
}

unsafe fn group_tables<'a>(
    s: &'a DState,
    locals: &ParserLocals,
) -> Result<
    (
        &'a [Int32; BZ_MAX_ALPHA_SIZE],
        &'a [Int32; BZ_MAX_ALPHA_SIZE],
        &'a [Int32; BZ_MAX_ALPHA_SIZE],
    ),
    c_int,
> {
    let group = usize::try_from(locals.gSel).map_err(|_| BZ_DATA_ERROR)?;
    let limit = s.limit.get(group).ok_or(BZ_DATA_ERROR)?;
    let base = s.base.get(group).ok_or(BZ_DATA_ERROR)?;
    let perm = s.perm.get(group).ok_or(BZ_DATA_ERROR)?;
    Ok((limit, base, perm))
}

unsafe fn select_mtf_group(s: &mut DState, locals: &mut ParserLocals) -> Result<(), c_int> {
    locals.groupNo = locals.groupNo.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    if locals.groupNo >= locals.nSelectors {
        return Err(BZ_DATA_ERROR);
    }
    locals.groupPos = BZ_G_SIZE;
    let selector = *s
        .selector
        .get(usize::try_from(locals.groupNo).map_err(|_| BZ_DATA_ERROR)?)
        .ok_or(BZ_DATA_ERROR)?;
    locals.gSel = Int32::from(selector);
    if locals.gSel < 0 || locals.gSel >= locals.nGroups {
        return Err(BZ_DATA_ERROR);
    }
    locals.gMinlen = *s
        .minLens
        .get(usize::try_from(locals.gSel).map_err(|_| BZ_DATA_ERROR)?)
        .ok_or(BZ_DATA_ERROR)?;
    sync_saved_group_ptrs(s, locals);
    Ok(())
}

unsafe fn get_mtf_val(
    s: &mut DState,
    locals: &mut ParserLocals,
    label1: DecodeState,
    label2: DecodeState,
) -> Result<Option<Int32>, c_int> {
    let resume = DecodeState::from_raw(s.state).ok_or(BZ_DATA_ERROR)?;
    let fresh_entry =
        (resume != label1 && resume != label2) || (resume == label1 && locals.zn == 0);
    if fresh_entry {
        if locals.groupPos == 0 {
            select_mtf_group(s, locals)?;
        }
        locals.groupPos = locals.groupPos.checked_sub(1).ok_or(BZ_DATA_ERROR)?;
        locals.zn = locals.gMinlen;
        let Some(zvec) = get_bits(s, label1, locals.zn) else {
            return Ok(None);
        };
        locals.zvec = zvec as Int32;
    } else if resume == label1 {
        let Some(zvec) = get_bits(s, label1, locals.zn) else {
            return Ok(None);
        };
        locals.zvec = zvec as Int32;
    } else {
        let Some(zj) = get_bit(s, label2) else {
            return Ok(None);
        };
        locals.zj = Int32::from(zj);
        locals.zvec = locals
            .zvec
            .checked_shl(1)
            .and_then(|value| value.checked_add(locals.zj))
            .ok_or(BZ_DATA_ERROR)?;
    }

    loop {
        if locals.zn > MAX_HUFFMAN_LEN {
            return Err(BZ_DATA_ERROR);
        }
        let (limit, base, perm) = group_tables(s, locals)?;
        let limit_entry = *limit
            .get(usize::try_from(locals.zn).map_err(|_| BZ_DATA_ERROR)?)
            .ok_or(BZ_DATA_ERROR)?;
        if locals.zvec <= limit_entry {
            let base_entry = *base
                .get(usize::try_from(locals.zn).map_err(|_| BZ_DATA_ERROR)?)
                .ok_or(BZ_DATA_ERROR)?;
            let perm_index = locals.zvec.checked_sub(base_entry).ok_or(BZ_DATA_ERROR)?;
            if perm_index < 0 || perm_index >= BZ_MAX_ALPHA_SIZE as Int32 {
                return Err(BZ_DATA_ERROR);
            }
            return Ok(Some(
                *perm
                    .get(usize::try_from(perm_index).map_err(|_| BZ_DATA_ERROR)?)
                    .ok_or(BZ_DATA_ERROR)?,
            ));
        }
        locals.zn = locals.zn.checked_add(1).ok_or(BZ_DATA_ERROR)?;
        let Some(zj) = get_bit(s, label2) else {
            return Ok(None);
        };
        locals.zj = Int32::from(zj);
        locals.zvec = locals
            .zvec
            .checked_shl(1)
            .and_then(|value| value.checked_add(locals.zj))
            .ok_or(BZ_DATA_ERROR)?;
    }
}

unsafe fn decode_regular_mtf_symbol(s: &mut DState, next_sym: Int32) -> Result<UChar, c_int> {
    let mut nn = UInt32::try_from(next_sym.checked_sub(1).ok_or(BZ_DATA_ERROR)?)
        .map_err(|_| BZ_DATA_ERROR)?;

    if nn < 16 {
        let pp = usize::try_from(s.mtfbase[0]).map_err(|_| BZ_DATA_ERROR)?;
        let idx = pp.checked_add(nn as usize).ok_or(BZ_DATA_ERROR)?;
        let uc = *s.mtfa.get(idx).ok_or(BZ_DATA_ERROR)?;
        while nn > 3 {
            let z = pp.checked_add(nn as usize).ok_or(BZ_DATA_ERROR)?;
            let prev1 = *s.mtfa.get(z - 1).ok_or(BZ_DATA_ERROR)?;
            let prev2 = *s.mtfa.get(z - 2).ok_or(BZ_DATA_ERROR)?;
            let prev3 = *s.mtfa.get(z - 3).ok_or(BZ_DATA_ERROR)?;
            let prev4 = *s.mtfa.get(z - 4).ok_or(BZ_DATA_ERROR)?;
            s.mtfa[z] = prev1;
            s.mtfa[z - 1] = prev2;
            s.mtfa[z - 2] = prev3;
            s.mtfa[z - 3] = prev4;
            nn -= 4;
        }
        while nn > 0 {
            let z = pp.checked_add(nn as usize).ok_or(BZ_DATA_ERROR)?;
            let prev = *s.mtfa.get(z - 1).ok_or(BZ_DATA_ERROR)?;
            s.mtfa[z] = prev;
            nn -= 1;
        }
        s.mtfa[pp] = uc;
        Ok(uc)
    } else {
        let mut lno = usize::try_from(nn / 16).map_err(|_| BZ_DATA_ERROR)?;
        let off = usize::try_from(nn % 16).map_err(|_| BZ_DATA_ERROR)?;
        let base = usize::try_from(*s.mtfbase.get(lno).ok_or(BZ_DATA_ERROR)?)
            .map_err(|_| BZ_DATA_ERROR)?;
        let mut pp = base.checked_add(off).ok_or(BZ_DATA_ERROR)?;
        let uc = *s.mtfa.get(pp).ok_or(BZ_DATA_ERROR)?;
        while pp > base {
            let prev = *s.mtfa.get(pp - 1).ok_or(BZ_DATA_ERROR)?;
            s.mtfa[pp] = prev;
            pp -= 1;
        }
        s.mtfbase[lno] = s.mtfbase[lno].checked_add(1).ok_or(BZ_DATA_ERROR)?;
        while lno > 0 {
            s.mtfbase[lno] = s.mtfbase[lno].checked_sub(1).ok_or(BZ_DATA_ERROR)?;
            let dst = usize::try_from(s.mtfbase[lno]).map_err(|_| BZ_DATA_ERROR)?;
            let src = usize::try_from(s.mtfbase[lno - 1].checked_add(15).ok_or(BZ_DATA_ERROR)?)
                .map_err(|_| BZ_DATA_ERROR)?;
            s.mtfa[dst] = *s.mtfa.get(src).ok_or(BZ_DATA_ERROR)?;
            lno -= 1;
        }
        s.mtfbase[0] = s.mtfbase[0].checked_sub(1).ok_or(BZ_DATA_ERROR)?;
        let head = usize::try_from(s.mtfbase[0]).map_err(|_| BZ_DATA_ERROR)?;
        s.mtfa[head] = uc;
        if s.mtfbase[0] == 0 {
            let mut kk = (s.mtfa.len() - 1) as Int32;
            for ii in (0..s.mtfbase.len()).rev() {
                for jj in (0..16usize).rev() {
                    let src = usize::try_from(
                        s.mtfbase[ii]
                            .checked_add(jj as Int32)
                            .ok_or(BZ_DATA_ERROR)?,
                    )
                    .map_err(|_| BZ_DATA_ERROR)?;
                    let dst = usize::try_from(kk).map_err(|_| BZ_DATA_ERROR)?;
                    s.mtfa[dst] = *s.mtfa.get(src).ok_or(BZ_DATA_ERROR)?;
                    kk = kk.checked_sub(1).ok_or(BZ_DATA_ERROR)?;
                }
                s.mtfbase[ii] = kk.checked_add(1).ok_or(BZ_DATA_ERROR)?;
            }
        }
        Ok(uc)
    }
}

unsafe fn store_decoded_byte(
    s: &mut DState,
    locals: &mut ParserLocals,
    byte: UChar,
) -> Result<(), c_int> {
    if locals.nblock >= locals.nblockMAX {
        return Err(BZ_DATA_ERROR);
    }
    let nblock = usize::try_from(locals.nblock).map_err(|_| BZ_DATA_ERROR)?;
    if s.smallDecompress != 0 {
        let ll16 = ll16_slice_mut(s)?;
        *ll16.get_mut(nblock).ok_or(BZ_DATA_ERROR)? = UInt16::from(byte);
    } else {
        let tt = tt_slice_mut(s)?;
        *tt.get_mut(nblock).ok_or(BZ_DATA_ERROR)? = UInt32::from(byte);
    }
    locals.nblock = locals.nblock.checked_add(1).ok_or(BZ_DATA_ERROR)?;
    Ok(())
}

unsafe fn prepare_output_block(s: &mut DState, locals: &mut ParserLocals) -> Result<(), c_int> {
    if locals.nblock == 0 {
        return Err(BZ_DATA_ERROR);
    }
    if s.origPtr < 0 || s.origPtr >= locals.nblock {
        return Err(BZ_DATA_ERROR);
    }

    for &entry in &s.unzftab {
        if entry < 0 || entry > locals.nblock {
            return Err(BZ_DATA_ERROR);
        }
    }

    s.cftab[0] = 0;
    for i in 1..=256 {
        s.cftab[i] = s.unzftab[i - 1];
    }
    for i in 1..=256 {
        s.cftab[i] = s.cftab[i]
            .checked_add(s.cftab[i - 1])
            .ok_or(BZ_DATA_ERROR)?;
    }
    for &entry in &s.cftab {
        if entry < 0 || entry > locals.nblock {
            return Err(BZ_DATA_ERROR);
        }
    }
    for i in 1..=256 {
        if s.cftab[i - 1] > s.cftab[i] {
            return Err(BZ_DATA_ERROR);
        }
    }

    s.state_out_len = 0;
    s.state_out_ch = 0;
    s.calculatedBlockCRC = bz_crc_init();
    s.state = DecodeState::Output as Int32;

    if s.smallDecompress != 0 {
        s.cftabCopy.copy_from_slice(&s.cftab);
        for i in 0..usize::try_from(locals.nblock).map_err(|_| BZ_DATA_ERROR)? {
            let uc = UChar::try_from(*ll16_slice_mut(s)?.get(i).ok_or(BZ_DATA_ERROR)?)
                .map_err(|_| BZ_DATA_ERROR)?;
            let pos = usize::try_from(s.cftabCopy[usize::from(uc)]).map_err(|_| BZ_DATA_ERROR)?;
            set_ll(s, i, UInt32::try_from(pos).map_err(|_| BZ_DATA_ERROR)?)?;
            s.cftabCopy[usize::from(uc)] = s.cftabCopy[usize::from(uc)]
                .checked_add(1)
                .ok_or(BZ_DATA_ERROR)?;
        }

        let mut i = usize::try_from(s.origPtr).map_err(|_| BZ_DATA_ERROR)?;
        let mut j = usize::try_from(get_ll(s, i)?).map_err(|_| BZ_DATA_ERROR)?;
        loop {
            let tmp = usize::try_from(get_ll(s, j)?).map_err(|_| BZ_DATA_ERROR)?;
            set_ll(s, j, UInt32::try_from(i).map_err(|_| BZ_DATA_ERROR)?)?;
            i = j;
            j = tmp;
            if i == usize::try_from(s.origPtr).map_err(|_| BZ_DATA_ERROR)? {
                break;
            }
        }

        s.tPos = UInt32::try_from(s.origPtr).map_err(|_| BZ_DATA_ERROR)?;
        s.nblock_used = 0;
        if s.blockRandomised != 0 {
            rand_init(&mut s.rNToGo, &mut s.rTPos);
            s.k0 = Int32::from(get_small_value(s)?);
            s.nblock_used = 1;
            rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
            s.k0 ^= Int32::from(rand_mask(s.rNToGo));
        } else {
            s.k0 = Int32::from(get_small_value(s)?);
            s.nblock_used = 1;
        }
    } else {
        for i in 0..usize::try_from(locals.nblock).map_err(|_| BZ_DATA_ERROR)? {
            let uc = {
                let tt = tt_slice_mut(s)?;
                (tt.get(i).ok_or(BZ_DATA_ERROR)? & 0xff) as usize
            };
            let pos = usize::try_from(s.cftab[uc]).map_err(|_| BZ_DATA_ERROR)?;
            let value = UInt32::try_from(i).map_err(|_| BZ_DATA_ERROR)?;
            {
                let tt = tt_slice_mut(s)?;
                let entry = tt.get_mut(pos).ok_or(BZ_DATA_ERROR)?;
                *entry |= value.checked_shl(8).ok_or(BZ_DATA_ERROR)?;
            }
            s.cftab[uc] = s.cftab[uc].checked_add(1).ok_or(BZ_DATA_ERROR)?;
        }

        let orig_ptr = usize::try_from(s.origPtr).map_err(|_| BZ_DATA_ERROR)?;
        s.tPos = *tt_slice_mut(s)?.get(orig_ptr).ok_or(BZ_DATA_ERROR)? >> 8;
        s.nblock_used = 0;
        if s.blockRandomised != 0 {
            rand_init(&mut s.rNToGo, &mut s.rTPos);
            s.k0 = Int32::from(get_fast_value(s)?);
            s.nblock_used = 1;
            rand_update_mask(&mut s.rNToGo, &mut s.rTPos);
            s.k0 ^= Int32::from(rand_mask(s.rNToGo));
        } else {
            s.k0 = Int32::from(get_fast_value(s)?);
            s.nblock_used = 1;
        }
    }

    Ok(())
}

unsafe fn validate_stream_buffers(strm: &bz_stream) -> bool {
    !(strm.avail_in > 0 && strm.next_in.is_null() || strm.avail_out > 0 && strm.next_out.is_null())
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzDecompressInit(
    strm: *mut bz_stream,
    verbosity: c_int,
    small: c_int,
) -> c_int {
    if !bz_config_ok() {
        return BZ_CONFIG_ERROR;
    }
    if strm.is_null() || (small != 0 && small != 1) || !(0..=MAX_VERBOSITY).contains(&verbosity) {
        return BZ_PARAM_ERROR;
    }

    ensure_default_allocators(strm, Some(default_bzalloc), Some(default_bzfree));
    reset_stream_totals(strm);

    let state = match alloc_zeroed_with_bzalloc::<DState>(strm) {
        Ok(ptr) => ptr,
        Err(code) => return code,
    };
    (*state).strm = strm;
    (*state).state = DecodeState::Magic1 as Int32;
    (*state).bsLive = 0;
    (*state).bsBuff = 0;
    (*state).calculatedCombinedCRC = 0;
    (*state).smallDecompress = small as UChar;
    (*state).ll4 = ptr::null_mut();
    (*state).ll16 = ptr::null_mut();
    (*state).tt = ptr::null_mut();
    (*state).currBlockNo = 0;
    (*state).verbosity = verbosity;
    (*strm).state = state.cast();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzDecompress(strm: *mut bz_stream) -> c_int {
    if strm.is_null() {
        return BZ_PARAM_ERROR;
    }
    if (*strm).state.is_null() {
        return BZ_PARAM_ERROR;
    }
    if !validate_stream_buffers(&*strm) {
        return BZ_PARAM_ERROR;
    }

    let s = &mut *stream_state::<DState>(strm);
    if s.strm != strm {
        return BZ_PARAM_ERROR;
    }

    loop {
        if s.state == DecodeState::Idle as Int32 {
            return BZ_SEQUENCE_ERROR;
        }
        if s.state == DecodeState::Output as Int32 {
            let corrupt = if s.smallDecompress != 0 {
                if s.blockRandomised != 0 {
                    output_small_randomized(s)
                } else {
                    output_small_nonrandomized(s)
                }
            } else if s.blockRandomised != 0 {
                output_fast_randomized(s)
            } else {
                output_fast_nonrandomized(s)
            };

            match corrupt {
                Err(BZ_OK) => return BZ_OK,
                Err(code) => return code,
                Ok(true) => return BZ_DATA_ERROR,
                Ok(false) => {
                    if s.nblock_used == s.save_nblock + 1 && s.state_out_len == 0 {
                        s.calculatedBlockCRC = bz_crc_finalize(s.calculatedBlockCRC);
                        if s.calculatedBlockCRC != s.storedBlockCRC {
                            return BZ_DATA_ERROR;
                        }
                        s.calculatedCombinedCRC =
                            (s.calculatedCombinedCRC << 1) | (s.calculatedCombinedCRC >> 31);
                        s.calculatedCombinedCRC ^= s.calculatedBlockCRC;
                        s.state = DecodeState::BlkHdr1 as Int32;
                    } else {
                        return BZ_OK;
                    }
                }
            }
        }

        if s.state >= DecodeState::Magic1 as Int32 {
            let ret = BZ2_decompress(s);
            if ret == BZ_STREAM_END {
                if s.calculatedCombinedCRC != s.storedCombinedCRC {
                    return BZ_DATA_ERROR;
                }
                return ret;
            }
            if s.state != DecodeState::Output as Int32 {
                return ret;
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzDecompressEnd(strm: *mut bz_stream) -> c_int {
    if strm.is_null() {
        return BZ_PARAM_ERROR;
    }
    if (*strm).state.is_null() {
        return BZ_PARAM_ERROR;
    }

    let state = stream_state::<DState>(strm);
    if (*state).strm != strm {
        return BZ_PARAM_ERROR;
    }

    release_block_storage(state);
    free_with_bzfree(strm, state);
    (*strm).state = ptr::null_mut();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_indexIntoF(index: Int32, cftab: *mut Int32) -> Int32 {
    if cftab.is_null() {
        return 0;
    }

    let cftab = slice::from_raw_parts(cftab, 257);
    let mut nb = 0usize;
    let mut na = 256usize;
    while na - nb != 1 {
        let mid = (nb + na) >> 1;
        if index >= cftab[mid] {
            nb = mid;
        } else {
            na = mid;
        }
    }
    nb as Int32
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_decompress(state: *mut DState) -> Int32 {
    let s = &mut *state;
    if s.state == DecodeState::Magic1 as Int32 {
        save_locals(s, &ParserLocals::default());
    }
    let mut locals = restore_locals(s);
    let Some(mut current) = DecodeState::from_raw(s.state) else {
        return return_with_code(s, &locals, BZ_DATA_ERROR);
    };

    loop {
        match current {
            DecodeState::Magic1 => {
                let Some(uc) = get_uchar(s, DecodeState::Magic1) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != BZ_HDR_B {
                    return return_with_code(s, &locals, BZ_DATA_ERROR_MAGIC);
                }
                current = DecodeState::Magic2;
                s.state = current as Int32;
            }
            DecodeState::Magic2 => {
                let Some(uc) = get_uchar(s, DecodeState::Magic2) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != BZ_HDR_Z {
                    return return_with_code(s, &locals, BZ_DATA_ERROR_MAGIC);
                }
                current = DecodeState::Magic3;
                s.state = current as Int32;
            }
            DecodeState::Magic3 => {
                let Some(uc) = get_uchar(s, DecodeState::Magic3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != BZ_HDR_h {
                    return return_with_code(s, &locals, BZ_DATA_ERROR_MAGIC);
                }
                current = DecodeState::Magic4;
                s.state = current as Int32;
            }
            DecodeState::Magic4 => {
                let Some(block_size) = get_bits(s, DecodeState::Magic4, 8) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.blockSize100k = block_size as Int32;
                if !(BZ_HDR_0 + 1..=BZ_HDR_0 + 9).contains(&s.blockSize100k) {
                    return return_with_code(s, &locals, BZ_DATA_ERROR_MAGIC);
                }
                s.blockSize100k -= BZ_HDR_0;
                let alloc_code = allocate_block_storage(state);
                if alloc_code != BZ_OK {
                    return return_with_code(s, &locals, alloc_code);
                }
                current = DecodeState::BlkHdr1;
                s.state = current as Int32;
            }
            DecodeState::BlkHdr1 => {
                let Some(uc) = get_uchar(s, DecodeState::BlkHdr1) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc == 0x17 {
                    current = DecodeState::EndHdr2;
                    s.state = current as Int32;
                    continue;
                }
                if uc != 0x31 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::BlkHdr2;
                s.state = current as Int32;
            }
            DecodeState::BlkHdr2 => {
                let Some(uc) = get_uchar(s, DecodeState::BlkHdr2) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x41 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::BlkHdr3;
                s.state = current as Int32;
            }
            DecodeState::BlkHdr3 => {
                let Some(uc) = get_uchar(s, DecodeState::BlkHdr3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x59 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::BlkHdr4;
                s.state = current as Int32;
            }
            DecodeState::BlkHdr4 => {
                let Some(uc) = get_uchar(s, DecodeState::BlkHdr4) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x26 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::BlkHdr5;
                s.state = current as Int32;
            }
            DecodeState::BlkHdr5 => {
                let Some(uc) = get_uchar(s, DecodeState::BlkHdr5) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x53 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::BlkHdr6;
                s.state = current as Int32;
            }
            DecodeState::BlkHdr6 => {
                let Some(uc) = get_uchar(s, DecodeState::BlkHdr6) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x59 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                s.currBlockNo = or_return_code!(
                    s.currBlockNo.checked_add(1).ok_or(BZ_DATA_ERROR),
                    s,
                    &locals
                );
                s.storedBlockCRC = 0;
                current = DecodeState::Bcrc1;
                s.state = current as Int32;
            }
            DecodeState::Bcrc1 => {
                let Some(uc) = get_uchar(s, DecodeState::Bcrc1) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedBlockCRC = UInt32::from(uc);
                current = DecodeState::Bcrc2;
                s.state = current as Int32;
            }
            DecodeState::Bcrc2 => {
                let Some(uc) = get_uchar(s, DecodeState::Bcrc2) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedBlockCRC = (s.storedBlockCRC << 8) | UInt32::from(uc);
                current = DecodeState::Bcrc3;
                s.state = current as Int32;
            }
            DecodeState::Bcrc3 => {
                let Some(uc) = get_uchar(s, DecodeState::Bcrc3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedBlockCRC = (s.storedBlockCRC << 8) | UInt32::from(uc);
                current = DecodeState::Bcrc4;
                s.state = current as Int32;
            }
            DecodeState::Bcrc4 => {
                let Some(uc) = get_uchar(s, DecodeState::Bcrc4) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedBlockCRC = (s.storedBlockCRC << 8) | UInt32::from(uc);
                current = DecodeState::RandBit;
                s.state = current as Int32;
            }
            DecodeState::RandBit => {
                let Some(randomised) = get_bits(s, DecodeState::RandBit, 1) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.blockRandomised = randomised as UChar;
                s.origPtr = 0;
                current = DecodeState::OrigPtr1;
                s.state = current as Int32;
            }
            DecodeState::OrigPtr1 => {
                let Some(uc) = get_uchar(s, DecodeState::OrigPtr1) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.origPtr = Int32::from(uc);
                current = DecodeState::OrigPtr2;
                s.state = current as Int32;
            }
            DecodeState::OrigPtr2 => {
                let Some(uc) = get_uchar(s, DecodeState::OrigPtr2) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.origPtr = (s.origPtr << 8) | Int32::from(uc);
                current = DecodeState::OrigPtr3;
                s.state = current as Int32;
            }
            DecodeState::OrigPtr3 => {
                let Some(uc) = get_uchar(s, DecodeState::OrigPtr3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.origPtr = (s.origPtr << 8) | Int32::from(uc);
                let block_max = or_return_code!(
                    s.blockSize100k.checked_mul(100_000).ok_or(BZ_DATA_ERROR),
                    s,
                    &locals
                );
                let max_orig_ptr = or_return_code!(
                    10i32.checked_add(block_max).ok_or(BZ_DATA_ERROR),
                    s,
                    &locals
                );
                if s.origPtr < 0 || s.origPtr > max_orig_ptr {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                locals.i = 0;
                current = DecodeState::Mapping1;
                s.state = current as Int32;
            }
            DecodeState::Mapping1 => {
                while locals.i < 16 {
                    let Some(bit) = get_bit(s, DecodeState::Mapping1) else {
                        return return_with_code(s, &locals, BZ_OK);
                    };
                    let i = or_return_code!(usize_from_i32(locals.i), s, &locals);
                    s.inUse16[i] = if bit == 1 { 1 } else { 0 };
                    locals.i += 1;
                }
                s.inUse.fill(0);
                locals.i = 0;
                locals.j = 0;
                current = DecodeState::Mapping2;
                s.state = current as Int32;
            }
            DecodeState::Mapping2 => {
                while locals.i < 16 {
                    let i = or_return_code!(usize_from_i32(locals.i), s, &locals);
                    if s.inUse16[i] != 0 {
                        while locals.j < 16 {
                            let Some(bit) = get_bit(s, DecodeState::Mapping2) else {
                                return return_with_code(s, &locals, BZ_OK);
                            };
                            if bit == 1 {
                                let idx = or_return_code!(
                                    usize_from_i32(locals.i * 16 + locals.j),
                                    s,
                                    &locals
                                );
                                s.inUse[idx] = 1;
                            }
                            locals.j += 1;
                        }
                    }
                    locals.i += 1;
                    locals.j = 0;
                }
                make_maps_d(s);
                if s.nInUse == 0 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                locals.alphaSize =
                    or_return_code!(s.nInUse.checked_add(2).ok_or(BZ_DATA_ERROR), s, &locals);
                if locals.alphaSize < 0 || locals.alphaSize > BZ_MAX_ALPHA_SIZE as Int32 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::Selector1;
                s.state = current as Int32;
            }
            DecodeState::Selector1 => {
                let Some(value) = get_bits(s, DecodeState::Selector1, 3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                locals.nGroups = value as Int32;
                if locals.nGroups < 2 || locals.nGroups > s.limit.len() as Int32 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::Selector2;
                s.state = current as Int32;
            }
            DecodeState::Selector2 => {
                let Some(value) = get_bits(s, DecodeState::Selector2, 15) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                locals.nSelectors = value as Int32;
                if locals.nSelectors < 1 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                locals.i = 0;
                locals.j = 0;
                current = DecodeState::Selector3;
                s.state = current as Int32;
            }
            DecodeState::Selector3 => {
                while locals.i < locals.nSelectors {
                    let Some(bit) = get_bit(s, DecodeState::Selector3) else {
                        return return_with_code(s, &locals, BZ_OK);
                    };
                    if bit == 0 {
                        if locals.i < BZ_MAX_SELECTORS as Int32 {
                            let idx = or_return_code!(usize_from_i32(locals.i), s, &locals);
                            s.selectorMtf[idx] =
                                or_return_code!(uchar_from_i32(locals.j), s, &locals);
                        }
                        locals.i += 1;
                        locals.j = 0;
                    } else {
                        locals.j += 1;
                        if locals.j >= locals.nGroups {
                            return return_with_code(s, &locals, BZ_DATA_ERROR);
                        }
                    }
                }
                // Keep the 1.0.8 semantics for nSelectors so CVE-2019-12900 stays fixed
                // without rejecting streams that round the selector count up.
                if locals.nSelectors > BZ_MAX_SELECTORS as Int32 {
                    locals.nSelectors = BZ_MAX_SELECTORS as Int32;
                }
                let mut pos = [0u8; crate::constants::BZ_N_GROUPS];
                let n_groups = or_return_code!(usize_from_i32(locals.nGroups), s, &locals);
                for value in 0..n_groups {
                    pos[value] = value as u8;
                }
                let n_selectors = or_return_code!(usize_from_i32(locals.nSelectors), s, &locals);
                for idx in 0..n_selectors {
                    let mut value = usize::from(s.selectorMtf[idx]);
                    if value >= n_groups {
                        return return_with_code(s, &locals, BZ_DATA_ERROR);
                    }
                    let tmp = pos[value];
                    while value > 0 {
                        pos[value] = pos[value - 1];
                        value -= 1;
                    }
                    pos[0] = tmp;
                    s.selector[idx] = tmp;
                }
                locals.t = 0;
                current = DecodeState::Coding1;
                s.state = current as Int32;
            }
            DecodeState::Coding1 | DecodeState::Coding2 | DecodeState::Coding3 => {
                while locals.t < locals.nGroups {
                    if current == DecodeState::Coding1 {
                        let Some(value) = get_bits(s, DecodeState::Coding1, 5) else {
                            return return_with_code(s, &locals, BZ_OK);
                        };
                        locals.curr = value as Int32;
                        locals.i = 0;
                        current = DecodeState::Coding2;
                        s.state = current as Int32;
                    }

                    while locals.i < locals.alphaSize {
                        if locals.curr < 1 || locals.curr > MAX_HUFFMAN_LEN {
                            return return_with_code(s, &locals, BZ_DATA_ERROR);
                        }
                        if current == DecodeState::Coding2 {
                            let Some(bit) = get_bit(s, DecodeState::Coding2) else {
                                return return_with_code(s, &locals, BZ_OK);
                            };
                            if bit == 0 {
                                let t = or_return_code!(usize_from_i32(locals.t), s, &locals);
                                let i = or_return_code!(usize_from_i32(locals.i), s, &locals);
                                s.len[t][i] =
                                    or_return_code!(uchar_from_i32(locals.curr), s, &locals);
                                locals.i += 1;
                                continue;
                            }
                            current = DecodeState::Coding3;
                            s.state = current as Int32;
                        }

                        let Some(bit) = get_bit(s, DecodeState::Coding3) else {
                            return return_with_code(s, &locals, BZ_OK);
                        };
                        locals.curr = if bit == 0 {
                            or_return_code!(
                                locals.curr.checked_add(1).ok_or(BZ_DATA_ERROR),
                                s,
                                &locals
                            )
                        } else {
                            or_return_code!(
                                locals.curr.checked_sub(1).ok_or(BZ_DATA_ERROR),
                                s,
                                &locals
                            )
                        };
                        current = DecodeState::Coding2;
                        s.state = current as Int32;
                    }

                    locals.t += 1;
                    if locals.t < locals.nGroups {
                        current = DecodeState::Coding1;
                        s.state = current as Int32;
                    }
                }

                let n_groups = or_return_code!(usize_from_i32(locals.nGroups), s, &locals);
                let alpha_size = or_return_code!(usize_from_i32(locals.alphaSize), s, &locals);
                for t in 0..n_groups {
                    let mut min_len = 32;
                    let mut max_len = 0;
                    for i in 0..alpha_size {
                        let length = Int32::from(s.len[t][i]);
                        if length > max_len {
                            max_len = length;
                        }
                        if length < min_len {
                            min_len = length;
                        }
                    }
                    if hb_create_decode_tables_checked(
                        &mut s.limit[t],
                        &mut s.base[t],
                        &mut s.perm[t],
                        &s.len[t],
                        min_len,
                        max_len,
                        locals.alphaSize,
                    )
                    .is_err()
                    {
                        return return_with_code(s, &locals, BZ_DATA_ERROR);
                    }
                    s.minLens[t] = min_len;
                }

                locals.EOB =
                    or_return_code!(s.nInUse.checked_add(1).ok_or(BZ_DATA_ERROR), s, &locals);
                locals.nblockMAX = or_return_code!(
                    s.blockSize100k.checked_mul(100_000).ok_or(BZ_DATA_ERROR),
                    s,
                    &locals
                );
                locals.groupNo = -1;
                locals.groupPos = 0;
                s.unzftab.fill(0);

                let mut kk = (s.mtfa.len() - 1) as Int32;
                for ii in (0..s.mtfbase.len()).rev() {
                    for jj in (0..16).rev() {
                        let dst = or_return_code!(usize_from_i32(kk), s, &locals);
                        s.mtfa[dst] = (ii * 16 + jj) as UChar;
                        kk -= 1;
                    }
                    s.mtfbase[ii] = kk + 1;
                }

                locals.nblock = 0;
                locals.zn = 0;
                current = DecodeState::Mtf1;
                s.state = current as Int32;
            }
            DecodeState::Mtf1
            | DecodeState::Mtf2
            | DecodeState::Mtf3
            | DecodeState::Mtf4
            | DecodeState::Mtf5
            | DecodeState::Mtf6 => {
                let mut resuming_run = false;
                if matches!(current, DecodeState::Mtf1 | DecodeState::Mtf2) {
                    locals.nextSym =
                        match get_mtf_val(s, &mut locals, DecodeState::Mtf1, DecodeState::Mtf2) {
                            Ok(Some(next_sym)) => next_sym,
                            Ok(None) => return return_with_code(s, &locals, BZ_OK),
                            Err(code) => return return_with_code(s, &locals, code),
                        };
                    current = FIRST_MTF_GROUP_LABEL;
                    s.state = current as Int32;
                } else if matches!(current, DecodeState::Mtf3 | DecodeState::Mtf4) {
                    locals.nextSym =
                        match get_mtf_val(s, &mut locals, DecodeState::Mtf3, DecodeState::Mtf4) {
                            Ok(Some(next_sym)) => next_sym,
                            Ok(None) => return return_with_code(s, &locals, BZ_OK),
                            Err(code) => return return_with_code(s, &locals, code),
                        };
                    resuming_run = true;
                    current = FIRST_MTF_GROUP_LABEL;
                    s.state = current as Int32;
                } else if matches!(current, DecodeState::Mtf5 | DecodeState::Mtf6) {
                    locals.nextSym =
                        match get_mtf_val(s, &mut locals, DecodeState::Mtf5, DecodeState::Mtf6) {
                            Ok(Some(next_sym)) => next_sym,
                            Ok(None) => return return_with_code(s, &locals, BZ_OK),
                            Err(code) => return return_with_code(s, &locals, code),
                        };
                    current = FIRST_MTF_GROUP_LABEL;
                    s.state = current as Int32;
                }

                loop {
                    if resuming_run || locals.nextSym == BZ_RUNA || locals.nextSym == BZ_RUNB {
                        if !resuming_run {
                            locals.es = -1;
                            locals.N = 1;
                        }
                        resuming_run = false;
                        // CVE-2005-1260 / CVE-2010-0405: bound RUNA/RUNB growth so malformed
                        // streams cannot spin forever or overflow the run accumulator.
                        while locals.nextSym == BZ_RUNA || locals.nextSym == BZ_RUNB {
                            if locals.N >= MAX_RUN_ACCUMULATOR {
                                return return_with_code(s, &locals, BZ_DATA_ERROR);
                            }
                            let delta = if locals.nextSym == BZ_RUNA {
                                locals.N
                            } else {
                                or_return_code!(
                                    locals.N.checked_mul(2).ok_or(BZ_DATA_ERROR),
                                    s,
                                    &locals
                                )
                            };
                            locals.es = or_return_code!(
                                locals.es.checked_add(delta).ok_or(BZ_DATA_ERROR),
                                s,
                                &locals
                            );
                            locals.N = or_return_code!(
                                locals.N.checked_mul(2).ok_or(BZ_DATA_ERROR),
                                s,
                                &locals
                            );
                            locals.nextSym = match get_mtf_val(
                                s,
                                &mut locals,
                                DecodeState::Mtf3,
                                DecodeState::Mtf4,
                            ) {
                                Ok(Some(next_sym)) => next_sym,
                                Ok(None) => return return_with_code(s, &locals, BZ_OK),
                                Err(code) => return return_with_code(s, &locals, code),
                            };
                            current = FIRST_MTF_GROUP_LABEL;
                            s.state = current as Int32;
                        }

                        locals.es = or_return_code!(
                            locals.es.checked_add(1).ok_or(BZ_DATA_ERROR),
                            s,
                            &locals
                        );
                        let head = or_return_code!(usize_from_i32(s.mtfbase[0]), s, &locals);
                        let mtf_symbol = match s.mtfa.get(head).copied() {
                            Some(value) => value,
                            None => return return_with_code(s, &locals, BZ_DATA_ERROR),
                        };
                        let byte = match s.seqToUnseq.get(usize::from(mtf_symbol)).copied() {
                            Some(value) => value,
                            None => return return_with_code(s, &locals, BZ_DATA_ERROR),
                        };
                        let freq = match s.unzftab.get_mut(usize::from(byte)) {
                            Some(value) => value,
                            None => return return_with_code(s, &locals, BZ_DATA_ERROR),
                        };
                        *freq = or_return_code!(
                            freq.checked_add(locals.es).ok_or(BZ_DATA_ERROR),
                            s,
                            &locals
                        );
                        while locals.es > 0 {
                            if let Err(code) = store_decoded_byte(s, &mut locals, byte) {
                                return return_with_code(s, &locals, code);
                            }
                            locals.es -= 1;
                        }
                        current = FIRST_MTF_GROUP_LABEL;
                        s.state = current as Int32;
                        continue;
                    }

                    if locals.nextSym == locals.EOB {
                        if let Err(code) = prepare_output_block(s, &mut locals) {
                            return return_with_code(s, &locals, code);
                        }
                        return return_with_code(s, &locals, BZ_OK);
                    }

                    let mtf_symbol =
                        or_return_code!(decode_regular_mtf_symbol(s, locals.nextSym), s, &locals);
                    let byte = match s.seqToUnseq.get(usize::from(mtf_symbol)).copied() {
                        Some(value) => value,
                        None => return return_with_code(s, &locals, BZ_DATA_ERROR),
                    };
                    let freq = match s.unzftab.get_mut(usize::from(byte)) {
                        Some(value) => value,
                        None => return return_with_code(s, &locals, BZ_DATA_ERROR),
                    };
                    *freq = or_return_code!(freq.checked_add(1).ok_or(BZ_DATA_ERROR), s, &locals);
                    if let Err(code) = store_decoded_byte(s, &mut locals, byte) {
                        return return_with_code(s, &locals, code);
                    }

                    locals.nextSym =
                        match get_mtf_val(s, &mut locals, DecodeState::Mtf5, DecodeState::Mtf6) {
                            Ok(Some(next_sym)) => next_sym,
                            Ok(None) => return return_with_code(s, &locals, BZ_OK),
                            Err(code) => return return_with_code(s, &locals, code),
                        };
                    current = FIRST_MTF_GROUP_LABEL;
                    s.state = current as Int32;
                }
            }
            DecodeState::EndHdr2 => {
                let Some(uc) = get_uchar(s, DecodeState::EndHdr2) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x72 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::EndHdr3;
                s.state = current as Int32;
            }
            DecodeState::EndHdr3 => {
                let Some(uc) = get_uchar(s, DecodeState::EndHdr3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x45 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::EndHdr4;
                s.state = current as Int32;
            }
            DecodeState::EndHdr4 => {
                let Some(uc) = get_uchar(s, DecodeState::EndHdr4) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x38 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::EndHdr5;
                s.state = current as Int32;
            }
            DecodeState::EndHdr5 => {
                let Some(uc) = get_uchar(s, DecodeState::EndHdr5) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x50 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                current = DecodeState::EndHdr6;
                s.state = current as Int32;
            }
            DecodeState::EndHdr6 => {
                let Some(uc) = get_uchar(s, DecodeState::EndHdr6) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                if uc != 0x90 {
                    return return_with_code(s, &locals, BZ_DATA_ERROR);
                }
                s.storedCombinedCRC = 0;
                current = DecodeState::Ccrc1;
                s.state = current as Int32;
            }
            DecodeState::Ccrc1 => {
                let Some(uc) = get_uchar(s, DecodeState::Ccrc1) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedCombinedCRC = UInt32::from(uc);
                current = DecodeState::Ccrc2;
                s.state = current as Int32;
            }
            DecodeState::Ccrc2 => {
                let Some(uc) = get_uchar(s, DecodeState::Ccrc2) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedCombinedCRC = (s.storedCombinedCRC << 8) | UInt32::from(uc);
                current = DecodeState::Ccrc3;
                s.state = current as Int32;
            }
            DecodeState::Ccrc3 => {
                let Some(uc) = get_uchar(s, DecodeState::Ccrc3) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedCombinedCRC = (s.storedCombinedCRC << 8) | UInt32::from(uc);
                current = DecodeState::Ccrc4;
                s.state = current as Int32;
            }
            DecodeState::Ccrc4 => {
                let Some(uc) = get_uchar(s, DecodeState::Ccrc4) else {
                    return return_with_code(s, &locals, BZ_OK);
                };
                s.storedCombinedCRC = (s.storedCombinedCRC << 8) | UInt32::from(uc);
                s.state = DecodeState::Idle as Int32;
                return return_with_code(s, &locals, BZ_STREAM_END);
            }
            DecodeState::Idle | DecodeState::Output => {
                return return_with_code(s, &locals, BZ_DATA_ERROR);
            }
        }
    }
}
