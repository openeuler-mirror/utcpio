/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

type ModeT = u32;

// 文件权限位常量，使用 u32 类型以匹配常见的 POSIX mode_t
pub const S_ISUID: u32 = 0o4000; // Set-user-ID bit
pub const S_ISGID: u32 = 0o2000; // Set-group-ID bit
pub const S_ISVTX: u32 = 0o1000; // Sticky bit

pub const S_IRUSR: u32 = 0o400; // Read permission, owner
pub const S_IWUSR: u32 = 0o200; // Write permission, owner
pub const S_IXUSR: u32 = 0o100; // Execute permission, owner

pub const S_IRGRP: u32 = 0o040; // Read permission, group
pub const S_IWGRP: u32 = 0o020; // Write permission, group
pub const S_IXGRP: u32 = 0o010; // Execute permission, group

pub const S_IROTH: u32 = 0o004; // Read permission, others
pub const S_IWOTH: u32 = 0o002; // Write permission, others
pub const S_IXOTH: u32 = 0o001; // Execute permission, others

// 组合权限
// pub const MODE_WXUSR: u32 = S_IWUSR | S_IXUSR; // Owner write + execute
pub const MODE_R: u32 = S_IRUSR | S_IRGRP | S_IROTH; // Read for all
pub const MODE_RW: u32 = S_IWUSR | S_IWGRP | S_IWOTH | MODE_R; // Read + write for all
pub const MODE_RWX: u32 = S_IXUSR | S_IXGRP | S_IXOTH | MODE_RW; // Read + write + execute for all
pub const MODE_ALL: u32 = S_ISUID | S_ISGID | S_ISVTX | MODE_RWX; // All bits

// 假设的常量定义，需要根据实际情况替换
const S_IFMT: ModeT = 0o170000;
const S_IFBLK: ModeT = 0o060000;
const S_IFCHR: ModeT = 0o020000;
const S_IFDIR: ModeT = 0o040000;
const S_IFREG: ModeT = 0o100000;
const S_IFIFO: ModeT = 0o010000;
const S_IFLNK: ModeT = 0o120000;
const S_IFSOCK: ModeT = 0o140000;
//const S_IFNWK: mode_t = 0o110000;

// POSIX 宏定义
pub fn s_isblk(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFBLK
}

pub fn s_ischr(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFCHR
}

pub fn s_isdir(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFDIR
}

pub fn s_isreg(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFREG
}

pub fn s_isfifo(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFIFO
}

pub fn s_islnk(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFLNK
}

pub fn s_issock(m: ModeT) -> bool {
    (m & S_IFMT) == S_IFSOCK
}

// pub fn s_isnwk(m: mode_t) -> bool {
//     (m & S_IFMT) == S_IFNWK
// }

// cpio 文件类型位定义
pub const CP_IFMT: ModeT = 0o170000;
pub const CP_IFBLK: ModeT = 0o060000;
pub const CP_IFCHR: ModeT = 0o020000;
pub const CP_IFDIR: ModeT = 0o040000;
pub const CP_IFREG: ModeT = 0o100000;
pub const CP_IFIFO: ModeT = 0o010000;
pub const CP_IFLNK: ModeT = 0o120000;
pub const CP_IFSOCK: ModeT = 0o140000;
//pub const CP_IFNWK: mode_t = 0o110000;

// lstat 别名定义
#[cfg(not(target_os = "linux"))] // 示例：仅在非 Linux 系统上定义别名
pub use std::fs::metadata as lstat;

#[cfg(target_os = "linux")] // 示例：仅在 Linux 系统上定义别名
pub use std::fs::metadata as lstat;
