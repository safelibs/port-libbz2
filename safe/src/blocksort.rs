use crate::constants::{BZ_N_OVERSHOOT, BZ_N_QSORT, BZ_N_RADIX};
use crate::types::{Bool, EState, Int32, UChar, UInt16, UInt32};
use std::slice;

unsafe extern "C" {
    fn BZ2_bz__AssertH__fail(errcode: Int32);
}

const True: Bool = 1;
const False: Bool = 0;

const FALLBACK_QSORT_SMALL_THRESH: Int32 = 10;
const FALLBACK_QSORT_STACK_SIZE: usize = 100;
const MAIN_QSORT_SMALL_THRESH: Int32 = 20;
const MAIN_QSORT_DEPTH_THRESH: Int32 = BZ_N_RADIX + BZ_N_QSORT;
const MAIN_QSORT_STACK_SIZE: usize = 100;
const INCS: [Int32; 14] = [
    1, 4, 13, 40, 121, 364, 1093, 3280, 9841, 29524, 88573, 265720, 797161, 2391484,
];
const SETMASK: UInt32 = 1 << 21;
const CLEARMASK: UInt32 = !SETMASK;

#[inline]
unsafe fn assert_h(cond: bool, errcode: Int32) {
    if !cond {
        BZ2_bz__AssertH__fail(errcode);
    }
}

#[inline]
fn as_bool(value: bool) -> Bool {
    if value {
        True
    } else {
        False
    }
}

#[inline]
fn fmin(a: Int32, b: Int32) -> Int32 {
    if a < b {
        a
    } else {
        b
    }
}

#[inline]
fn mmed3(mut a: UChar, mut b: UChar, c: UChar) -> UChar {
    if a > b {
        (a, b) = (b, a);
    }
    if b > c {
        b = c;
        if a > b {
            b = a;
        }
    }
    b
}

#[inline]
unsafe fn block_capacity(state: &EState) -> usize {
    usize::try_from(state.blockSize100k).unwrap_or_default() * 100_000
}

#[inline]
fn block_byte_capacity(block_cap: usize) -> usize {
    (block_cap + BZ_N_OVERSHOOT as usize) * core::mem::size_of::<UInt32>()
}

struct MainSortStorage<'a> {
    storage: &'a mut [UChar],
    quadrant_offset: usize,
}

impl<'a> MainSortStorage<'a> {
    #[inline]
    fn new(storage: &'a mut [UChar], nblock: Int32) -> Self {
        let mut quadrant_offset = usize::try_from(nblock + BZ_N_OVERSHOOT).unwrap();
        if (quadrant_offset & 1) != 0 {
            quadrant_offset += 1;
        }
        Self {
            storage,
            quadrant_offset,
        }
    }

    #[inline]
    fn block_get(&self, idx: usize) -> UChar {
        self.storage[idx]
    }

    #[inline]
    fn block_set(&mut self, idx: usize, value: UChar) {
        self.storage[idx] = value;
    }

    #[inline]
    fn quadrant_get(&self, idx: usize) -> UInt16 {
        let base = self.quadrant_offset + idx * core::mem::size_of::<UInt16>();
        UInt16::from_ne_bytes([self.storage[base], self.storage[base + 1]])
    }

    #[inline]
    fn quadrant_set(&mut self, idx: usize, value: UInt16) {
        let base = self.quadrant_offset + idx * core::mem::size_of::<UInt16>();
        let [lo, hi] = value.to_ne_bytes();
        self.storage[base] = lo;
        self.storage[base + 1] = hi;
    }
}

#[inline]
unsafe fn bh_set(bhtab: &mut [UInt32], idx: Int32) {
    let word = usize::try_from(idx >> 5).unwrap();
    let bit = (idx & 31) as u32;
    bhtab[word] |= 1u32 << bit;
}

#[inline]
unsafe fn bh_clear(bhtab: &mut [UInt32], idx: Int32) {
    let word = usize::try_from(idx >> 5).unwrap();
    let bit = (idx & 31) as u32;
    bhtab[word] &= !(1u32 << bit);
}

#[inline]
unsafe fn bh_is_set(bhtab: &[UInt32], idx: Int32) -> bool {
    let word = usize::try_from(idx >> 5).unwrap();
    let bit = (idx & 31) as u32;
    (bhtab[word] & (1u32 << bit)) != 0
}

