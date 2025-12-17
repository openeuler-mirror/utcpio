// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

#![allow(dead_code)]

use lazy_static::lazy_static;
use nix::libc::mode_t;
use std::sync::{Arc, Mutex};

pub type RettypeMajor = i32;
pub type RettypeMinor = u32; // Assuming RETTYPE_MINOR is unsigned int

pub const CPIO_TRAILER_NAME: &str = "TRAILER!!!";

#[derive(Clone)]
pub struct Deferment {
    pub header: CpioFileStat,
    pub next_index: Option<usize>, // 使用索引模拟指针
}
impl Deferment {
    pub fn new(file_hdr: &CpioFileStat) -> Self {
        Deferment {
            header: file_hdr.clone(),
            next_index: None,
        }
    }
}

pub struct DelayedSetStat {
    pub stat: CpioFileStat,
    pub invert_permissions: mode_t,
    pub next: Option<Arc<Mutex<DelayedSetStat>>>,
}
pub type DelayedSetStatPtr = Option<Arc<Mutex<DelayedSetStat>>>;

// 使用 lazy_static 和 Mutex 来实现静态可变变量
lazy_static! {
    pub static ref DELAYED_SET_STAT_HEAD: Mutex<DelayedSetStatPtr> = Mutex::new(None);
    pub static ref GLOBAL_DEFERMENTS: Mutex<Vec<Deferment>> = Mutex::new(Vec::new());
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct OldCpioHeader {
    pub c_magic: u16,
    pub c_dev: u16,
    pub c_ino: u16,
    pub c_mode: u16,
    pub c_uid: u16,
    pub c_gid: u16,
    pub c_nlink: u16,
    pub c_rdev: u16,
    pub c_mtimes: [u16; 2],
    pub c_namesize: u16,
    pub c_filesizes: [u16; 2],
}
impl OldCpioHeader {
    pub fn new() -> Self {
        OldCpioHeader {
            c_magic: 0,
            c_dev: 0,
            c_ino: 0,
            c_mode: 0,
            c_uid: 0,
            c_gid: 0,
            c_nlink: 0,
            c_rdev: 0,
            c_mtimes: [0; 2],
            c_namesize: 0,
            c_filesizes: [0; 2],
        }
    }
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(
                (self as *mut OldCpioHeader) as *mut u8,
                std::mem::size_of::<OldCpioHeader>(),
            )
        }
    }
    pub fn from_bytes(bytes: [u8; 6]) -> Self {
        OldCpioHeader {
            c_magic: u16::from_le_bytes([bytes[0], bytes[1]]), // 前 2 字节
            c_dev: u16::from_le_bytes([bytes[2], bytes[3]]),   // 中间 2 字节
            c_ino: u16::from_le_bytes([bytes[4], bytes[5]]),   // 后 2 字节
            // 其他字段设为默认值
            c_mode: 0,
            c_uid: 0,
            c_gid: 0,
            c_nlink: 0,
            c_rdev: 0,
            c_mtimes: [0, 0],
            c_namesize: 0,
            c_filesizes: [0, 0],
        }
    }
}

