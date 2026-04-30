use crate::alloc::{
    alloc_zeroed_slice_with_bzalloc, alloc_zeroed_with_bzalloc, bz_config_ok,
    ensure_default_allocators, free_with_bzfree, reset_stream_totals,
};
use crate::blocksort::BZ2_blockSort;
use crate::constants::{
    BZ_CONFIG_ERROR, BZ_FINISH, BZ_FINISH_OK, BZ_FLUSH, BZ_FLUSH_OK, BZ_G_SIZE, BZ_MAX_SELECTORS,
    BZ_M_FINISHING, BZ_M_FLUSHING, BZ_M_IDLE, BZ_M_RUNNING, BZ_N_GROUPS, BZ_N_ITERS,
    BZ_N_OVERSHOOT, BZ_OK, BZ_PARAM_ERROR, BZ_RUN, BZ_RUNA, BZ_RUNB, BZ_RUN_OK, BZ_SEQUENCE_ERROR,
    BZ_STREAM_END, BZ_S_INPUT, BZ_S_OUTPUT,
};
use crate::crc::{bz_crc_finalize, bz_crc_init, bz_crc_update};
use crate::huffman::{BZ2_hbAssignCodes, BZ2_hbMakeCodeLengths};
use crate::types::{bz_stream, stream_state, Bool, CFile, EState, Int32, UChar, UInt16, UInt32};
use core::mem::size_of;
use std::os::raw::{c_char, c_int};
use std::{ptr, slice};

const BZ_HDR_B: UChar = 0x42;
const BZ_HDR_Z: UChar = 0x5a;
const BZ_HDR_h: UChar = 0x68;
const BZ_HDR_0: Int32 = 0x30;
const BZ_LESSER_ICOST: UChar = 0;
const BZ_GREATER_ICOST: UChar = 15;
const MAX_HUFFMAN_LEN: Int32 = 17;
static BLOCK_VERBOSE_FORMAT: &[u8] =
    b"    block %d: crc = 0x%08x, combined CRC = 0x%08x, size = %d\n\0";
static FINAL_CRC_VERBOSE_FORMAT: &[u8] = b"    final combined CRC = 0x%08x\n   \0";

extern "C" {
    fn malloc(size: usize) -> *mut core::ffi::c_void;
    fn free(ptr: *mut core::ffi::c_void);
    fn fprintf(stream: *mut CFile, format: *const c_char, ...) -> c_int;
    fn BZ2_bz__AssertH__fail(errcode: c_int);
    static mut stderr: *mut CFile;
}

#[inline]
unsafe fn assert_h(cond: bool, errcode: Int32) {
    if !cond {
        BZ2_bz__AssertH__fail(errcode);
    }
}

unsafe extern "C" fn default_bzalloc(
    _opaque: *mut core::ffi::c_void,
    items: c_int,
    size: c_int,
) -> *mut core::ffi::c_void {
    if items < 0 || size < 0 {
        return ptr::null_mut();
    }
    let Ok(items) = usize::try_from(items) else {
        return ptr::null_mut();
    };
    let Ok(size) = usize::try_from(size) else {
        return ptr::null_mut();
    };
    let Some(bytes) = items.checked_mul(size) else {
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

fn block_byte_capacity(block_size_100k: Int32) -> Option<usize> {
    block_capacity(block_size_100k)?
        .checked_add(BZ_N_OVERSHOOT as usize)?
        .checked_mul(size_of::<UInt32>())
}

#[inline]
unsafe fn block_storage<'a>(block: *const UChar, block_size_100k: Int32) -> &'a [UChar] {
    slice::from_raw_parts(block, block_byte_capacity(block_size_100k).unwrap())
}

#[inline]
unsafe fn block_storage_mut<'a>(block: *mut UChar, block_size_100k: Int32) -> &'a mut [UChar] {
    slice::from_raw_parts_mut(block, block_byte_capacity(block_size_100k).unwrap())
}

#[inline]
unsafe fn mtfv_storage<'a>(mtfv: *const UInt16, block_size_100k: Int32) -> &'a [UInt16] {
    slice::from_raw_parts(mtfv, block_capacity(block_size_100k).unwrap() * 2)
}

#[inline]
unsafe fn zbits_storage<'a>(
    zbits: *const UChar,
    block: *const UChar,
    block_size_100k: Int32,
) -> &'a [UChar] {
    let total = block_byte_capacity(block_size_100k).unwrap();
    let start = usize::try_from(zbits.offset_from(block)).unwrap();
    slice::from_raw_parts(zbits, total - start)
}