#[inline]
unsafe fn bh_word(bhtab: &[UInt32], idx: Int32) -> UInt32 {
    bhtab[usize::try_from(idx >> 5).unwrap()]
}

#[inline]
fn unaligned_bh(idx: Int32) -> Int32 {
    idx & 0x01f
}

unsafe fn fallbackSimpleSort(fmap: &mut [UInt32], eclass: &[UInt32], lo: Int32, hi: Int32) {
    if lo == hi {
        return;
    }

    if hi - lo > 3 {
        let mut i = hi - 4;
        while i >= lo {
            let tmp = fmap[i as usize];
            let ec_tmp = eclass[tmp as usize];
            let mut j = i + 4;
            while j <= hi && ec_tmp > eclass[fmap[j as usize] as usize] {
                fmap[(j - 4) as usize] = fmap[j as usize];
                j += 4;
            }
            fmap[(j - 4) as usize] = tmp;
            i -= 1;
        }
    }

    let mut i = hi - 1;
    while i >= lo {
        let tmp = fmap[i as usize];
        let ec_tmp = eclass[tmp as usize];
        let mut j = i + 1;
        while j <= hi && ec_tmp > eclass[fmap[j as usize] as usize] {
            fmap[(j - 1) as usize] = fmap[j as usize];
            j += 1;
        }
        fmap[(j - 1) as usize] = tmp;
        i -= 1;
    }
}

unsafe fn fallbackQSort3(fmap: &mut [UInt32], eclass: &[UInt32], lo_st: Int32, hi_st: Int32) {
    let mut stack_lo = [0; FALLBACK_QSORT_STACK_SIZE];
    let mut stack_hi = [0; FALLBACK_QSORT_STACK_SIZE];
    let mut sp = 0usize;
    let mut r: UInt32 = 0;

    stack_lo[sp] = lo_st;
    stack_hi[sp] = hi_st;
    sp += 1;

    while sp > 0 {
        assert_h(sp < FALLBACK_QSORT_STACK_SIZE - 1, 1004);
        sp -= 1;
        let lo = stack_lo[sp];
        let hi = stack_hi[sp];

        if hi - lo < FALLBACK_QSORT_SMALL_THRESH {
            fallbackSimpleSort(fmap, eclass, lo, hi);
            continue;
        }

        r = (r.wrapping_mul(7621)).wrapping_add(1) % 32768;
        let med = match r % 3 {
            0 => eclass[fmap[lo as usize] as usize],
            1 => eclass[fmap[((lo + hi) >> 1) as usize] as usize],
            _ => eclass[fmap[hi as usize] as usize],
        };

        let mut un_lo = lo;
        let mut lt_lo = lo;
        let mut un_hi = hi;
        let mut gt_hi = hi;

        loop {
            loop {
                if un_lo > un_hi {
                    break;
                }
                let n = eclass[fmap[un_lo as usize] as usize] as Int32 - med as Int32;
                if n == 0 {
                    fmap.swap(un_lo as usize, lt_lo as usize);
                    lt_lo += 1;
                    un_lo += 1;
                    continue;
                }
                if n > 0 {
                    break;
                }
                un_lo += 1;
            }

            loop {
                if un_lo > un_hi {
                    break;
                }
                let n = eclass[fmap[un_hi as usize] as usize] as Int32 - med as Int32;
                if n == 0 {
                    fmap.swap(un_hi as usize, gt_hi as usize);
                    gt_hi -= 1;
                    un_hi -= 1;
                    continue;
                }
                if n < 0 {
                    break;
                }
                un_hi -= 1;
            }

            if un_lo > un_hi {
                break;
            }
            fmap.swap(un_lo as usize, un_hi as usize);
            un_lo += 1;
            un_hi -= 1;
        }

        if gt_hi < lt_lo {
            continue;
        }

        let n = fmin(lt_lo - lo, un_lo - lt_lo);
        for offset in 0..n {
            fmap.swap((lo + offset) as usize, (un_lo - n + offset) as usize);
        }
        let m = fmin(hi - gt_hi, gt_hi - un_hi);
        for offset in 0..m {
            fmap.swap((un_lo + offset) as usize, (hi - m + 1 + offset) as usize);
        }

        let n = lo + un_lo - lt_lo - 1;
        let m = hi - (gt_hi - un_hi) + 1;

        if n - lo > hi - m {
            stack_lo[sp] = lo;
            stack_hi[sp] = n;
            sp += 1;
            stack_lo[sp] = m;
            stack_hi[sp] = hi;
            sp += 1;
        } else {
            stack_lo[sp] = m;
            stack_hi[sp] = hi;
            sp += 1;
            stack_lo[sp] = lo;
            stack_hi[sp] = n;
            sp += 1;
        }
    }
}

