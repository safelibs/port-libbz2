use std::os::raw::c_int;

pub const BZ_RUN: c_int = 0;
pub const BZ_FLUSH: c_int = 1;
pub const BZ_FINISH: c_int = 2;

pub const BZ_OK: c_int = 0;
pub const BZ_RUN_OK: c_int = 1;
pub const BZ_FLUSH_OK: c_int = 2;
pub const BZ_FINISH_OK: c_int = 3;
pub const BZ_STREAM_END: c_int = 4;
pub const BZ_SEQUENCE_ERROR: c_int = -1;
pub const BZ_PARAM_ERROR: c_int = -2;
pub const BZ_MEM_ERROR: c_int = -3;
pub const BZ_DATA_ERROR: c_int = -4;
pub const BZ_DATA_ERROR_MAGIC: c_int = -5;
pub const BZ_IO_ERROR: c_int = -6;
pub const BZ_UNEXPECTED_EOF: c_int = -7;
pub const BZ_OUTBUFF_FULL: c_int = -8;
pub const BZ_CONFIG_ERROR: c_int = -9;

pub const BZ_MAX_UNUSED: c_int = 5000;

// BZ2_bzlibVersion must keep returning the upstream version string.
pub const BZ_VERSION_BYTES: &[u8] = b"1.0.8, 13-Jul-2019\0";

pub const BZ_MAX_ALPHA_SIZE: usize = 258;
pub const BZ_MAX_CODE_LEN: c_int = 23;
pub const BZ_RUNA: c_int = 0;
pub const BZ_RUNB: c_int = 1;
pub const BZ_N_GROUPS: usize = 6;
pub const BZ_G_SIZE: c_int = 50;
pub const BZ_N_ITERS: c_int = 4;
pub const BZ_MAX_SELECTORS: usize = 2 + (900000 / 50);

pub const BZ_M_IDLE: c_int = 1;
pub const BZ_M_RUNNING: c_int = 2;
pub const BZ_M_FLUSHING: c_int = 3;
pub const BZ_M_FINISHING: c_int = 4;

pub const BZ_S_OUTPUT: c_int = 1;
pub const BZ_S_INPUT: c_int = 2;

pub const BZ_N_RADIX: c_int = 2;
pub const BZ_N_QSORT: c_int = 12;
pub const BZ_N_SHELL: c_int = 18;
pub const BZ_N_OVERSHOOT: c_int = BZ_N_RADIX + BZ_N_QSORT + BZ_N_SHELL + 2;

pub const BZ_X_IDLE: c_int = 1;
pub const BZ_X_OUTPUT: c_int = 2;
pub const BZ_X_MAGIC_1: c_int = 10;
pub const BZ_X_MAGIC_2: c_int = 11;
pub const BZ_X_MAGIC_3: c_int = 12;
pub const BZ_X_MAGIC_4: c_int = 13;
pub const BZ_X_BLKHDR_1: c_int = 14;
pub const BZ_X_BLKHDR_2: c_int = 15;
pub const BZ_X_BLKHDR_3: c_int = 16;
pub const BZ_X_BLKHDR_4: c_int = 17;
pub const BZ_X_BLKHDR_5: c_int = 18;
pub const BZ_X_BLKHDR_6: c_int = 19;
pub const BZ_X_BCRC_1: c_int = 20;
pub const BZ_X_BCRC_2: c_int = 21;
pub const BZ_X_BCRC_3: c_int = 22;
pub const BZ_X_BCRC_4: c_int = 23;
pub const BZ_X_RANDBIT: c_int = 24;
pub const BZ_X_ORIGPTR_1: c_int = 25;
pub const BZ_X_ORIGPTR_2: c_int = 26;
pub const BZ_X_ORIGPTR_3: c_int = 27;
pub const BZ_X_MAPPING_1: c_int = 28;
pub const BZ_X_MAPPING_2: c_int = 29;
pub const BZ_X_SELECTOR_1: c_int = 30;
pub const BZ_X_SELECTOR_2: c_int = 31;
pub const BZ_X_SELECTOR_3: c_int = 32;
pub const BZ_X_CODING_1: c_int = 33;
pub const BZ_X_CODING_2: c_int = 34;
pub const BZ_X_CODING_3: c_int = 35;
pub const BZ_X_MTF_1: c_int = 36;
pub const BZ_X_MTF_2: c_int = 37;
pub const BZ_X_MTF_3: c_int = 38;
pub const BZ_X_MTF_4: c_int = 39;
pub const BZ_X_MTF_5: c_int = 40;
pub const BZ_X_MTF_6: c_int = 41;
pub const BZ_X_ENDHDR_2: c_int = 42;
pub const BZ_X_ENDHDR_3: c_int = 43;
pub const BZ_X_ENDHDR_4: c_int = 44;
pub const BZ_X_ENDHDR_5: c_int = 45;
pub const BZ_X_ENDHDR_6: c_int = 46;
pub const BZ_X_CCRC_1: c_int = 47;
pub const BZ_X_CCRC_2: c_int = 48;
pub const BZ_X_CCRC_3: c_int = 49;
pub const BZ_X_CCRC_4: c_int = 50;

pub const MTFA_SIZE: usize = 4096;
pub const MTFL_SIZE: usize = 16;

pub const BZFILE_MODE_READ: c_int = 1;
pub const BZFILE_MODE_WRITE: c_int = 2;