#[inline]
unsafe fn zbits_storage_mut<'a>(
    zbits: *mut UChar,
    block: *mut UChar,
    block_size_100k: Int32,
) -> &'a mut [UChar] {
    let total = block_byte_capacity(block_size_100k).unwrap();
    let start = usize::try_from(zbits.offset_from(block)).unwrap();
    slice::from_raw_parts_mut(zbits, total - start)
}

#[inline]
fn push_block_byte(block: &mut [UChar], nblock: &mut Int32, byte: UChar) {
    block[usize::try_from(*nblock).unwrap()] = byte;
    *nblock += 1;
}

#[inline]
unsafe fn ptr_value(ptr: *const UInt32, block_size_100k: Int32, idx: usize) -> UInt32 {
    slice::from_raw_parts(ptr, block_capacity(block_size_100k).unwrap())[idx]
}

#[inline]
unsafe fn push_mtf_value(mtfv: *mut UInt16, block_size_100k: Int32, wr: &mut Int32, value: UInt16) {
    slice::from_raw_parts_mut(mtfv, block_capacity(block_size_100k).unwrap() * 2)
        [usize::try_from(*wr).unwrap()] = value;
    *wr += 1;
}

#[inline]
fn add_total(lo32: &mut UInt32, hi32: &mut UInt32, amount: UInt32) {
    let (next_lo, carry) = lo32.overflowing_add(amount);
    *lo32 = next_lo;
    if carry {
        *hi32 = hi32.wrapping_add(1);
    }
}

#[inline]
unsafe fn push_zbit_byte(s: &mut EState, byte: UChar) {
    let zbits = zbits_storage_mut(s.zbits, s.block, s.blockSize100k);
    zbits[usize::try_from(s.numZ).unwrap()] = byte;
    s.numZ += 1;
}

unsafe fn release_storage(s: &mut EState) {
    if block_capacity(s.blockSize100k).is_some() {
        if !s.arr1.is_null() {
            free_with_bzfree(s.strm, s.arr1);
            s.arr1 = ptr::null_mut();
            s.ptr = ptr::null_mut();
            s.mtfv = ptr::null_mut();
        }
        if !s.arr2.is_null() {
            free_with_bzfree(s.strm, s.arr2);
            s.arr2 = ptr::null_mut();
            s.block = ptr::null_mut();
            s.zbits = ptr::null_mut();
        }
    } else {
        s.arr1 = ptr::null_mut();
        s.arr2 = ptr::null_mut();
        s.ptr = ptr::null_mut();
        s.block = ptr::null_mut();
        s.mtfv = ptr::null_mut();
        s.zbits = ptr::null_mut();
    }

    if !s.ftab.is_null() {
        free_with_bzfree(s.strm, s.ftab);
        s.ftab = ptr::null_mut();
    }
}

unsafe fn prepare_new_block(s: &mut EState) {
    s.nblock = 0;
    s.numZ = 0;
    s.state_out_pos = 0;
    s.blockCRC = bz_crc_init();
    s.inUse.fill(0);
    s.blockNo += 1;
}

unsafe fn init_RL(s: &mut EState) {
    s.state_in_ch = 256;
    s.state_in_len = 0;
}

unsafe fn isempty_RL(s: &EState) -> bool {
    !(s.state_in_ch < 256 && s.state_in_len > 0)
}

unsafe fn add_pair_to_block(s: &mut EState) {
    let ch = s.state_in_ch as UChar;
    let block = block_storage_mut(s.block, s.blockSize100k);
    for _ in 0..s.state_in_len {
        s.blockCRC = bz_crc_update(s.blockCRC, ch);
    }
    s.inUse[s.state_in_ch as usize] = 1;

    match s.state_in_len {
        1 => {
            push_block_byte(block, &mut s.nblock, ch);
        }
        2 => {
            push_block_byte(block, &mut s.nblock, ch);
            push_block_byte(block, &mut s.nblock, ch);
        }
        3 => {
            push_block_byte(block, &mut s.nblock, ch);
            push_block_byte(block, &mut s.nblock, ch);
            push_block_byte(block, &mut s.nblock, ch);
        }
        _ => {
            s.inUse[(s.state_in_len - 4) as usize] = 1;
            for _ in 0..4 {
                push_block_byte(block, &mut s.nblock, ch);
            }
            push_block_byte(block, &mut s.nblock, (s.state_in_len - 4) as UChar);
        }
    }
}