unsafe fn fallbackSort(
    fmap: &mut [UInt32],
    eclass_ptr: *mut UInt32,
    bhtab: &mut [UInt32],
    nblock: Int32,
    _verb: Int32,
) {
    let nblock_usize = usize::try_from(nblock).unwrap();
    let mut ftab: [Int32; 257] = [0; 257];
    let mut ftab_copy: [Int32; 256] = [0; 256];

    for i in 0..257usize {
        ftab[i] = 0;
    }
    {
        let eclass8 = slice::from_raw_parts(eclass_ptr.cast::<UChar>(), nblock_usize);
        for &class in eclass8 {
            ftab[class as usize] += 1;
        }

        ftab_copy.copy_from_slice(&ftab[..256]);
        for i in 1..257usize {
            ftab[i] += ftab[i - 1];
        }

        for (i, &class) in eclass8.iter().enumerate() {
            let j = class as usize;
            let k = ftab[j].wrapping_sub(1);
            ftab[j] = k;
            fmap[k as usize] = i as UInt32;
        }
    }

    let n_bhtab = usize::try_from(2 + (nblock / 32)).unwrap();
    for entry in bhtab.iter_mut().take(n_bhtab) {
        *entry = 0;
    }
    for i in 0..256i32 {
        bh_set(bhtab, ftab[i as usize]);
    }

    for i in 0..32i32 {
        bh_set(bhtab, nblock + 2 * i);
        bh_clear(bhtab, nblock + 2 * i + 1);
    }

    let mut h = 1;
    loop {
        let mut n_not_done = 0;
        {
            let eclass = slice::from_raw_parts_mut(eclass_ptr, nblock_usize);
            let mut j = 0;
            for i in 0..nblock {
                if bh_is_set(bhtab, i) {
                    j = i;
                }
                let mut k = fmap[i as usize] as Int32 - h;
                if k < 0 {
                    k += nblock;
                }
                eclass[k as usize] = j as UInt32;
            }

            let mut r = -1;
            loop {
                let mut k = r + 1;
                while bh_is_set(bhtab, k) && unaligned_bh(k) != 0 {
                    k += 1;
                }
                if bh_is_set(bhtab, k) {
                    while bh_word(bhtab, k) == 0xffff_ffff {
                        k += 32;
                    }
                    while bh_is_set(bhtab, k) {
                        k += 1;
                    }
                }
                let l = k - 1;
                if l >= nblock {
                    break;
                }
                while !bh_is_set(bhtab, k) && unaligned_bh(k) != 0 {
                    k += 1;
                }
                if !bh_is_set(bhtab, k) {
                    while bh_word(bhtab, k) == 0 {
                        k += 32;
                    }
                    while !bh_is_set(bhtab, k) {
                        k += 1;
                    }
                }
                r = k - 1;
                if r >= nblock {
                    break;
                }

                if r > l {
                    n_not_done += r - l + 1;
                    fallbackQSort3(fmap, eclass, l, r);
                    let mut cc = -1;
                    for i in l..=r {
                        let cc1 = eclass[fmap[i as usize] as usize] as Int32;
                        if cc != cc1 {
                            bh_set(bhtab, i);
                            cc = cc1;
                        }
                    }
                }
            }
        }

        h *= 2;
        if h > nblock || n_not_done == 0 {
            break;
        }
    }

    let mut j = 0usize;
    {
        let eclass8 = slice::from_raw_parts_mut(eclass_ptr.cast::<UChar>(), nblock_usize);
        for i in 0..nblock_usize {
            while ftab_copy[j] == 0 {
                j += 1;
            }
            ftab_copy[j] -= 1;
            eclass8[fmap[i] as usize] = j as UChar;
        }
    }
    assert_h(j < 256, 1005);
}