#[repr(C, packed)]
pub struct NewAsciiHeader {
    pub c_magic: [u8; 6],
    pub c_ino: [u8; 8],
    pub c_mode: [u8; 8],
    pub c_uid: [u8; 8],
    pub c_gid: [u8; 8],
    pub c_nlink: [u8; 8],
    pub c_mtime: [u8; 8],
    pub c_filesize: [u8; 8],
    pub c_dev_maj: [u8; 8],
    pub c_dev_min: [u8; 8],
    pub c_rdev_maj: [u8; 8],
    pub c_rdev_min: [u8; 8],
    pub c_namesize: [u8; 8],
    pub c_chksum: [u8; 8],
}
impl NewAsciiHeader {
    pub fn new() -> Self {
        NewAsciiHeader {
            c_magic: [0; 6],
            c_ino: [0; 8],
            c_mode: [0; 8],
            c_uid: [0; 8],
            c_gid: [0; 8],
            c_nlink: [0; 8],
            c_mtime: [0; 8],
            c_filesize: [0; 8],
            c_dev_maj: [0; 8],
            c_dev_min: [0; 8],
            c_rdev_maj: [0; 8],
            c_rdev_min: [0; 8],
            c_namesize: [0; 8],
            c_chksum: [0; 8],
        }
    }
}
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct OldAsciiHeader {
    pub c_magic: [u8; 6],
    pub c_dev: [u8; 6],
    pub c_ino: [u8; 6],
    pub c_mode: [u8; 6],
    pub c_uid: [u8; 6],
    pub c_gid: [u8; 6],
    pub c_nlink: [u8; 6],
    pub c_rdev: [u8; 6],
    pub c_mtime: [u8; 11],
    pub c_namesize: [u8; 6],
    pub c_filesize: [u8; 11],
}
impl OldAsciiHeader {
    pub fn new() -> Self {
        OldAsciiHeader {
            c_magic: [0; 6],
            c_dev: [0; 6],
            c_ino: [0; 6],
            c_mode: [0; 6],
            c_uid: [0; 6],
            c_gid: [0; 6],
            c_nlink: [0; 6],
            c_rdev: [0; 6],
            c_mtime: [0; 11],
            c_namesize: [0; 6],
            c_filesize: [0; 11],
        }
    }
}

#[derive(Clone)]
pub struct CpioFileStat {
    pub c_magic: u16,
    pub c_ino: u64,  // Assuming ino_t is u64
    pub c_mode: u32, // Assuming mode_t is u32
    pub c_uid: u32,  // Assuming uid_t is u32
    pub c_gid: u32,  // Assuming gid_t is u32
    pub c_nlink: usize,
    pub c_mtime: i64,    // Assuming time_t is i64
    pub c_filesize: i64, // Assuming off_t is i64
    pub c_dev_maj: RettypeMajor,
    pub c_dev_min: RettypeMinor,
    pub c_rdev_maj: RettypeMajor,
    pub c_rdev_min: RettypeMinor,
    pub c_namesize: usize,
    pub c_chksum: u32,
    pub c_name: Vec<u8>, //这个字段用于存储文件名，不需要外部直接访问
    pub c_name_buflen: usize,
    pub c_tar_linkname: Option<String>,
}

impl CpioFileStat {
    pub fn new() -> Self {
        Self {
            c_magic: 0,
            c_ino: 0,
            c_mode: 0,
            c_uid: 0,
            c_gid: 0,
            c_nlink: 0,
            c_mtime: 0,
            c_filesize: 0,
            c_dev_maj: 0,
            c_dev_min: 0,
            c_rdev_maj: 0,
            c_rdev_min: 0,
            c_namesize: 0,
            c_chksum: 0,
            c_name: Vec::new(),
            c_name_buflen: 0,
            c_tar_linkname: None,
        }
    }

    // pub fn free(&mut self) {
    //      self.c_name.clear();
    //      self.c_name_buflen = 0;
    //      *self = Self::new();
    // }

    // pub fn init(&mut self) {
    //     self.c_magic = 0o070707;
    //     self.c_ino = 0;
    //     self.c_mode = 0;
    // }

    // pub fn realloc_c_name(&mut self, len: usize) {
    //     if self.c_name_buflen < len {
    //         self.c_name.reserve(len);
    //         self.c_name_buflen = self.c_name.capacity();
    //     }
    // }

    pub fn set_c_name(&mut self, name: &str) {
        let trimmed_name = name.trim_end_matches('\0');
        self.c_name = trimmed_name.as_bytes().to_vec();
        self.c_name.push(b'\0'); // 添加结束符
        self.c_namesize = self.c_name.len();
        self.c_name_buflen = self.c_name.capacity()
    }

    pub fn get_c_name(&mut self) -> String {
        let len = self.c_namesize - 1;
        String::from_utf8_lossy(&self.c_name[..len]).into_owned()
    }
}