unsafe fn flush_RL(s: &mut EState) {
    if s.state_in_ch < 256 {
        add_pair_to_block(s);
    }
    init_RL(s);
}

unsafe fn add_char_to_block(s: &mut EState, ch: UInt32) {
    if ch != s.state_in_ch && s.state_in_len == 1 {
        let ch0 = s.state_in_ch as UChar;
        let block = block_storage_mut(s.block, s.blockSize100k);
        s.blockCRC = bz_crc_update(s.blockCRC, ch0);
        s.inUse[s.state_in_ch as usize] = 1;
        push_block_byte(block, &mut s.nblock, ch0);
        s.state_in_ch = ch;
        return;
    }

    if ch != s.state_in_ch || s.state_in_len == 255 {
        if s.state_in_ch < 256 {
            add_pair_to_block(s);
        }
        s.state_in_ch = ch;
        s.state_in_len = 1;
    } else {
        s.state_in_len += 1;
    }
}

unsafe fn validate_stream_buffers(strm: &bz_stream) -> bool {
    !(strm.avail_in > 0 && strm.next_in.is_null() || strm.avail_out > 0 && strm.next_out.is_null())
}

unsafe fn copy_input_until_stop(s: &mut EState) -> bool {
    let strm = &mut *s.strm;
    if strm.avail_in == 0
        || s.nblock >= s.nblockMAX
        || (s.mode != BZ_M_RUNNING && s.avail_in_expect == 0)
    {
        return false;
    }

    let input = slice::from_raw_parts(
        strm.next_in.cast::<UChar>(),
        usize::try_from(strm.avail_in).unwrap(),
    );
    let input_limit = if s.mode == BZ_M_RUNNING {
        input.len()
    } else {
        input.len().min(usize::try_from(s.avail_in_expect).unwrap())
    };
    let mut consumed = 0usize;

    while consumed < input_limit && s.nblock < s.nblockMAX {
        add_char_to_block(s, input[consumed] as UInt32);
        consumed += 1;
    }

    strm.next_in = input[consumed..].as_ptr().cast::<c_char>() as *mut c_char;
    strm.avail_in -= consumed as UInt32;
    add_total(
        &mut strm.total_in_lo32,
        &mut strm.total_in_hi32,
        consumed as UInt32,
    );
    if s.mode != BZ_M_RUNNING {
        s.avail_in_expect -= consumed as UInt32;
    }

    consumed != 0
}

unsafe fn copy_output_until_stop(s: &mut EState) -> bool {
    let strm = &mut *s.strm;
    if strm.avail_out == 0 || s.state_out_pos >= s.numZ {
        return false;
    }

    let zbits = zbits_storage(s.zbits, s.block, s.blockSize100k);
    let output = slice::from_raw_parts_mut(
        strm.next_out.cast::<UChar>(),
        usize::try_from(strm.avail_out).unwrap(),
    );
    let mut produced = 0usize;

    while produced < output.len() && s.state_out_pos < s.numZ {
        output[produced] = zbits[usize::try_from(s.state_out_pos).unwrap()];
        s.state_out_pos += 1;
        produced += 1;
    }

    strm.next_out = output[produced..].as_mut_ptr().cast::<c_char>();
    strm.avail_out -= produced as UInt32;
    add_total(
        &mut strm.total_out_lo32,
        &mut strm.total_out_hi32,
        produced as UInt32,
    );

    produced != 0
}