unsafe fn mainGtU(
    mut i1: UInt32,
    mut i2: UInt32,
    storage: &MainSortStorage<'_>,
    nblock: UInt32,
    budget: &mut Int32,
) -> Bool {
    macro_rules! cmp_byte {
        () => {{
            let c1 = storage.block_get(i1 as usize);
            let c2 = storage.block_get(i2 as usize);
            if c1 != c2 {
                return as_bool(c1 > c2);
            }
            i1 += 1;
            i2 += 1;
        }};
    }

    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();
    cmp_byte!();

    let mut k = nblock as Int32 + 8;
    loop {
        for _ in 0..8 {
            let c1 = storage.block_get(i1 as usize);
            let c2 = storage.block_get(i2 as usize);
            if c1 != c2 {
                return as_bool(c1 > c2);
            }
            let s1 = storage.quadrant_get(i1 as usize);
            let s2 = storage.quadrant_get(i2 as usize);
            if s1 != s2 {
                return as_bool(s1 > s2);
            }
            i1 += 1;
            i2 += 1;
        }

        if i1 >= nblock {
            i1 -= nblock;
        }
        if i2 >= nblock {
            i2 -= nblock;
        }

        k -= 8;
        *budget -= 1;
        if k < 0 {
            break;
        }
    }

    False
}

unsafe fn mainSimpleSort(
    ptr: &mut [UInt32],
    storage: &MainSortStorage<'_>,
    nblock: Int32,
    lo: Int32,
    hi: Int32,
    d: Int32,
    budget: &mut Int32,
) {
    let big_n = hi - lo + 1;
    if big_n < 2 {
        return;
    }

    let mut hp = 0usize;
    while hp < INCS.len() && INCS[hp] < big_n {
        hp += 1;
    }
    hp -= 1;

    loop {
        let h = INCS[hp];
        let mut i = lo + h;
        'copies: loop {
            for _ in 0..3 {
                if i > hi {
                    break 'copies;
                }
                let v = ptr[i as usize];
                let mut j = i;
                while mainGtU(
                    ptr[(j - h) as usize] + d as UInt32,
                    v + d as UInt32,
                    storage,
                    nblock as UInt32,
                    budget,
                ) != 0
                {
                    ptr[j as usize] = ptr[(j - h) as usize];
                    j -= h;
                    if j <= lo + h - 1 {
                        break;
                    }
                }
                ptr[j as usize] = v;
                i += 1;
            }
            if *budget < 0 {
                return;
            }
        }

        if hp == 0 {
            break;
        }
        hp -= 1;
    }
}

