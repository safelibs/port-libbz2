use crate::constants::{BZ_MAX_ALPHA_SIZE, BZ_MAX_SELECTORS, BZ_N_GROUPS, MTFA_SIZE, MTFL_SIZE};
use core::ffi::c_void;
use std::os::raw::{c_char, c_int, c_short, c_uchar, c_uint, c_ushort};

pub type Char = c_char;
pub type Bool = c_uchar;
pub type UChar = c_uchar;
pub type Int32 = c_int;
pub type UInt32 = c_uint;
pub type Int16 = c_short;
pub type UInt16 = c_ushort;
pub type CFile = c_void;

pub type bz_alloc_func = Option<unsafe extern "C" fn(*mut c_void, c_int, c_int) -> *mut c_void>;
pub type bz_free_func = Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>;

#[repr(C)]
pub struct bz_stream {
    pub next_in: *mut c_char,
    pub avail_in: c_uint,
    pub total_in_lo32: c_uint,
    pub total_in_hi32: c_uint,
    pub next_out: *mut c_char,
    pub avail_out: c_uint,
    pub total_out_lo32: c_uint,
    pub total_out_hi32: c_uint,
    pub state: *mut c_void,
    pub bzalloc: bz_alloc_func,
    pub bzfree: bz_free_func,
    pub opaque: *mut c_void,
}

#[repr(C)]
pub struct EState {
    pub strm: *mut bz_stream,
    pub mode: Int32,
    pub state: Int32,
    pub avail_in_expect: UInt32,
    pub arr1: *mut UInt32,
    pub arr2: *mut UInt32,
    pub ftab: *mut UInt32,
    pub origPtr: Int32,
    // These pointers are fixed aliases layered over `arr1` and `arr2`.
    pub ptr: *mut UInt32,
    pub block: *mut UChar,
    pub mtfv: *mut UInt16,
    pub zbits: *mut UChar,
    pub workFactor: Int32,
    pub state_in_ch: UInt32,
    pub state_in_len: Int32,
    pub rNToGo: Int32,
    pub rTPos: Int32,
    pub nblock: Int32,
    pub nblockMAX: Int32,
    pub numZ: Int32,
    pub state_out_pos: Int32,
    pub nInUse: Int32,
    pub inUse: [Bool; 256],
    pub unseqToSeq: [UChar; 256],
    pub bsBuff: UInt32,
    pub bsLive: Int32,
    pub blockCRC: UInt32,
    pub combinedCRC: UInt32,
    pub verbosity: Int32,
    pub blockNo: Int32,
    pub blockSize100k: Int32,
    pub nMTF: Int32,
    pub mtfFreq: [Int32; BZ_MAX_ALPHA_SIZE],
    pub selector: [UChar; BZ_MAX_SELECTORS],
    pub selectorMtf: [UChar; BZ_MAX_SELECTORS],
    pub len: [[UChar; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub code: [[Int32; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub rfreq: [[Int32; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub len_pack: [[UInt32; 4]; BZ_MAX_ALPHA_SIZE],
}

#[repr(C)]
pub struct DState {
    pub strm: *mut bz_stream,
    pub state: Int32,
    pub state_out_ch: UChar,
    pub state_out_len: Int32,
    pub blockRandomised: Bool,
    pub rNToGo: Int32,
    pub rTPos: Int32,
    pub bsBuff: UInt32,
    pub bsLive: Int32,
    pub blockSize100k: Int32,
    // Later phases fill both the fast (`tt`) and small (`ll16`/`ll4`) decode paths.
    pub smallDecompress: Bool,
    pub currBlockNo: Int32,
    pub verbosity: Int32,
    pub origPtr: Int32,
    pub tPos: UInt32,
    pub k0: Int32,
    pub unzftab: [Int32; 256],
    pub nblock_used: Int32,
    pub cftab: [Int32; 257],
    pub cftabCopy: [Int32; 257],
    pub tt: *mut UInt32,
    pub ll16: *mut UInt16,
    pub ll4: *mut UChar,
    pub storedBlockCRC: UInt32,
    pub storedCombinedCRC: UInt32,
    pub calculatedBlockCRC: UInt32,
    pub calculatedCombinedCRC: UInt32,
    pub nInUse: Int32,
    pub inUse: [Bool; 256],
    pub inUse16: [Bool; 16],
    pub seqToUnseq: [UChar; 256],
    pub mtfa: [UChar; MTFA_SIZE],
    pub mtfbase: [Int32; 256 / MTFL_SIZE],
    pub selector: [UChar; BZ_MAX_SELECTORS],
    pub selectorMtf: [UChar; BZ_MAX_SELECTORS],
    pub len: [[UChar; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub limit: [[Int32; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub base: [[Int32; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub perm: [[Int32; BZ_MAX_ALPHA_SIZE]; BZ_N_GROUPS],
    pub minLens: [Int32; BZ_N_GROUPS],
    pub save_i: Int32,
    pub save_j: Int32,
    pub save_t: Int32,
    pub save_alphaSize: Int32,
    pub save_nGroups: Int32,
    pub save_nSelectors: Int32,
    pub save_EOB: Int32,
    pub save_groupNo: Int32,
    pub save_groupPos: Int32,
    pub save_nextSym: Int32,
    pub save_nblockMAX: Int32,
    pub save_nblock: Int32,
    pub save_es: Int32,
    pub save_N: Int32,
    pub save_curr: Int32,
    pub save_zt: Int32,
    pub save_zn: Int32,
    pub save_zvec: Int32,
    pub save_zj: Int32,
    pub save_gSel: Int32,
    pub save_gMinlen: Int32,
    pub save_gLimit: *mut Int32,
    pub save_gBase: *mut Int32,
    pub save_gPerm: *mut Int32,
}

#[repr(C)]
pub struct BzFileState {
    pub mode: c_int,
    pub last_err: c_int,
    pub file: *mut CFile,
    pub small: c_int,
    pub verbosity: c_int,
    pub block_size_100k: c_int,
    pub work_factor: c_int,
}