unsafe fn handle_compress(strm: *mut bz_stream) -> bool {
    let s = &mut *((*strm).state.cast::<EState>());
    let mut progress_in = false;
    let mut progress_out = false;

    loop {
        if s.state == BZ_S_OUTPUT {
            progress_out |= copy_output_until_stop(s);
            if s.state_out_pos < s.numZ {
                break;
            }
            if s.mode == BZ_M_FINISHING && s.avail_in_expect == 0 && isempty_RL(s) {
                break;
            }
            prepare_new_block(s);
            s.state = BZ_S_INPUT;
            if s.mode == BZ_M_FLUSHING && s.avail_in_expect == 0 && isempty_RL(s) {
                break;
            }
        }

        if s.state == BZ_S_INPUT {
            progress_in |= copy_input_until_stop(s);
            if s.mode != BZ_M_RUNNING && s.avail_in_expect == 0 {
                flush_RL(s);
                BZ2_compressBlock(s, (s.mode == BZ_M_FINISHING) as Bool);
                s.state = BZ_S_OUTPUT;
            } else if s.nblock >= s.nblockMAX {
                BZ2_compressBlock(s, 0);
                s.state = BZ_S_OUTPUT;
            } else if (*s.strm).avail_in == 0 {
                break;
            }
        }
    }

    progress_in || progress_out
}

unsafe fn makeMaps_e(s: &mut EState) {
    s.nInUse = 0;
    for i in 0..256usize {
        if s.inUse[i] != 0 {
            s.unseqToSeq[i] = s.nInUse as UChar;
            s.nInUse += 1;
        }
    }
}

unsafe fn emit_zero_run(
    s: &mut EState,
    mtfv: *mut UInt16,
    block_size_100k: Int32,
    wr: &mut Int32,
    z_pend: &mut Int32,
) {
    if *z_pend <= 0 {
        return;
    }

    *z_pend -= 1;
    loop {
        if (*z_pend & 1) != 0 {
            push_mtf_value(mtfv, block_size_100k, wr, BZ_RUNB as UInt16);
            s.mtfFreq[BZ_RUNB as usize] += 1;
        } else {
            push_mtf_value(mtfv, block_size_100k, wr, BZ_RUNA as UInt16);
            s.mtfFreq[BZ_RUNA as usize] += 1;
        }
        if *z_pend < 2 {
            break;
        }
        *z_pend = (*z_pend - 2) / 2;
    }
    *z_pend = 0;
}

unsafe fn generateMTFValues(s: &mut EState) {
    let mut yy = [0u8; 256];
    let block = block_storage(s.block, s.blockSize100k);

    makeMaps_e(s);
    let eob = s.nInUse + 1;
    for entry in s.mtfFreq.iter_mut().take((eob + 1) as usize) {
        *entry = 0;
    }

    let mut wr = 0;
    let mut z_pend = 0;
    for i in 0..usize::try_from(s.nInUse).unwrap() {
        yy[i] = i as UChar;
    }

    for i in 0..usize::try_from(s.nblock).unwrap() {
        let mut j = ptr_value(s.ptr, s.blockSize100k, i) as Int32 - 1;
        if j < 0 {
            j += s.nblock;
        }
        let ll_i = s.unseqToSeq[block[usize::try_from(j).unwrap()] as usize];

        if yy[0] == ll_i {
            z_pend += 1;
        } else {
            emit_zero_run(s, s.mtfv, s.blockSize100k, &mut wr, &mut z_pend);

            let mut rtmp = yy[1];
            yy[1] = yy[0];
            let mut pos = 1usize;
            while ll_i != rtmp {
                pos += 1;
                let rtmp2 = rtmp;
                rtmp = yy[pos];
                yy[pos] = rtmp2;
            }
            yy[0] = rtmp;
            push_mtf_value(
                s.mtfv,
                s.blockSize100k,
                &mut wr,
                (pos as Int32 + 1) as UInt16,
            );
            s.mtfFreq[pos + 1] += 1;
        }
    }

    emit_zero_run(s, s.mtfv, s.blockSize100k, &mut wr, &mut z_pend);
    push_mtf_value(s.mtfv, s.blockSize100k, &mut wr, eob as UInt16);
    s.mtfFreq[eob as usize] += 1;
    s.nMTF = wr;
}

unsafe fn bsW(s: &mut EState, n: Int32, value: UInt32) {
    while s.bsLive >= 8 {
        push_zbit_byte(s, (s.bsBuff >> 24) as UChar);
        s.bsBuff <<= 8;
        s.bsLive -= 8;
    }
    s.bsBuff |= value << (32 - s.bsLive - n);
    s.bsLive += n;
}