unsafe fn mainQSort3(
    ptr: &mut [UInt32],
    storage: &MainSortStorage<'_>,
    nblock: Int32,
    lo_st: Int32,
    hi_st: Int32,
    d_st: Int32,
    budget: &mut Int32,
) {
    let mut stack_lo = [0; MAIN_QSORT_STACK_SIZE];
    let mut stack_hi = [0; MAIN_QSORT_STACK_SIZE];
    let mut stack_d = [0; MAIN_QSORT_STACK_SIZE];
    let mut sp = 0usize;

    stack_lo[sp] = lo_st;
    stack_hi[sp] = hi_st;
    stack_d[sp] = d_st;
    sp += 1;

    while sp > 0 {
        assert_h(sp < MAIN_QSORT_STACK_SIZE - 2, 1001);
        sp -= 1;
        let lo = stack_lo[sp];
        let hi = stack_hi[sp];
        let d = stack_d[sp];

        if hi - lo < MAIN_QSORT_SMALL_THRESH || d > MAIN_QSORT_DEPTH_THRESH {
            mainSimpleSort(ptr, storage, nblock, lo, hi, d, budget);
            if *budget < 0 {
                return;
            }
            continue;
        }

        let med = mmed3(
            storage.block_get((ptr[lo as usize] + d as UInt32) as usize),
            storage.block_get((ptr[hi as usize] + d as UInt32) as usize),
            storage.block_get((ptr[((lo + hi) >> 1) as usize] + d as UInt32) as usize),
        ) as Int32;

        let mut un_lo = lo;
        let mut lt_lo = lo;
        let mut un_hi = hi;
        let mut gt_hi = hi;

        loop {
            loop {
                if un_lo > un_hi {
                    break;
                }
                let n =
                    storage.block_get((ptr[un_lo as usize] + d as UInt32) as usize) as Int32 - med;
                if n == 0 {
                    ptr.swap(un_lo as usize, lt_lo as usize);
                    lt_lo += 1;
                    un_lo += 1;
                    continue;
                }
                if n > 0 {
                    break;
                }
                un_lo += 1;
            }
            loop {
                if un_lo > un_hi {
                    break;
                }
                let n =
                    storage.block_get((ptr[un_hi as usize] + d as UInt32) as usize) as Int32 - med;
                if n == 0 {
                    ptr.swap(un_hi as usize, gt_hi as usize);
                    gt_hi -= 1;
                    un_hi -= 1;
                    continue;
                }
                if n < 0 {
                    break;
                }
                un_hi -= 1;
            }
            if un_lo > un_hi {
                break;
            }
            ptr.swap(un_lo as usize, un_hi as usize);
            un_lo += 1;
            un_hi -= 1;
        }

        if gt_hi < lt_lo {
            stack_lo[sp] = lo;
            stack_hi[sp] = hi;
            stack_d[sp] = d + 1;
            sp += 1;
            continue;
        }

        let n = fmin(lt_lo - lo, un_lo - lt_lo);
        for offset in 0..n {
            ptr.swap((lo + offset) as usize, (un_lo - n + offset) as usize);
        }
        let m = fmin(hi - gt_hi, gt_hi - un_hi);
        for offset in 0..m {
            ptr.swap((un_lo + offset) as usize, (hi - m + 1 + offset) as usize);
        }

        let mut next_lo = [0; 3];
        let mut next_hi = [0; 3];
        let mut next_d = [0; 3];

        let n = lo + un_lo - lt_lo - 1;
        let m = hi - (gt_hi - un_hi) + 1;

        next_lo[0] = lo;
        next_hi[0] = n;
        next_d[0] = d;
        next_lo[1] = m;
        next_hi[1] = hi;
        next_d[1] = d;
        next_lo[2] = n + 1;
        next_hi[2] = m - 1;
        next_d[2] = d + 1;

        if next_hi[0] - next_lo[0] < next_hi[1] - next_lo[1] {
            next_lo.swap(0, 1);
            next_hi.swap(0, 1);
            next_d.swap(0, 1);
        }
        if next_hi[1] - next_lo[1] < next_hi[2] - next_lo[2] {
            next_lo.swap(1, 2);
            next_hi.swap(1, 2);
            next_d.swap(1, 2);
        }
        if next_hi[0] - next_lo[0] < next_hi[1] - next_lo[1] {
            next_lo.swap(0, 1);
            next_hi.swap(0, 1);
            next_d.swap(0, 1);
        }

        stack_lo[sp] = next_lo[0];
        stack_hi[sp] = next_hi[0];
        stack_d[sp] = next_d[0];
        sp += 1;
        stack_lo[sp] = next_lo[1];
        stack_hi[sp] = next_hi[1];
        stack_d[sp] = next_d[1];
        sp += 1;
        stack_lo[sp] = next_lo[2];
        stack_hi[sp] = next_hi[2];
        stack_d[sp] = next_d[2];
        sp += 1;
    }
}

