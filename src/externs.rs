/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */
use lazy_static::lazy_static;
use std::sync::Mutex;

pub const LG_8: u32 = 3;
pub const LG_16: u32 = 4;

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ArchiveFormat {
    Unknown,
    Binary,
    Oldascii,
    Newascii,
    Crcascii,
    Tar,
    Ustar,
    Hpoldascii,
    Hpbinary,
}

lazy_static! {
    //pub static ref SVR4_COMPAT: Mutex<bool> = Mutex::new(false);
    //pub static ref DEBUG_FLAG: Mutex<bool> = Mutex::new(false);
    pub static ref NEWDIR_UMASK: Mutex<u32> = Mutex::new(0);
    pub static ref LAST_HEADER_START: Mutex<i32> = Mutex::new(0);
    pub static ref  SWAPPING_HALFWORDS: Mutex<bool> = Mutex::new(false);
    pub static ref  SWAPPING_BYTES: Mutex<bool> = Mutex::new(false);
}

pub fn set_swapping_halfwords(value: bool) {
    *SWAPPING_HALFWORDS.lock().unwrap() = value;
}
pub fn get_swapping_halfwords() -> bool {
    *SWAPPING_HALFWORDS.lock().unwrap()
}

pub fn set_swapping_bytes(swapping: bool) {
    *SWAPPING_BYTES.lock().unwrap() = swapping;
}
pub fn get_swapping_bytes() -> bool {
    *SWAPPING_BYTES.lock().unwrap()
}
pub fn set_last_header_start(start: i32) {
    *LAST_HEADER_START.lock().unwrap() = start;
}
pub fn get_last_header_start() -> i32 {
    *LAST_HEADER_START.lock().unwrap()
}

pub fn set_newdir_umask(umask: u32) {
    *NEWDIR_UMASK.lock().unwrap() = umask;
}
pub fn get_newdir_umask() -> u32 {
    *NEWDIR_UMASK.lock().unwrap()
}

// pub fn set_debug_flag(flag: bool) {
//     *DEBUG_FLAG.lock().unwrap() = flag;
// }
// pub fn get_debug_flag() -> bool {
//     *DEBUG_FLAG.lock().unwrap()
// }

// pub fn get_svr4_compat() -> bool {
//     *SVR4_COMPAT.lock().unwrap()
// }
// pub fn set_svr4_compat(svr4: bool) {
//     *SVR4_COMPAT.lock().unwrap() = svr4;
// }

// 假设这些全局变量和函数已经定义
pub const DISK_IO_BLOCK_SIZE: usize = 512;

pub type CopyFunctionFn = fn() -> Result<(), std::io::Error>;
// pub static COPY_FUNCTION: OnceLock<CopyFunctionFn> = OnceLock::new();

//pub type Xstat = fn() -> i32;
//pub static XSTAT: OnceLock<Xstat> = OnceLock::new();

pub const TTY_NAME: &str = "/dev/tty";
/* Values for warn_option */
// pub const  CPIO_WARN_NONE: usize = 0;
pub const CPIO_WARN_TRUNCATE: usize = 1;
pub const CPIO_WARN_INTERDIR: usize = 2;
pub const CPIO_WARN_ALL: usize = usize::MAX;