unsafe fn bsPutUInt32(s: &mut EState, value: UInt32) {
    bsW(s, 8, (value >> 24) & 0xff);
    bsW(s, 8, (value >> 16) & 0xff);
    bsW(s, 8, (value >> 8) & 0xff);
    bsW(s, 8, value & 0xff);
}

unsafe fn bsPutUChar(s: &mut EState, value: UChar) {
    bsW(s, 8, value as UInt32);
}

unsafe fn bsFinishWrite(s: &mut EState) {
    while s.bsLive > 0 {
        push_zbit_byte(s, (s.bsBuff >> 24) as UChar);
        s.bsBuff <<= 8;
        s.bsLive -= 8;
    }
}

unsafe fn sendMTFValues(s: &mut EState) {
    let mtfv = mtfv_storage(s.mtfv, s.blockSize100k);
    let alpha_size = s.nInUse + 2;
    for t in 0..BZ_N_GROUPS {
        for v in 0..usize::try_from(alpha_size).unwrap() {
            s.len[t][v] = BZ_GREATER_ICOST;
        }
    }

    assert_h(s.nMTF > 0, 3001);
    let n_groups = if s.nMTF < 200 {
        2
    } else if s.nMTF < 600 {
        3
    } else if s.nMTF < 1200 {
        4
    } else if s.nMTF < 2400 {
        5
    } else {
        6
    };

    let mut n_part = n_groups;
    let mut rem_f = s.nMTF;
    let mut gs = 0;
    while n_part > 0 {
        let t_freq = rem_f / n_part;
        let mut ge = gs - 1;
        let mut a_freq = 0;
        while a_freq < t_freq && ge < alpha_size - 1 {
            ge += 1;
            a_freq += s.mtfFreq[ge as usize];
        }
        if ge > gs && n_part != n_groups && n_part != 1 && ((n_groups - n_part) % 2 == 1) {
            a_freq -= s.mtfFreq[ge as usize];
            ge -= 1;
        }
        for v in 0..usize::try_from(alpha_size).unwrap() {
            s.len[(n_part - 1) as usize][v] = if (v as Int32) >= gs && (v as Int32) <= ge {
                BZ_LESSER_ICOST
            } else {
                BZ_GREATER_ICOST
            };
        }
        n_part -= 1;
        gs = ge + 1;
        rem_f -= a_freq;
    }

    let mut n_selectors = 0;
    for _iter in 0..BZ_N_ITERS {
        let mut fave = [0; BZ_N_GROUPS];
        for table in s.rfreq.iter_mut().take(n_groups as usize) {
            for freq in table.iter_mut().take(alpha_size as usize) {
                *freq = 0;
            }
        }

        if n_groups == 6 {
            for v in 0..alpha_size as usize {
                s.len_pack[v][0] = ((s.len[1][v] as UInt32) << 16) | s.len[0][v] as UInt32;
                s.len_pack[v][1] = ((s.len[3][v] as UInt32) << 16) | s.len[2][v] as UInt32;
                s.len_pack[v][2] = ((s.len[5][v] as UInt32) << 16) | s.len[4][v] as UInt32;
            }
        }

        n_selectors = 0;
        gs = 0;
        while gs < s.nMTF {
            let ge = (gs + BZ_G_SIZE - 1).min(s.nMTF - 1);
            let mut cost = [0u16; BZ_N_GROUPS];

            if n_groups == 6 && ge - gs + 1 == 50 {
                let mut cost01 = 0u32;
                let mut cost23 = 0u32;
                let mut cost45 = 0u32;
                for nn in 0..50usize {
                    let icv = mtfv[usize::try_from(gs).unwrap() + nn] as usize;
                    cost01 = cost01.wrapping_add(s.len_pack[icv][0]);
                    cost23 = cost23.wrapping_add(s.len_pack[icv][1]);
                    cost45 = cost45.wrapping_add(s.len_pack[icv][2]);
                }
                cost[0] = (cost01 & 0xffff) as u16;
                cost[1] = (cost01 >> 16) as u16;
                cost[2] = (cost23 & 0xffff) as u16;
                cost[3] = (cost23 >> 16) as u16;
                cost[4] = (cost45 & 0xffff) as u16;
                cost[5] = (cost45 >> 16) as u16;
            } else {
                for i in gs..=ge {
                    let icv = mtfv[usize::try_from(i).unwrap()] as usize;
                    for t in 0..n_groups as usize {
                        cost[t] = cost[t].wrapping_add(s.len[t][icv] as u16);
                    }
                }
            }

            let mut bc = i32::MAX;
            let mut bt = -1;
            for t in 0..n_groups {
                if (cost[t as usize] as Int32) < bc {
                    bc = cost[t as usize] as Int32;
                    bt = t;
                }
            }
            fave[bt as usize] += 1;
            s.selector[n_selectors as usize] = bt as UChar;
            n_selectors += 1;

            if n_groups == 6 && ge - gs + 1 == 50 {
                for nn in 0..50usize {
                    let sym = mtfv[usize::try_from(gs).unwrap() + nn] as usize;
                    s.rfreq[bt as usize][sym] += 1;
                }
            } else {
                for i in gs..=ge {
                    let sym = mtfv[usize::try_from(i).unwrap()] as usize;
                    s.rfreq[bt as usize][sym] += 1;
                }
            }
            gs = ge + 1;
        }

        for t in 0..n_groups as usize {
            BZ2_hbMakeCodeLengths(
                s.len[t].as_mut_ptr(),
                s.rfreq[t].as_mut_ptr(),
                alpha_size,
                MAX_HUFFMAN_LEN,
            );
        }
    }

    assert_h(n_groups < 8, 3002);
    assert_h(
        n_selectors < 32768 && n_selectors <= BZ_MAX_SELECTORS as Int32,
        3003,
    );

    let mut pos = [0u8; BZ_N_GROUPS];
    for i in 0..n_groups as usize {
        pos[i] = i as UChar;
    }
    for i in 0..n_selectors as usize {
        let ll_i = s.selector[i];
        let mut j = 0usize;
        let mut tmp = pos[j];
        while ll_i != tmp {
            j += 1;
            let tmp2 = tmp;
            tmp = pos[j];
            pos[j] = tmp2;
        }
        pos[0] = tmp;
        s.selectorMtf[i] = j as UChar;
    }

    for t in 0..n_groups as usize {
        let mut min_len = 32;
        let mut max_len = 0;
        for i in 0..alpha_size as usize {
            max_len = max_len.max(s.len[t][i] as Int32);
            min_len = min_len.min(s.len[t][i] as Int32);
        }
        assert_h(max_len <= MAX_HUFFMAN_LEN, 3004);
        assert_h(min_len >= 1, 3005);
        BZ2_hbAssignCodes(
            s.code[t].as_mut_ptr(),
            s.len[t].as_mut_ptr(),
            min_len,
            max_len,
            alpha_size,
        );
    }

    let mut in_use16 = [0u8; 16];
    for i in 0..16usize {
        let mut used = 0;
        for j in 0..16usize {
            if s.inUse[i * 16 + j] != 0 {
                used = 1;
            }
        }
        in_use16[i] = used;
    }
    for used in in_use16 {
        bsW(s, 1, used as UInt32);
    }
    for i in 0..16usize {
        if in_use16[i] != 0 {
            for j in 0..16usize {
                bsW(s, 1, s.inUse[i * 16 + j] as UInt32);
            }
        }
    }

    bsW(s, 3, n_groups as UInt32);
    bsW(s, 15, n_selectors as UInt32);
    for i in 0..n_selectors as usize {
        for _ in 0..s.selectorMtf[i] {
            bsW(s, 1, 1);
        }
        bsW(s, 1, 0);
    }

    for t in 0..n_groups as usize {
        let mut curr = s.len[t][0] as Int32;
        bsW(s, 5, curr as UInt32);
        for i in 0..alpha_size as usize {
            while curr < s.len[t][i] as Int32 {
                bsW(s, 2, 2);
                curr += 1;
            }
            while curr > s.len[t][i] as Int32 {
                bsW(s, 2, 3);
                curr -= 1;
            }
            bsW(s, 1, 0);
        }
    }

    let mut sel_ctr = 0;
    let mut gs = 0;
    while gs < s.nMTF {
        let ge = (gs + BZ_G_SIZE - 1).min(s.nMTF - 1);
        let table = s.selector[sel_ctr as usize] as usize;
        assert_h((table as Int32) < n_groups, 3006);
        if n_groups == 6 && ge - gs + 1 == 50 {
            for nn in 0..50usize {
                let mtfv_i = mtfv[usize::try_from(gs).unwrap() + nn] as usize;
                bsW(
                    s,
                    s.len[table][mtfv_i] as Int32,
                    s.code[table][mtfv_i] as UInt32,
                );
            }
        } else {
            for i in gs..=ge {
                let mtfv_i = mtfv[usize::try_from(i).unwrap()] as usize;
                bsW(
                    s,
                    s.len[table][mtfv_i] as Int32,
                    s.code[table][mtfv_i] as UInt32,
                );
            }
        }
        gs = ge + 1;
        sel_ctr += 1;
    }
    assert_h(sel_ctr == n_selectors, 3007);
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bsInitWrite(state: *mut EState) {
    if state.is_null() {
        return;
    }
    (*state).bsLive = 0;
    (*state).bsBuff = 0;
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_compressBlock(state: *mut EState, is_last_block: Bool) {
    if state.is_null() {
        return;
    }
    let s = &mut *state;

    if s.nblock > 0 {
        s.blockCRC = bz_crc_finalize(s.blockCRC);
        s.combinedCRC = s.combinedCRC.rotate_left(1) ^ s.blockCRC;
        if s.blockNo > 1 {
            s.numZ = 0;
        }
        if s.verbosity >= 2 {
            let _ = fprintf(
                stderr,
                BLOCK_VERBOSE_FORMAT.as_ptr().cast(),
                s.blockNo,
                s.blockCRC,
                s.combinedCRC,
                s.nblock,
            );
        }
        BZ2_blockSort(state);
    }

    let (_, zbits) = block_storage_mut(s.block, s.blockSize100k)
        .split_at_mut(usize::try_from(s.nblock).unwrap());
    s.zbits = zbits.as_mut_ptr();

    if s.blockNo == 1 {
        BZ2_bsInitWrite(state);
        bsPutUChar(s, BZ_HDR_B);
        bsPutUChar(s, BZ_HDR_Z);
        bsPutUChar(s, BZ_HDR_h);
        bsPutUChar(s, (BZ_HDR_0 + s.blockSize100k) as UChar);
    }

    if s.nblock > 0 {
        for byte in [0x31, 0x41, 0x59, 0x26, 0x53, 0x59] {
            bsPutUChar(s, byte);
        }
        bsPutUInt32(s, s.blockCRC);
        bsW(s, 1, 0);
        bsW(s, 24, s.origPtr as UInt32);
        generateMTFValues(s);
        sendMTFValues(s);
    }

    if is_last_block != 0 {
        for byte in [0x17, 0x72, 0x45, 0x38, 0x50, 0x90] {
            bsPutUChar(s, byte);
        }
        bsPutUInt32(s, s.combinedCRC);
        if s.verbosity >= 2 {
            let _ = fprintf(
                stderr,
                FINAL_CRC_VERBOSE_FORMAT.as_ptr().cast(),
                s.combinedCRC,
            );
        }
        bsFinishWrite(s);
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzCompressInit(
    strm: *mut bz_stream,
    blockSize100k: c_int,
    verbosity: c_int,
    workFactor: c_int,
) -> c_int {
    if !bz_config_ok() {
        return BZ_CONFIG_ERROR;
    }
    if strm.is_null() || !(1..=9).contains(&blockSize100k) || !(0..=250).contains(&workFactor) {
        return BZ_PARAM_ERROR;
    }

    ensure_default_allocators(strm, Some(default_bzalloc), Some(default_bzfree));
    reset_stream_totals(strm);

    let mut work_factor = workFactor;
    if work_factor == 0 {
        work_factor = 30;
    }

    let Some(block_cap) = block_capacity(blockSize100k) else {
        return BZ_CONFIG_ERROR;
    };

    let state = match alloc_zeroed_with_bzalloc::<EState>(strm) {
        Ok(ptr) => ptr,
        Err(code) => return code,
    };
    (*state).strm = strm;
    (*state).mode = BZ_M_RUNNING;
    (*state).state = BZ_S_INPUT;
    (*state).combinedCRC = 0;
    (*state).blockSize100k = blockSize100k;
    (*state).nblockMAX = block_cap as Int32 - 19;
    (*state).verbosity = verbosity;
    (*state).workFactor = work_factor;
    (*state).arr1 = ptr::null_mut();
    (*state).arr2 = ptr::null_mut();
    (*state).ftab = ptr::null_mut();

    let arr1 = match alloc_zeroed_slice_with_bzalloc::<UInt32>(strm, block_cap) {
        Ok(ptr) => ptr,
        Err(code) => {
            free_with_bzfree(strm, state);
            return code;
        }
    };
    let arr2 = match alloc_zeroed_slice_with_bzalloc::<UInt32>(
        strm,
        block_cap + BZ_N_OVERSHOOT as usize,
    ) {
        Ok(ptr) => ptr,
        Err(code) => {
            free_with_bzfree(strm, arr1);
            free_with_bzfree(strm, state);
            return code;
        }
    };
    let ftab = match alloc_zeroed_slice_with_bzalloc::<UInt32>(strm, 65_537) {
        Ok(ptr) => ptr,
        Err(code) => {
            free_with_bzfree(strm, arr1);
            free_with_bzfree(strm, arr2);
            free_with_bzfree(strm, state);
            return code;
        }
    };

    (*state).arr1 = arr1;
    (*state).arr2 = arr2;
    (*state).ftab = ftab;
    (*state).ptr = arr1;
    (*state).mtfv = arr1.cast::<UInt16>();
    (*state).block = arr2.cast::<UChar>();
    (*state).zbits = ptr::null_mut();

    init_RL(&mut *state);
    prepare_new_block(&mut *state);
    (*strm).state = state.cast();
    BZ_OK
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzCompress(strm: *mut bz_stream, action: c_int) -> c_int {
    if strm.is_null() || (*strm).state.is_null() || !validate_stream_buffers(&*strm) {
        return BZ_PARAM_ERROR;
    }

    let s = &mut *stream_state::<EState>(strm);
    if s.strm != strm {
        return BZ_PARAM_ERROR;
    }

    loop {
        match s.mode {
            BZ_M_IDLE => return BZ_SEQUENCE_ERROR,
            BZ_M_RUNNING => match action {
                BZ_RUN => {
                    return if handle_compress(strm) {
                        BZ_RUN_OK
                    } else {
                        BZ_PARAM_ERROR
                    }
                }
                BZ_FLUSH => {
                    s.avail_in_expect = (*strm).avail_in;
                    s.mode = BZ_M_FLUSHING;
                }
                BZ_FINISH => {
                    s.avail_in_expect = (*strm).avail_in;
                    s.mode = BZ_M_FINISHING;
                }
                _ => return BZ_PARAM_ERROR,
            },
            BZ_M_FLUSHING => {
                if action != BZ_FLUSH || s.avail_in_expect != (*strm).avail_in {
                    return BZ_SEQUENCE_ERROR;
                }
                let _progress = handle_compress(strm);
                if s.avail_in_expect > 0 || !isempty_RL(s) || s.state_out_pos < s.numZ {
                    return BZ_FLUSH_OK;
                }
                s.mode = BZ_M_RUNNING;
                return BZ_RUN_OK;
            }
            BZ_M_FINISHING => {
                if action != BZ_FINISH || s.avail_in_expect != (*strm).avail_in {
                    return BZ_SEQUENCE_ERROR;
                }
                let progress = handle_compress(strm);
                if !progress {
                    return BZ_SEQUENCE_ERROR;
                }
                if s.avail_in_expect > 0 || !isempty_RL(s) || s.state_out_pos < s.numZ {
                    return BZ_FINISH_OK;
                }
                s.mode = BZ_M_IDLE;
                return BZ_STREAM_END;
            }
            _ => return BZ_SEQUENCE_ERROR,
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_bzCompressEnd(strm: *mut bz_stream) -> c_int {
    if strm.is_null() || (*strm).state.is_null() {
        return BZ_PARAM_ERROR;
    }
    let state = &mut *stream_state::<EState>(strm);
    if state.strm != strm {
        return BZ_PARAM_ERROR;
    }
    release_storage(state);
    free_with_bzfree(strm, state);
    (*strm).state = ptr::null_mut();
    BZ_OK
}