unsafe fn mainSort(
    ptr: &mut [UInt32],
    storage: &mut MainSortStorage<'_>,
    ftab: &mut [UInt32],
    nblock: Int32,
    verb: Int32,
    budget: &mut Int32,
) {
    let _ = verb;
    let mut running_order = [0; 256];
    let mut big_done = [False; 256];
    let mut copy_start = [0; 256];
    let mut copy_end = [0; 256];
    let mut num_q_sorted = 0;

    for i in (0..=65_536usize).rev() {
        ftab[i] = 0;
    }

    let mut j = (storage.block_get(0) as UInt16) << 8;
    let mut i = nblock - 1;
    while i >= 3 {
        storage.quadrant_set(i as usize, 0);
        j = (j >> 8) | ((storage.block_get(i as usize) as UInt16) << 8);
        ftab[j as usize] += 1;

        storage.quadrant_set((i - 1) as usize, 0);
        j = (j >> 8) | ((storage.block_get((i - 1) as usize) as UInt16) << 8);
        ftab[j as usize] += 1;

        storage.quadrant_set((i - 2) as usize, 0);
        j = (j >> 8) | ((storage.block_get((i - 2) as usize) as UInt16) << 8);
        ftab[j as usize] += 1;

        storage.quadrant_set((i - 3) as usize, 0);
        j = (j >> 8) | ((storage.block_get((i - 3) as usize) as UInt16) << 8);
        ftab[j as usize] += 1;

        i -= 4;
    }
    while i >= 0 {
        storage.quadrant_set(i as usize, 0);
        j = (j >> 8) | ((storage.block_get(i as usize) as UInt16) << 8);
        ftab[j as usize] += 1;
        i -= 1;
    }

    let nblock_usize = usize::try_from(nblock).unwrap();
    for i in 0..BZ_N_OVERSHOOT as usize {
        let byte = storage.block_get(i);
        storage.block_set(nblock_usize + i, byte);
        storage.quadrant_set(nblock_usize + i, 0);
    }

    for i in 1..=65_536usize {
        ftab[i] += ftab[i - 1];
    }

    let mut s = (storage.block_get(0) as UInt16) << 8;
    let mut i = nblock - 1;
    while i >= 3 {
        s = (s >> 8) | ((storage.block_get(i as usize) as UInt16) << 8);
        j = ftab[s as usize].wrapping_sub(1) as UInt16;
        ftab[s as usize] = j as UInt32;
        ptr[j as usize] = i as UInt32;

        s = (s >> 8) | ((storage.block_get((i - 1) as usize) as UInt16) << 8);
        j = ftab[s as usize].wrapping_sub(1) as UInt16;
        ftab[s as usize] = j as UInt32;
        ptr[j as usize] = (i - 1) as UInt32;

        s = (s >> 8) | ((storage.block_get((i - 2) as usize) as UInt16) << 8);
        j = ftab[s as usize].wrapping_sub(1) as UInt16;
        ftab[s as usize] = j as UInt32;
        ptr[j as usize] = (i - 2) as UInt32;

        s = (s >> 8) | ((storage.block_get((i - 3) as usize) as UInt16) << 8);
        j = ftab[s as usize].wrapping_sub(1) as UInt16;
        ftab[s as usize] = j as UInt32;
        ptr[j as usize] = (i - 3) as UInt32;

        i -= 4;
    }
    while i >= 0 {
        s = (s >> 8) | ((storage.block_get(i as usize) as UInt16) << 8);
        j = ftab[s as usize].wrapping_sub(1) as UInt16;
        ftab[s as usize] = j as UInt32;
        ptr[j as usize] = i as UInt32;
        i -= 1;
    }

    for i in 0..=255usize {
        big_done[i] = False;
        running_order[i] = i as Int32;
    }

    let mut h = 1;
    while h <= 256 {
        h = 3 * h + 1;
    }
    loop {
        h /= 3;
        for i in h..=255 {
            let vv = running_order[i as usize];
            let mut j = i;
            while ftab[((running_order[(j - h) as usize] + 1) << 8) as usize]
                .wrapping_sub(ftab[(running_order[(j - h) as usize] << 8) as usize])
                > ftab[((vv + 1) << 8) as usize].wrapping_sub(ftab[(vv << 8) as usize])
            {
                running_order[j as usize] = running_order[(j - h) as usize];
                j -= h;
                if j <= h - 1 {
                    break;
                }
            }
            running_order[j as usize] = vv;
        }
        if h == 1 {
            break;
        }
    }

    for i in 0..=255i32 {
        let ss = running_order[i as usize];

        for j in 0..=255i32 {
            if j == ss {
                continue;
            }
            let sb = (ss << 8) + j;
            if (ftab[sb as usize] & SETMASK) == 0 {
                let lo = (ftab[sb as usize] & CLEARMASK) as Int32;
                let hi = ((ftab[(sb + 1) as usize] & CLEARMASK) as Int32) - 1;
                if hi > lo {
                    mainQSort3(ptr, storage, nblock, lo, hi, BZ_N_RADIX, budget);
                    num_q_sorted += hi - lo + 1;
                    if *budget < 0 {
                        return;
                    }
                }
            }
            ftab[sb as usize] |= SETMASK;
        }

        assert_h(big_done[ss as usize] == 0, 1006);

        for j in 0..=255usize {
            copy_start[j] = (ftab[(j << 8) + ss as usize] & CLEARMASK) as Int32;
            copy_end[j] = ((ftab[(j << 8) + ss as usize + 1] & CLEARMASK) as Int32) - 1;
        }

        let mut j = (ftab[(ss << 8) as usize] & CLEARMASK) as Int32;
        while j < copy_start[ss as usize] {
            let mut k = ptr[j as usize] as Int32 - 1;
            if k < 0 {
                k += nblock;
            }
            let c1 = storage.block_get(k as usize);
            if big_done[c1 as usize] == 0 {
                ptr[copy_start[c1 as usize] as usize] = k as UInt32;
                copy_start[c1 as usize] += 1;
            }
            j += 1;
        }

        let mut j = ((ftab[((ss + 1) << 8) as usize] & CLEARMASK) as Int32) - 1;
        while j > copy_end[ss as usize] {
            let mut k = ptr[j as usize] as Int32 - 1;
            if k < 0 {
                k += nblock;
            }
            let c1 = storage.block_get(k as usize);
            if big_done[c1 as usize] == 0 {
                ptr[copy_end[c1 as usize] as usize] = k as UInt32;
                copy_end[c1 as usize] -= 1;
            }
            j -= 1;
        }

        assert_h(
            (copy_start[ss as usize] - 1 == copy_end[ss as usize])
                || (copy_start[ss as usize] == 0 && copy_end[ss as usize] == nblock - 1),
            1007,
        );

        for j in 0..=255usize {
            ftab[(j << 8) + ss as usize] |= SETMASK;
        }

        big_done[ss as usize] = True;

        if i < 255 {
            let bb_start = (ftab[(ss << 8) as usize] & CLEARMASK) as Int32;
            let bb_size = ((ftab[((ss + 1) << 8) as usize] & CLEARMASK) as Int32) - bb_start;
            let mut shifts = 0;
            while (bb_size >> shifts) > 65_534 {
                shifts += 1;
            }
            let mut j = bb_size - 1;
            while j >= 0 {
                let a2update = ptr[(bb_start + j) as usize] as Int32;
                let q_val = (j >> shifts) as UInt16;
                storage.quadrant_set(a2update as usize, q_val);
                if a2update < BZ_N_OVERSHOOT {
                    storage.quadrant_set((a2update + nblock) as usize, q_val);
                }
                j -= 1;
            }
            assert_h(((bb_size - 1) >> shifts) <= 65_535, 1002);
        }
    }

    let _ = num_q_sorted;
}

#[no_mangle]
pub unsafe extern "C" fn BZ2_blockSort(s: *mut EState) {
    if s.is_null() {
        return;
    }

    let s = &mut *s;
    let nblock = s.nblock;
    let verb = s.verbosity;
    let mut wfact = s.workFactor;
    let block_cap = block_capacity(s);
    let arr2_ptr = s.arr2;
    let ptr = slice::from_raw_parts_mut(s.arr1, block_cap);
    let ftab = slice::from_raw_parts_mut(s.ftab, 65_537);

    if nblock < 10_000 {
        fallbackSort(ptr, arr2_ptr, ftab, nblock, verb);
    } else {
        let storage =
            slice::from_raw_parts_mut(arr2_ptr.cast::<UChar>(), block_byte_capacity(block_cap));
        let mut storage = MainSortStorage::new(storage, nblock);

        if wfact < 1 {
            wfact = 1;
        }
        if wfact > 100 {
            wfact = 100;
        }
        let budget_init = nblock * ((wfact - 1) / 3);
        let mut budget = budget_init;
        mainSort(ptr, &mut storage, ftab, nblock, verb, &mut budget);
        if budget < 0 {
            fallbackSort(ptr, arr2_ptr, ftab, nblock, verb);
        }
    }

    s.origPtr = -1;
    for i in 0..usize::try_from(s.nblock).unwrap() {
        if ptr[i] == 0 {
            s.origPtr = i as Int32;
            break;
        }
    }
    assert_h(s.origPtr != -1, 1003);
}
