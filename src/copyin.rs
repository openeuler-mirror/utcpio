// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

#![allow(
    dead_code,
    clippy::needless_late_init,
    clippy::manual_memcpy,
    clippy::large_enum_variant,
    clippy::unnecessary_mut_passed,
    clippy::if_same_then_else,
    unused_mut
)]

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::str;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use chrono::{TimeZone, Utc};
use lazy_static::lazy_static;

use libc::{dev_t, fnmatch, lchown, symlink, timespec, umask, unlink};
use nix::sys::stat::fstat;
use nix::unistd::{Gid, Uid};

use pax::paxerror::*;
use pax::paxlib::PAXEXIT_FAILURE;
use pax::rmt::*;

use crate::appargs::*;
use crate::copypass::*;
use crate::cpiohdr::*;
use crate::dstring::*;
use crate::externs::*;
use crate::filemode::*;
use crate::filetype::*;
use crate::filetype::{CP_IFBLK, CP_IFCHR, CP_IFIFO, CP_IFMT, CP_IFSOCK};
use crate::global::*;
use crate::idcache::*;
use crate::tar::*;
use crate::util::*;

use gnu::error::*;
use gnu::gettime::*;
use gnu::quotearg::*;

static mut CURRENT_TIME: timespec = timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DelayedLinkKey {
    pub dev: u64, // dev_t
    pub ino: u64, // ino_t
}

#[derive(Debug, Clone)]
struct DelayedLinkValue {
    pub mode: u32,  // mode_t
    pub uid: u32,   // uid_t
    pub gid: u32,   // gid_t
    pub mtime: i64, // time_t, representing seconds since epoch
    pub source: String,
    pub target: String,
}

struct DelayedLink {
    table: HashMap<DelayedLinkKey, DelayedLinkValue>,
}

impl DelayedLink {
    fn new() -> Self {
        DelayedLink {
            table: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
    // fn dl_hash(&self, entry: &DelayedLinkKey, table_size: usize) -> usize {
    //     let n = entry.dev;
    //     let nshift = (mem::size_of::<u64>() - mem::size_of::<u64>()) * 8; // CHAR_BIT is 8
    //     let shifted_n = if nshift > 0 { n << nshift } else { n };
    //     (shifted_n ^ entry.ino) as usize % table_size
    // }

    // fn dl_compare(&self, a: &DelayedLinkKey, b: &DelayedLinkKey) -> bool {
    //     a.dev == b.dev && a.ino == b.ino
    // }
    // fn get_first(&self) -> Option<(&DelayedLinkKey, &DelayedLinkValue)> {
    //     self.table.iter().next()
    // }

    // fn get_next<'a>(
    //     &'a self,
    //     current_key: &'a DelayedLinkKey,
    // ) -> Option<(&'a DelayedLinkKey, &'a DelayedLinkValue)> {
    //     let mut iter = self.table.iter();
    //     while let Some((key, _)) = iter.next() {
    //         if key == current_key {
    //             return iter.next();
    //         }
    //     }
    //     None
    // }

    fn insert(&mut self, key: DelayedLinkKey, value: DelayedLinkValue) {
        self.table.insert(key, value);
    }

    // fn get(&self, key: &DelayedLinkKey) -> Option<&DelayedLinkValue> {
    //     self.table.get(key)
    // }

    // fn remove(&mut self, key: &DelayedLinkKey) -> Option<DelayedLinkValue> {
    //     self.table.remove(key)
    // }
}

// fn test_read() {
//     let mut stdin = io::stdin();
//     let mut buffer = [0u8; 8192]; // 8KB 缓冲区

//     loop {
//         match stdin.read(&mut buffer[..512]) {
//             Ok(0) => {
//                 // EOF，读取完成
//                 break;
//             }
//             Ok(read_bytes) => {
//                 // 处理读取的数据
//                 println!("Read {} bytes", read_bytes);
//                 // 在这里添加你的数据处理逻辑
//                 // 例如，你可以将数据写入另一个文件，或者进行其他操作
//                 //println!("Read data: {:?}", &buffer[..read_bytes]);
//             }
//             Err(e) => {
//                 // 读取错误
//                 eprintln!("Error reading from stdin: {}", e);
//                 break;
//             }
//         }
//     }
// }

lazy_static! {
    static ref NEW_NAME: Mutex<Option<DynamicString>> = Mutex::new(None);
    static ref INITIALIZED_NEW_NAME: Mutex<bool> = Mutex::new(false);
    static ref COPYIN_DEFERMENTS: Mutex<Vec<Deferment>> = Mutex::new(Vec::new());
    static ref GLOBAL_DELAYED_LINK: Mutex<DelayedLink> = Mutex::new(DelayedLink::new());
}

//只在当前文件使用

pub fn warn_junk_bytes(bytes_skipped: u64) {
    // 只有当跳过的字节数超过一定阈值时才显示警告
    const WARN_THRESHOLD: u64 = 100; // 跳过超过100字节才显示警告

    if bytes_skipped > WARN_THRESHOLD {
        error(
            0,
            0,
            format_args!("warning: skipped {} bytes of junk", bytes_skipped),
        );
    }
}

pub fn query_rename(
    file_hdr: &mut CpioFileStat,
    tty_in: &mut File,
    tty_out: &mut File,
    rename_in: &mut File,
) -> i32 {
    let mut new_name_guard = NEW_NAME.lock().unwrap();
    let mut initialized_guard = INITIALIZED_NEW_NAME.lock().unwrap();

    if !*initialized_guard {
        *new_name_guard = Some(DYNAMIC_STRING_INITIALIZER);
        *initialized_guard = true;
    }

    let c_name = file_hdr.get_c_name();

    let str_res = if get_rename_flag() {
        write!(tty_out, "rename {} -> ", c_name).unwrap();
        tty_out.flush().unwrap();
        ds_fgets(tty_in, new_name_guard.as_mut().unwrap())
    } else {
        ds_fgetstr(rename_in, new_name_guard.as_mut().unwrap(), b'\n')
    };

    if str_res.is_none() || str_res.as_ref().map_or(true, |s| s.is_empty()) {
        -1
    } else {
        let name = new_name_guard
            .as_mut()
            .map(|s| String::from_utf8(s.ds_string.clone()).unwrap())
            .unwrap();
        cpio_set_c_name(file_hdr, name.as_str());
        0
    }
}

pub fn tape_skip_padding(
    input_tape: &mut MutexGuard<TapeInput>,
    in_file_des: &mut File,
    offset: u64,
) {
    let pad: u64 = match get_archive_format() {
        ArchiveFormat::Crcascii | ArchiveFormat::Newascii => (4 - (offset % 4)) % 4,
        ArchiveFormat::Binary | ArchiveFormat::Hpbinary => (2 - (offset % 2)) % 2,
        ArchiveFormat::Tar | ArchiveFormat::Ustar => (512 - (offset % 512)) % 512,
        _ => 0,
    };

    if pad != 0 {
        tape_toss_input(input_tape, in_file_des, pad as i32);
    }
}

pub fn get_link_name(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
) -> Option<String> {
    // 放宽文件名长度验证条件，使用更合理的上限
    const MAX_LINK_NAME_SIZE: i64 = 1024 * 1024; // 1MB 作为合理的上限

    if file_hdr.c_filesize < 0 || file_hdr.c_filesize > MAX_LINK_NAME_SIZE {
        error(
            0,
            0,
            format_args!(
                "{}: stored filename length is out of range",
                file_hdr.get_c_name()
            ),
        );
        None
    } else {
        let size = file_hdr.c_filesize as usize;
        let mut link_name = vec![0; size];
        tape_buffered_read(input_tape, &mut link_name, in_file_des, size);

        // 跳过padding
        tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);

        // 直接使用读取的字节转换为字符串，移除末尾的null字符
        let clean_name = link_name
            .iter()
            .take_while(|&&x| x != 0)
            .cloned()
            .collect::<Vec<u8>>();

        String::from_utf8(clean_name).ok()
    }
}

fn list_file(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
) {
    if get_verbose_flag() {
        if (file_hdr.c_mode & CP_IFMT) == CP_IFLNK {
            if get_archive_format() != ArchiveFormat::Tar
                && get_archive_format() != ArchiveFormat::Ustar
            {
                let link_name = get_link_name(input_tape, file_hdr, in_file_des);
                if link_name.is_some() {
                    long_format(file_hdr, link_name);
                }
            } else {
                long_format(file_hdr, file_hdr.c_tar_linkname.clone());
            }
        } else {
            long_format(file_hdr, None);
        }
    } else {
        println!("{}", file_hdr.get_c_name());
    }
    let crc = 0;
    tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
    tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
    if get_only_verify_crc_flag() {
        if (file_hdr.c_mode & CP_IFMT) == CP_IFLNK {
            return;
        }
        if crc != file_hdr.c_chksum {
            error(
                0,
                0,
                format_args!(
                    "{}: checksum error (0x{:x}, should be 0x{:x})",
                    file_hdr.get_c_name(),
                    crc,
                    file_hdr.c_chksum
                ),
            );
        }
    }
}

fn try_existing_file(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
    existing_dir: &mut bool,
) -> i32 {
    *existing_dir = false;

    let c_name = file_hdr.get_c_name();
    if let Ok(metadata) = fs::metadata(&c_name) {
        if metadata.is_dir() && (file_hdr.c_mode & CP_IFMT) == CP_IFDIR {
            *existing_dir = true;
            return 0;
        } else if !get_unconditional_flag() && file_hdr.c_mtime <= metadata.mtime() {
            error(
                0,
                0,
                format_args!("{} not created: newer or same age version exists", c_name),
            );
            tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
            tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
            return -1;
        } else {
            let res: io::Result<()>;
            if metadata.is_dir() {
                // 对于目录，先尝试删除，如果失败则跳过而不是报错
                res = fs::remove_dir(&c_name);
                if res.is_err() {
                    // 目录删除失败，可能是权限问题或目录非空，跳过处理
                    return 0;
                }
            } else {
                res = fs::remove_file(&c_name);
            }
            if res.is_err() {
                error(0, 0, format_args!("cannot remove {}", c_name));
                tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
                tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
            }
        }
    }
    0
}

fn defer_copyin(file_hdr: &CpioFileStat) {
    let deferment = Deferment::new(file_hdr);
    let mut copyin_deferments = COPYIN_DEFERMENTS.lock().unwrap();

    copyin_deferments.insert(0, deferment);
}

pub fn create_defered_links(file_hdr: &mut CpioFileStat) {
    let mut deferments_guard = COPYIN_DEFERMENTS.lock().unwrap();
    let mut i = 0;
    let mut prev_i: Option<usize> = None;

    while i < deferments_guard.len() {
        let mut deferment = deferments_guard[i].clone();

        if deferment.header.c_ino == file_hdr.c_ino
            && deferment.header.c_dev_maj == file_hdr.c_dev_maj
            && deferment.header.c_dev_min == file_hdr.c_dev_min
        {
            let link_res = link_to_name(&deferment.header.get_c_name(), &file_hdr.get_c_name());
            if link_res < 0 {
                error(
                    0,
                    0,
                    format_args!(
                        "cannot link {} to {}",
                        deferment.header.get_c_name(),
                        file_hdr.get_c_name()
                    ),
                );
            }

            if let Some(prev) = prev_i {
                deferments_guard[prev].next_index = deferment.next_index;
            } else if let Some(next_index) = deferment.next_index {
                deferments_guard.remove(i);
                i = next_index;
                continue;
            } else {
                deferments_guard.remove(i);
                continue;
            }
            deferments_guard.remove(i);
            continue;
        } else {
            prev_i = Some(i);
            i = deferment.next_index.unwrap_or(i + 1);
        }
    }
}

pub fn create_defered_links_to_skipped(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
) -> i32 {
    if file_hdr.c_filesize == 0 {
        return -1;
    }

    let mut deferments_guard = COPYIN_DEFERMENTS.lock().unwrap();
    let mut i = 0;
    let mut prev_i: Option<usize> = None;

    while i < deferments_guard.len() {
        let mut deferment = deferments_guard[i].clone();

        if deferment.header.c_ino == file_hdr.c_ino
            && deferment.header.c_dev_maj == file_hdr.c_dev_maj
            && deferment.header.c_dev_min == file_hdr.c_dev_min
        {
            if let Some(prev) = prev_i {
                deferments_guard[prev].next_index = deferment.next_index;
            } else if let Some(next_index) = deferment.next_index {
                deferments_guard.remove(i);
                i = next_index;
                continue;
            } else {
                deferments_guard.remove(i);
                continue;
            }

            cpio_set_c_name(file_hdr, deferment.header.get_c_name().as_str());
            deferments_guard.remove(i);
            // Convert RawFd to File
            copyin_regular_file(output_tape, input_tape, file_hdr, in_file_des);

            return 0;
        } else {
            prev_i = Some(i);
            i = deferment.next_index.unwrap_or(i + 1);
        }
    }
    -1
}

pub fn create_final_defers() {
    let mut deferments_guard = COPYIN_DEFERMENTS.lock().unwrap();

    for d in deferments_guard.iter_mut() {
        let c_name = d.header.get_c_name();
        let link_res = link_to_maj_min_ino(
            &c_name,
            d.header.c_dev_maj as u32,
            d.header.c_dev_min,
            d.header.c_ino,
        );
        if link_res == 0 {
            continue;
        }

        let out_file_des = OpenOptions::new()
            .create(true)
            .write(true)
            .custom_flags(libc::O_CREAT | libc::O_WRONLY)
            .mode(0o600)
            .open(&c_name);

        let out_file_des = match out_file_des {
            Ok(file) => file,
            Err(_e) => {
                if get_create_dir_flag() {
                    create_all_directories(&c_name);
                    match OpenOptions::new()
                        .create(true)
                        .write(true)
                        .custom_flags(libc::O_CREAT | libc::O_WRONLY)
                        .mode(0o600)
                        .open(&c_name)
                    {
                        Ok(file) => file,
                        Err(_e) => {
                            open_error(&c_name);
                            continue;
                        }
                    }
                } else {
                    open_error(&c_name);
                    continue;
                }
            }
        };
        set_perms(Some(&out_file_des), &mut d.header);
    }
}

pub fn copyin_regular_file(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
) {
    let to_stdout_option = get_to_stdout_option();
    let archive_format = get_archive_format();
    let create_dir_flag = get_create_dir_flag();
    // let swap_halfwords_flag = get_swap_halfwords_flag();
    // let swap_bytes_flag = get_swap_bytes_flag();

    let link_res: i32;

    let mut out_file_des = if to_stdout_option {
        unsafe { File::from_raw_fd(libc::STDOUT_FILENO) }
    } else {
        if file_hdr.c_nlink > 1
            && (archive_format == ArchiveFormat::Newascii
                || archive_format == ArchiveFormat::Crcascii)
        {
            if file_hdr.c_filesize == 0 {
                defer_copyin(file_hdr);
                tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
                tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
                return;
            }
            link_res = link_to_maj_min_ino(
                &file_hdr.get_c_name(),
                file_hdr.c_dev_maj as u32,
                file_hdr.c_dev_min,
                file_hdr.c_ino,
            );

            if link_res == 0 {
                tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
                tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
                return;
            }
        } else if file_hdr.c_nlink > 1
            && archive_format != ArchiveFormat::Tar
            && archive_format != ArchiveFormat::Ustar
        {
            link_res = link_to_maj_min_ino(
                &file_hdr.get_c_name(),
                file_hdr.c_dev_maj as u32,
                file_hdr.c_dev_min,
                file_hdr.c_ino,
            );
            if link_res == 0 {
                tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
                tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
                return;
            }
        } else if (archive_format == ArchiveFormat::Tar || archive_format == ArchiveFormat::Ustar)
            && file_hdr.c_tar_linkname.is_some()
            && !file_hdr.c_tar_linkname.as_ref().unwrap().is_empty()
        {
            link_res = link_to_name(
                file_hdr.get_c_name().as_str(),
                file_hdr.c_tar_linkname.as_ref().unwrap(),
            );
            if link_res < 0 {
                let tar_linkname = file_hdr.c_tar_linkname.clone().unwrap_or_default();
                error(
                    0,
                    0,
                    format_args!("cannot link {} to {}", tar_linkname, file_hdr.get_c_name()),
                );
            }
            return;
        }

        let out_file_res = OpenOptions::new()
            .create(true)
            .write(true)
            .custom_flags(libc::O_CREAT | libc::O_WRONLY)
            .mode(0o600)
            .open(file_hdr.get_c_name());

        let out_file = match out_file_res {
            Ok(file) => file,
            Err(_e) => {
                if create_dir_flag {
                    create_all_directories(&file_hdr.get_c_name());
                    match OpenOptions::new()
                        .create(true)
                        .write(true)
                        .custom_flags(libc::O_CREAT | libc::O_WRONLY)
                        .mode(0o600)
                        .open(file_hdr.get_c_name())
                    {
                        Ok(file) => file,
                        Err(_) => {
                            open_error(&file_hdr.get_c_name());
                            tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
                            tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
                            return;
                        }
                    }
                } else {
                    open_error(&file_hdr.get_c_name());
                    tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
                    tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
                    return;
                }
            }
        };

        out_file
    };

    set_crc(0);

    if get_swap_halfwords_flag() {
        if (file_hdr.c_filesize % 4) == 0 {
            set_swapping_halfwords(true);
        } else {
            error(
                0,
                0,
                format_args!(
                    "cannot swap halfwords of {}: odd number of halfwords",
                    file_hdr.get_c_name()
                ),
            );
        }
    }
    if get_swap_bytes_flag() {
        if (file_hdr.c_filesize % 2) == 0 {
            set_swapping_bytes(true);
        } else {
            error(
                0,
                0,
                format_args!(
                    "cannot swap bytes of {}: odd number of bytes",
                    file_hdr.get_c_name()
                ),
            );
        }
    }
    copy_files_tape_to_disk(
        input_tape,
        output_tape,
        in_file_des,
        &mut out_file_des,
        file_hdr.c_filesize as i32,
    );
    {
        disk_empty_output_buffer(output_tape, &mut out_file_des, true);
    }

    if to_stdout_option {
        if archive_format == ArchiveFormat::Crcascii && get_crc() != file_hdr.c_chksum as usize {
            eprintln!(
                "{}: checksum error (0x{:x}, should be 0x{:x})",
                file_hdr.get_c_name(),
                get_crc(),
                file_hdr.c_chksum
            );
        }
        tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
        return;
    }

    set_perms(Some(&out_file_des), file_hdr);

    // if unsafe { libc::close(out_file_des) } < 0 {
    //     close_error(&file_hdr.c_name);
    // }

    if archive_format == ArchiveFormat::Crcascii && get_crc() != file_hdr.c_chksum as usize {
        error(
            0,
            0,
            format_args!(
                "{}: checksum error (0x{:x}, should be 0x{:x})",
                file_hdr.get_c_name(),
                get_crc(),
                file_hdr.c_chksum
            ),
        );
    }

    tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
    if file_hdr.c_nlink > 1
        && (archive_format == ArchiveFormat::Newascii || archive_format == ArchiveFormat::Crcascii)
    {
        create_defered_links(file_hdr);
    }
}

pub fn copyin_device(file_hdr: &mut CpioFileStat) {
    let to_stdout_option = get_to_stdout_option();
    let archive_format = get_archive_format();
    let create_dir_flag = get_create_dir_flag();
    let no_chown_flag = get_no_chown_flag();
    let set_owner_flag = get_set_owner_flag();
    let set_group_flag = get_set_group_flag();
    let retain_time_flag = get_retain_time_flag();

    if to_stdout_option {
        return;
    }

    let link_res;

    if file_hdr.c_nlink > 1
        && archive_format != ArchiveFormat::Tar
        && archive_format != ArchiveFormat::Ustar
    {
        link_res = link_to_maj_min_ino(
            &file_hdr.get_c_name(),
            file_hdr.c_dev_maj as u32,
            file_hdr.c_dev_min,
            file_hdr.c_ino,
        );
        if link_res == 0 {
            return;
        }
    } else if archive_format == ArchiveFormat::Ustar
        && file_hdr.c_tar_linkname.is_some()
        && !file_hdr.c_tar_linkname.as_ref().unwrap().is_empty()
    {
        link_res = link_to_name(
            &file_hdr.get_c_name(),
            file_hdr.c_tar_linkname.as_ref().unwrap(),
        );
        if link_res < 0 {
            let tar_linkname = file_hdr.c_tar_linkname.clone().unwrap_or_default();

            error(
                0,
                0,
                format_args!("cannot link {} to {}", tar_linkname, file_hdr.get_c_name()),
            );
        }
        return;
    }

    let dev = makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8);
    let res: i32 = unsafe {
        let c_name_ptr = file_hdr.get_c_name().as_ptr();
        let mode = file_hdr.c_mode;

        libc::mknod(c_name_ptr as *const libc::c_char, mode, dev as dev_t)
    };

    if res < 0 && create_dir_flag {
        create_all_directories(&file_hdr.get_c_name());
        {
            let dev = makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8);
            let res = unsafe {
                let c_name_ptr = file_hdr.get_c_name().as_ptr();
                let mode = file_hdr.c_mode;

                libc::mknod(c_name_ptr as *const libc::c_char, mode, dev as dev_t)
            };
            if res < 0 {
                mknod_error(&file_hdr.get_c_name());
                return;
            }
        }
    }
    if res < 0 {
        mknod_error(&file_hdr.get_c_name());
        return;
    }

    if !no_chown_flag {
        let uid = if set_owner_flag {
            get_set_owner()
        } else {
            file_hdr.c_uid
        };
        let gid = if set_group_flag {
            get_set_group()
        } else {
            file_hdr.c_gid
        };

        let chown_res = unsafe {
            let c_name_ptr = file_hdr.get_c_name().as_ptr();

            libc::chown(c_name_ptr as *const libc::c_char, uid, gid)
        };
        if chown_res < 0 {
            // 对于符号链接，更宽容地处理权限设置错误
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::EPERM) | Some(libc::ENOENT) | Some(libc::EROFS) | Some(libc::EINVAL)
                | Some(libc::EACCES) | Some(libc::ENOTSUP) => {
                    // 这些错误对于符号链接来说是可以忽略的
                }
                _ => {
                    chown_error_details(&file_hdr.get_c_name(), uid, gid);
                }
            }
        }
    }

    let chmod_res = unsafe {
        let c_name_ptr = file_hdr.get_c_name().as_ptr();
        libc::chmod(c_name_ptr as *const libc::c_char, file_hdr.c_mode)
    };
    if chmod_res < 0 {
        chmod_error_details(&file_hdr.get_c_name(), file_hdr.c_mode);
    }

    if retain_time_flag {
        set_file_times(
            None,
            &file_hdr.get_c_name(),
            file_hdr.c_mtime,
            file_hdr.c_mtime,
            0,
        );
    }
}

pub fn symlink_placeholder(oldpath: &str, newpath: &str, file_stat: &CpioFileStat) -> i32 {
    let fd = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(newpath)
        .map(|file| file.as_raw_fd());
    let out_file_des = match fd {
        Ok(file) => file.as_raw_fd(),
        Err(_) if get_create_dir_flag() => {
            create_all_directories(newpath);
            OpenOptions::new()
                .create(true)
                .write(true)
                .custom_flags(libc::O_CREAT | libc::O_WRONLY)
                .mode(0o600)
                .open(newpath)
                .map(|file| file.as_raw_fd())
                .unwrap_or(-1)
        }
        Err(_) => -1,
    };

    if out_file_des < 0 {
        open_error(newpath);
        return -1;
    }

    let metadata = fs::metadata(newpath).map_err(|_e| {
        stat_error(newpath);
        -1
    });

    //unsafe { libc::close(fd) };

    let metadata = metadata.unwrap(); // 此时是安全的，上面已经检查过了
    let key = DelayedLinkKey {
        dev: metadata.dev(),
        ino: metadata.ino(),
    };

    let value = DelayedLinkValue {
        mode: file_stat.c_mode,
        uid: file_stat.c_uid,
        gid: file_stat.c_gid,
        mtime: file_stat.c_mtime,
        source: oldpath.to_string(),
        target: newpath.to_string(),
    };

    let mut delayed_link: std::sync::MutexGuard<'_, DelayedLink> =
        GLOBAL_DELAYED_LINK.lock().unwrap();
    delayed_link.insert(key, value);

    0
}

fn replace_symlink_placeholders() {
    let mut delayed_link: std::sync::MutexGuard<'_, DelayedLink> =
        GLOBAL_DELAYED_LINK.lock().unwrap();

    if delayed_link.is_empty() {
        return;
    }

    for (key, dl) in delayed_link.table.iter() {
        let metadata = fs::symlink_metadata(&dl.target);
        if let Ok(st) = metadata {
            if st.dev() == key.dev && st.ino() == key.ino {
                if unsafe { unlink(dl.target.as_ptr() as *const libc::c_char) != 0 } {
                    unlink_error(&dl.target);
                } else {
                    let source_cstr = std::ffi::CString::new(dl.source.as_str()).unwrap();
                    let target_cstr = std::ffi::CString::new(dl.target.as_str()).unwrap();
                    let mut res = unsafe { symlink(source_cstr.as_ptr(), target_cstr.as_ptr()) };
                    if res < 0 && get_create_dir_flag() {
                        create_all_directories(&dl.target);
                        res = unsafe { symlink(source_cstr.as_ptr(), target_cstr.as_ptr()) };
                    }
                    if res < 0 {
                        error(
                            0,
                            0,
                            format_args!(
                                "{:?}: Cannot create symlink to {:?}",
                                quotearg_colon(&dl.target),
                                quotearg(&dl.source)
                            ),
                        );
                    } else {
                        if !get_no_chown_flag() {
                            let uid = if get_set_owner_flag() {
                                get_set_owner()
                            } else {
                                dl.uid
                            };
                            let gid = if get_set_group_flag() {
                                get_set_group()
                            } else {
                                dl.gid
                            };
                            if unsafe {
                                libc::lchown(dl.target.as_ptr() as *const libc::c_char, uid, gid)
                            } != 0
                            {
                                // 对于符号链接，更宽容地处理权限设置错误
                                let err = io::Error::last_os_error();
                                match err.raw_os_error() {
                                    Some(libc::EPERM) | Some(libc::ENOENT) | Some(libc::EROFS)
                                    | Some(libc::EINVAL) | Some(libc::EACCES)
                                    | Some(libc::ENOTSUP) => {
                                        // 这些错误对于符号链接来说是可以忽略的
                                    }
                                    _ => {
                                        chown_error_details(&dl.target, uid, gid);
                                    }
                                }
                            }
                        }
                        if get_retain_time_flag() {
                            set_file_times(None, &dl.target, dl.mtime, dl.mtime, 0);
                        }
                    }
                }
            }
        }
    }
    delayed_link.table.clear();
}

fn copyin_link(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
) {
    let link_name = get_link_name(input_tape, file_hdr, in_file_des);
    if link_name.is_none() {
        return;
    }

    let str_link_name = link_name.unwrap();

    if get_no_abs_paths_flag() {
        symlink_placeholder(&str_link_name, file_hdr.get_c_name().as_str(), file_hdr);
    } else {
        let source_cstr = std::ffi::CString::new(str_link_name.clone()).unwrap();
        let target_cstr = std::ffi::CString::new(file_hdr.get_c_name().clone()).unwrap();
        let mut res = unsafe { symlink(source_cstr.as_ptr(), target_cstr.as_ptr()) };
        if res < 0 && get_create_dir_flag() {
            create_all_directories(&file_hdr.get_c_name());
            res = unsafe { symlink(source_cstr.as_ptr(), target_cstr.as_ptr()) };
        }
        if res < 0 {
            // 改进错误处理：对符号链接创建失败更加宽容
            let err = io::Error::last_os_error();
            match err.raw_os_error() {
                Some(libc::EPERM) | Some(libc::EACCES) | Some(libc::EROFS) => {
                    // 权限相关错误，可以忽略
                }
                Some(libc::EEXIST) => {
                    // 文件已存在，可以忽略
                }
                Some(libc::ENOENT) => {
                    // 目标路径不存在，可以忽略
                }
                Some(libc::EINVAL) | Some(libc::ENOTSUP) => {
                    // 无效参数或不支持，可以忽略
                }
                _ => {
                    // 其他错误仍然报告，但不中断程序
                    error(
                        err.raw_os_error().unwrap_or(0),
                        0,
                        format_args!(
                            "{:?}: Cannot create symlink to {:?}",
                            quotearg_colon(&file_hdr.get_c_name()),
                            quotearg(&str_link_name)
                        ),
                    );
                }
            }
        } else if !get_no_chown_flag() {
            let uid = if get_set_owner_flag() {
                get_set_owner()
            } else {
                file_hdr.c_uid
            };
            let gid = if get_set_group_flag() {
                get_set_group()
            } else {
                file_hdr.c_gid
            };
            if unsafe {
                lchown(
                    file_hdr.get_c_name().as_ptr() as *const libc::c_char,
                    u32::from(Uid::from_raw(uid)),
                    u32::from(Gid::from_raw(gid)),
                )
            } != 0
            {
                // 对于符号链接，更宽容地处理权限设置错误
                let err = io::Error::last_os_error();
                match err.raw_os_error() {
                    Some(libc::EPERM) | Some(libc::ENOENT) | Some(libc::EROFS)
                    | Some(libc::EINVAL) | Some(libc::EACCES) | Some(libc::ENOTSUP) => {
                        // 这些错误对于符号链接来说是可以忽略的
                    }
                    _ => {
                        chown_error_details(&file_hdr.get_c_name(), uid, gid);
                    }
                }
            }
        }

        if get_retain_time_flag() {
            set_file_times(
                None,
                &file_hdr.get_c_name(),
                file_hdr.c_mtime,
                file_hdr.c_mtime,
                0,
            );
        }
    }
}

fn copyin_file(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_file_des: &mut File,
) {
    let mut existing_dir = false;

    if !get_to_stdout_option()
        && try_existing_file(input_tape, file_hdr, in_file_des, &mut existing_dir) < 0
    {
        return;
    }

    match file_hdr.c_mode & CP_IFMT {
        CP_IFREG => copyin_regular_file(output_tape, input_tape, file_hdr, in_file_des),
        CP_IFDIR => {
            cpio_create_dir(file_hdr, existing_dir);
        }
        CP_IFCHR | CP_IFBLK => copyin_device(file_hdr),
        CP_IFLNK => copyin_link(input_tape, file_hdr, in_file_des),
        _ => {
            error(
                0,
                0,
                format_args!("{}: unknown file type", file_hdr.get_c_name()),
            );
            tape_toss_input(input_tape, in_file_des, file_hdr.c_filesize as i32);
            tape_skip_padding(input_tape, in_file_des, file_hdr.c_filesize as u64);
        }
    }
}

fn format_time(when: i64) -> String {
    let datetime = Utc.timestamp_opt(when, 0).unwrap();

    datetime.format("%a %b %d %H:%M:%S %Y").to_string()
}

fn long_format(file_hdr: &mut CpioFileStat, link_name: Option<String>) {
    let mut mbuf: [char; 11] = ['\0'; 11];

    //let mut when_timespec = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(when as u64);

    let mut current_time = SystemTime::now();
    let six_months_ago = current_time - std::time::Duration::from_secs(15778476); // 直接计算6个月的秒数

    mode_string(file_hdr.c_mode, &mut mbuf);
    mbuf[10] = '\0';

    print!(
        "{} {:3} ",
        mbuf.iter().collect::<String>(),
        file_hdr.c_nlink
    );

    if get_numeric_uid() {
        print!("{:<8} {:<8} ", file_hdr.c_uid, file_hdr.c_gid);
    } else {
        print!(
            "{:<8.8} {:<8.8} ",
            getuser(file_hdr.c_uid),
            getgroup(file_hdr.c_gid)
        );
    }

    if (file_hdr.c_mode & CP_IFMT) == CP_IFCHR || (file_hdr.c_mode & CP_IFMT) == CP_IFBLK {
        print!("{:3}, {:3} ", file_hdr.c_rdev_maj, file_hdr.c_rdev_min);
    } else {
        print!("{:8} ", file_hdr.c_filesize);
    }

    let when = file_hdr.c_mtime;
    let when_timespec = SystemTime::UNIX_EPOCH + Duration::new(when.try_into().unwrap(), 0);

    let binding = format_time(when).clone();
    let mut tbuf = binding.into_bytes();

    //    let tbuf = when_timespec.duration_since(UNIX_EPOCH).unwrap().as_secs();

    if when_timespec > current_time {
        current_time = SystemTime::now();
    }

    if !(six_months_ago < when_timespec && when_timespec < current_time) {
        let (left, right) = tbuf.split_at_mut(16);
        left[11..16].copy_from_slice(&right[3..8]); // Copy year " 1993"
    }

    // 调整 tbuf，去掉星期和换行符
    tbuf[16] = b' '; // 将时间部分替换为空格
    let tbuf_str = String::from_utf8_lossy(&tbuf[4..17]).to_string(); // 从第 5 个字符开始，取 13 个字符

    // 打印时间
    print!("{}", tbuf_str);

    // 打印文件名和链接名
    print!("{}", quotearg(file_hdr.get_c_name().as_str()));
    if let Some(link) = link_name {
        print!(" -> {}", quotearg(link.as_str()));
    }
    println!(); // 换行

    // todo
}

fn read_pattern_file() {
    let mut new_save_patterns: Vec<String> = Vec::new();
    let mut max_new_patterns: usize;
    let mut new_num_patterns: usize;
    //let pattern_name = DYNAMIC_STRING_INITIALIZER;
    //let pattern_fp: File;

    if get_num_patterns() < 0 {
        set_num_patterns(0);
    }
    new_num_patterns = get_num_patterns() as usize;
    max_new_patterns = get_num_patterns() as usize;
    new_save_patterns.reserve(max_new_patterns);

    let out_file_des = OpenOptions::new()
        .create(true)
        .write(true)
        .custom_flags(libc::O_CREAT | libc::O_WRONLY)
        .mode(0o600)
        .open(get_pattern_file_name().unwrap());
    match out_file_des {
        Ok(file) => {
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let line = line.unwrap();
                if new_num_patterns == max_new_patterns {
                    max_new_patterns *= 2;
                    new_save_patterns.reserve(max_new_patterns);
                }
                new_save_patterns.push(line);
                new_num_patterns += 1;
            }

            // if let Err(e) = reader.into_inner().sync_all() {
            //     close_error(get_pattern_file_name().as_deref().unwrap());
            // }
        }
        Err(_) => {
            open_error(get_pattern_file_name().as_deref().unwrap());
        }
    }

    for i in 0..get_num_patterns() as usize {
        new_save_patterns[i] = get_save_patterns()[i].clone();
    }

    set_save_patterns(new_save_patterns);
    set_num_patterns(new_num_patterns as i32);
}

pub fn from_octal(where_: &Vec<u8>) -> u64 {
    from_ascii(where_, where_.len(), LG_8)
}
pub fn from_hex(where_: &Vec<u8>) -> u64 {
    from_ascii(where_, where_.len(), LG_16)
}

fn from_ascii(where_: &Vec<u8>, digs: usize, logbase: u32) -> u64 {
    let mut value: u64 = 0;
    let buf = where_.as_slice();
    // let end = buf.len();
    let mut overflow = false;
    let codetab = b"0123456789ABCDEF";

    let mut buf_iter = buf.iter().take(digs).skip_while(|&&c| c == b' ');

    if buf_iter.clone().count() == 0 || buf_iter.clone().next() == Some(&0) {
        return 0;
    }

    while let Some(&c) = buf_iter.next() {
        let p = codetab.iter().position(|&x| x == c.to_ascii_uppercase());
        if let Some(d) = p {
            if (d >> logbase) > 1 {
                error(0, 0, format_args!("Malformed number {} {:?}", digs, where_));
                break;
            }
            value += d as u64;
            if buf_iter.clone().count() == 0 || buf_iter.clone().next() == Some(&0) {
                break;
            }
            overflow |= (value ^ (value << logbase >> logbase)) != 0;
            value <<= logbase;
        } else {
            error(0, 0, format_args!("Malformed number {} {:?}", digs, where_));
            break;
        }
    }

    if overflow {
        error(
            0,
            0,
            format_args!("Archive value {} {:?} is out of range", digs, where_),
        );
    }
    value
}

fn swab_short(i: u16) -> u16 {
    ((i << 8) & 0xff00) | ((i >> 8) & 0x00ff)
}

#[derive(Debug)]
enum Magic {
    Str([u8; 6]),
    Num(u16),
    OldHeader(OldCpioHeader),
}
#[derive(Debug)]
enum TmpBuf {
    S([u8; 512]),
    Us(u16),
}

pub fn read_in_header(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_des: &mut File,
) {
    let magic = Magic::Str([0; 6]);
    let mut bytes_skipped: i64 = 0;

    let mut archive_format = get_archive_format();

    if archive_format == ArchiveFormat::Unknown {
        let mut check_tar;
        let mut peeked_bytes: i32;

        let tmpbuf = TmpBuf::S([0; 512]);

        while archive_format == ArchiveFormat::Unknown {
            let mut tmpbuf_s = match tmpbuf {
                TmpBuf::S(arr) => arr,
                _ => unreachable!(),
            };

            peeked_bytes = tape_buffered_peek(input_tape, &mut tmpbuf_s, in_des, 512);
            //input_tape.test(0);
            if peeked_bytes < 6 {
                error(0, 0, format_args!("premature end of archive"));
            }

            let hdr = str::from_utf8(&tmpbuf_s[..6]).unwrap_or_default();

            if hdr == "070701" {
                archive_format = ArchiveFormat::Newascii;
            } else if hdr == "070707" {
                archive_format = ArchiveFormat::Oldascii;
            } else if hdr == "070702" {
                archive_format = ArchiveFormat::Crcascii;
                set_crc_i_flag(true);
            } else if peeked_bytes >= 2 && {
                let us = u16::from_le_bytes([tmpbuf_s[0], tmpbuf_s[1]]);
                us == 0o070707 || us == swab_short(0o070707)
            } {
                archive_format = ArchiveFormat::Binary;
            } else if peeked_bytes >= 512 && {
                check_tar = is_tar_header(&tmpbuf_s);
                check_tar != 0
            } {
                if check_tar == 2 {
                    archive_format = ArchiveFormat::Ustar;
                } else {
                    archive_format = ArchiveFormat::Tar;
                }
            } else {
                tape_buffered_read(input_tape, &mut tmpbuf_s[..1], in_des, 1);
                bytes_skipped += 1;
            }
        }
        set_archive_format(archive_format);
    }

    if archive_format == ArchiveFormat::Tar || archive_format == ArchiveFormat::Ustar {
        if get_append_flag() {
            let last_header_start =
                input_tape.input_bytes - get_io_block_size() as usize + input_tape.in_buff;
            set_last_header_start(last_header_start as i32);
        }
        if bytes_skipped > 0 {
            warn_junk_bytes(bytes_skipped as u64);
        }
        read_in_tar_header(input_tape, file_hdr, in_des);
        return;
    }

    file_hdr.c_tar_linkname = None;

    let mut magic_str = match magic {
        Magic::Str(arr) => arr,
        _ => unreachable!(),
    };

    tape_buffered_read(input_tape, &mut magic_str, in_des, 6);

    loop {
        if get_append_flag() {
            let last_header_start =
                input_tape.input_bytes - get_io_block_size() as usize + input_tape.in_buff - 6;
            set_last_header_start(last_header_start as i32);
        }
        if archive_format == ArchiveFormat::Newascii && &magic_str == b"070701" {
            if bytes_skipped > 0 {
                warn_junk_bytes(bytes_skipped as u64);
            }
            file_hdr.c_magic = 0o70701;
            read_in_new_ascii(input_tape, file_hdr, in_des);
            break;
        }
        if archive_format == ArchiveFormat::Crcascii && &magic_str == b"070702" {
            if bytes_skipped > 0 {
                warn_junk_bytes(bytes_skipped as u64);
            }
            file_hdr.c_magic = 0o70702;
            read_in_new_ascii(input_tape, file_hdr, in_des);
            break;
        }
        if (archive_format == ArchiveFormat::Oldascii
            || archive_format == ArchiveFormat::Hpoldascii)
            && &magic_str == b"070707"
        {
            if bytes_skipped > 0 {
                warn_junk_bytes(bytes_skipped as u64);
            }
            file_hdr.c_magic = 0o70707;
            read_in_old_ascii(input_tape, file_hdr, in_des);
            break;
        }
        if archive_format == ArchiveFormat::Binary || archive_format == ArchiveFormat::Hpbinary {
            let num = u16::from_le_bytes([magic_str[0], magic_str[1]]);

            if num == 0o70707 || num == swab_short(0o70707) {
                if bytes_skipped > 0 {
                    warn_junk_bytes(bytes_skipped as u64);
                }
                file_hdr.c_magic = 0o70707;

                let mut old_header: OldCpioHeader = OldCpioHeader::from_bytes(magic_str);

                read_in_binary(input_tape, file_hdr, &mut old_header, in_des);
                break;
            }
        }
        bytes_skipped += 1;
        magic_str.copy_within(1.., 0);
        tape_buffered_read(input_tape, &mut magic_str[5..6], in_des, 1);
    }
}

fn read_name_from_file(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    file: &mut File,
    len: usize,
) {
    const MAX_FILENAME_SIZE: usize = 1024 * 1024; // 1MB 作为文件名长度上限

    if len == 0 {
        error(
            0,
            0,
            format_args!("malformed header: file name of zero length"),
        );
    } else if len > MAX_FILENAME_SIZE {
        error(
            0,
            0,
            format_args!("malformed header: file name too long ({})", len),
        );
        // 跳过这个文件
        tape_toss_input(input_tape, file, len as i32);
    } else {
        //cpio_realloc_c_name(file_hdr, len);
        // 用 len 分配一个内存
        let mut in_buf = vec![0; len];

        tape_buffered_read(input_tape, &mut in_buf, file, len);
        file_hdr.set_c_name(String::from_utf8_lossy(&in_buf).as_ref());
        // if file_hdr.c_name.as_bytes()[len as usize - 1] != 0 {
        //     error(
        //         0,
        //         0,
        //         format_args!("malformed header: file name is not nul-terminated"),
        //     );
        //     len = 0;
        // }
    }
    file_hdr.c_namesize = len;
}

fn read_in_old_ascii(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_des: &mut File,
) {
    let mut dev: u64;

    // Read header fields individually
    let mut c_dev = [0u8; 6];
    let mut c_ino = [0u8; 6];
    let mut c_mode = [0u8; 6];
    let mut c_uid = [0u8; 6];
    let mut c_gid = [0u8; 6];
    let mut c_nlink = [0u8; 6];
    let mut c_rdev = [0u8; 6];
    let mut c_mtime = [0u8; 11];
    let mut c_namesize = [0u8; 6];
    let mut c_filesize = [0u8; 11];

    tape_buffered_read(input_tape, &mut c_dev, in_des, 6);
    tape_buffered_read(input_tape, &mut c_ino, in_des, 6);
    tape_buffered_read(input_tape, &mut c_mode, in_des, 6);
    tape_buffered_read(input_tape, &mut c_uid, in_des, 6);
    tape_buffered_read(input_tape, &mut c_gid, in_des, 6);
    tape_buffered_read(input_tape, &mut c_nlink, in_des, 6);
    tape_buffered_read(input_tape, &mut c_rdev, in_des, 6);
    tape_buffered_read(input_tape, &mut c_mtime, in_des, 11);
    tape_buffered_read(input_tape, &mut c_namesize, in_des, 6);
    tape_buffered_read(input_tape, &mut c_filesize, in_des, 11);

    dev = from_octal(&c_dev.to_vec());
    file_hdr.c_dev_maj = major(dev as u32) as i32;
    file_hdr.c_dev_min = minor(dev as u32) as u32;

    file_hdr.c_ino = from_octal(&c_ino.to_vec());
    file_hdr.c_mode = from_octal(&c_mode.to_vec()) as u32;
    file_hdr.c_uid = from_octal(&c_uid.to_vec()) as u32;
    file_hdr.c_gid = from_octal(&c_gid.to_vec()) as u32;
    file_hdr.c_nlink = from_octal(&c_nlink.to_vec()) as usize;
    dev = from_octal(&c_rdev.to_vec());
    file_hdr.c_rdev_maj = major(dev as u32) as i32;
    file_hdr.c_rdev_min = minor(dev as u32) as u32;

    file_hdr.c_mtime = from_octal(&c_mtime.to_vec()) as i64;
    file_hdr.c_filesize = from_octal(&c_filesize.to_vec()) as i64;
    read_name_from_file(
        input_tape,
        file_hdr,
        in_des,
        from_octal(&c_namesize.to_vec()) as usize,
    );

    match file_hdr.c_mode & CP_IFMT {
        CP_IFCHR | CP_IFBLK | CP_IFSOCK | CP_IFIFO => {
            if file_hdr.c_filesize != 0 && file_hdr.c_rdev_maj == 0 && file_hdr.c_rdev_min == 1 {
                file_hdr.c_rdev_maj = major(file_hdr.c_filesize as u32) as i32;
                file_hdr.c_rdev_min = minor(file_hdr.c_filesize as u32) as u32;
                file_hdr.c_filesize = 0;
            }
        }
        _ => {}
    }
}

fn read_in_new_ascii(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_des: &mut File,
) {
    // Read header fields individually
    let mut c_ino = [0u8; 8];
    let mut c_mode = [0u8; 8];
    let mut c_uid = [0u8; 8];
    let mut c_gid = [0u8; 8];
    let mut c_nlink = [0u8; 8];
    let mut c_mtime = [0u8; 8];
    let mut c_filesize = [0u8; 8];
    let mut c_dev_maj = [0u8; 8];
    let mut c_dev_min = [0u8; 8];
    let mut c_rdev_maj = [0u8; 8];
    let mut c_rdev_min = [0u8; 8];
    let mut c_namesize = [0u8; 8];
    let mut c_chksum = [0u8; 8];

    tape_buffered_read(input_tape, &mut c_ino, in_des, 8);
    tape_buffered_read(input_tape, &mut c_mode, in_des, 8);
    tape_buffered_read(input_tape, &mut c_uid, in_des, 8);
    tape_buffered_read(input_tape, &mut c_gid, in_des, 8);
    tape_buffered_read(input_tape, &mut c_nlink, in_des, 8);
    tape_buffered_read(input_tape, &mut c_mtime, in_des, 8);
    tape_buffered_read(input_tape, &mut c_filesize, in_des, 8);
    tape_buffered_read(input_tape, &mut c_dev_maj, in_des, 8);
    tape_buffered_read(input_tape, &mut c_dev_min, in_des, 8);
    tape_buffered_read(input_tape, &mut c_rdev_maj, in_des, 8);
    tape_buffered_read(input_tape, &mut c_rdev_min, in_des, 8);
    tape_buffered_read(input_tape, &mut c_namesize, in_des, 8);
    tape_buffered_read(input_tape, &mut c_chksum, in_des, 8);

    file_hdr.c_ino = from_hex(&c_ino.to_vec());
    file_hdr.c_mode = from_hex(&c_mode.to_vec()) as u32;
    file_hdr.c_uid = from_hex(&c_uid.to_vec()) as u32;
    file_hdr.c_gid = from_hex(&c_gid.to_vec()) as u32;
    file_hdr.c_nlink = from_hex(&c_nlink.to_vec()) as usize;
    file_hdr.c_mtime = from_hex(&c_mtime.to_vec()) as i64;
    file_hdr.c_filesize = from_hex(&c_filesize.to_vec()) as i64;
    file_hdr.c_dev_maj = major(from_hex(&c_dev_maj.to_vec()) as u32) as i32;
    file_hdr.c_dev_min = minor(from_hex(&c_dev_min.to_vec()) as u32) as u32;
    file_hdr.c_rdev_maj = major(from_hex(&c_rdev_maj.to_vec()) as u32) as i32;
    file_hdr.c_rdev_min = minor(from_hex(&c_rdev_min.to_vec()) as u32) as u32;
    file_hdr.c_chksum = from_hex(&c_chksum.to_vec()) as u32;

    read_name_from_file(
        input_tape,
        file_hdr,
        in_des,
        from_hex(&c_namesize.to_vec()) as usize,
    );

    tape_skip_padding(input_tape, in_des, (file_hdr.c_namesize + 110) as u64);
}

fn read_in_binary(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    old_hdr: &mut OldCpioHeader,
    in_des: &mut File,
) {
    file_hdr.c_magic = old_hdr.c_magic;

    // 6 实际上输入的都占用的字节数目，暂时这里记录一下
    tape_buffered_read(
        input_tape,
        &mut old_hdr.as_mut_slice()[6..],
        in_des,
        std::mem::size_of::<OldCpioHeader>() - 6,
    );

    if file_hdr.c_magic == swab_short(0o70707u16) {
        static mut WARNED: bool = false;

        unsafe {
            if !WARNED {
                error(
                    0,
                    0,
                    format_args!("warning: archive header has reverse byte-order"),
                );
                WARNED = true;
            }
        }

        swab_array(old_hdr.as_mut_slice(), 13);
    }

    file_hdr.c_dev_maj = major(old_hdr.c_dev as u32) as i32;
    file_hdr.c_dev_min = minor(old_hdr.c_dev as u32) as u32;
    file_hdr.c_ino = old_hdr.c_ino as u64;
    file_hdr.c_mode = old_hdr.c_mode as u32;
    file_hdr.c_uid = old_hdr.c_uid as u32;
    file_hdr.c_gid = old_hdr.c_gid as u32;
    file_hdr.c_nlink = old_hdr.c_nlink as usize;
    file_hdr.c_rdev_maj = major(old_hdr.c_rdev as u32) as i32;
    file_hdr.c_rdev_min = minor(old_hdr.c_rdev as u32) as u32;
    file_hdr.c_mtime = (old_hdr.c_mtimes[0] as i64) << 16 | old_hdr.c_mtimes[1] as i64;
    file_hdr.c_filesize = (old_hdr.c_filesizes[0] as i64) << 16 | old_hdr.c_filesizes[1] as i64;
    read_name_from_file(input_tape, file_hdr, in_des, old_hdr.c_namesize as usize);

    if file_hdr.c_namesize % 2 != 0 {
        tape_toss_input(input_tape, in_des, 1);
    }

    match file_hdr.c_mode & CP_IFMT {
        CP_IFCHR | CP_IFBLK | CP_IFSOCK | CP_IFIFO => {
            if file_hdr.c_filesize != 0 && file_hdr.c_rdev_maj == 0 && file_hdr.c_rdev_min == 1 {
                file_hdr.c_rdev_maj = major(file_hdr.c_filesize as u32) as i32;
                file_hdr.c_rdev_min = minor(file_hdr.c_filesize as u32) as u32;
                file_hdr.c_filesize = 0;
            }
        }
        _ => {}
    }
}

fn swab_array(ptr: &mut [u8], count: usize) {
    for i in 0..count {
        ptr.swap(i * 2, i * 2 + 1);
    }
}

pub fn process_copy_in() -> io::Result<()> {
    let mut tty_in: Option<File> = None;
    let mut tty_out: Option<File> = None;
    let mut rename_in: Option<File> = None;
    // let mut file_stat = std::fs::metadata("/dev/tty")?;
    let mut file_hdr = CpioFileStat::new();

    let mut skip_file: bool;

    // let mut delayed_link: std::sync::MutexGuard<'_, DelayedLink> =
    //     GLOBAL_DELAYED_LINK.lock().unwrap();

    set_newdir_umask(unsafe { umask(0) });

    // Initialize the copy in
    if get_pattern_file_name().is_some() {
        read_pattern_file();
    }

    if let Some(rename_batch_file) = get_rename_batch_file() {
        rename_in = Some(File::open(rename_batch_file)?);
        if rename_in.is_none() {
            error(PAXEXIT_FAILURE, errno(), format_args!("{}", TTY_NAME));
        }
    } else if get_rename_flag() {
        tty_in = Some(File::open("/dev/tty")?);
        if tty_in.is_none() {
            error(PAXEXIT_FAILURE, errno(), format_args!("{}", TTY_NAME));
        }
        tty_out = Some(File::create("/dev/tty")?);
        if tty_out.is_none() {
            error(PAXEXIT_FAILURE, errno(), format_args!("{}", TTY_NAME));
        }
    }

    let tabflag = get_table_flag();
    let verbflag = get_verbose_flag();

    if tabflag && verbflag {
        unsafe { CURRENT_TIME = current_timespec() };
    }

    let mut input_tape: std::sync::MutexGuard<'_, TapeInput> = TAPE_INPUT.lock().map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to lock TAPE_INPUT: {}", e),
        )
    })?;
    let mut output_tape: std::sync::MutexGuard<'_, TapeOutput> =
        TAPE_OUTPUT.lock().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to lock TAPE_OUTPUT: {}", e),
            )
        })?;

    let mut in_file_des = get_archive_des()?;

    // Check if input is a tape
    if isrmt(&mut in_file_des) {
        input_tape.input_is_seekable = false;
        input_tape.input_is_special = true;
    } else {
        let stat = fstat(in_file_des.as_raw_fd())?;
        input_tape.input_is_special = stat.st_mode & libc::S_IFMT == libc::S_IFBLK
            || stat.st_mode & libc::S_IFMT == libc::S_IFCHR;
        input_tape.input_is_seekable = stat.st_mode & libc::S_IFMT == libc::S_IFREG;
    }

    output_tape.output_is_seekable = true;

    change_dir();

    // Process each file in the archive
    loop {
        set_swapping_bytes(false);
        set_swapping_halfwords(false);
        read_in_header(&mut input_tape, &mut file_hdr, &mut in_file_des);

        if file_hdr.c_namesize == 0 {
            skip_file = true;
        } else {
            let name = file_hdr.get_c_name();
            if name == CPIO_TRAILER_NAME {
                break;
            }

            cpio_safer_name_suffix(&mut name.clone(), false, !get_no_abs_paths_flag(), false);

            let num_patterns = get_num_patterns();

            if get_num_patterns() <= 0 {
                skip_file = false;
            } else {
                skip_file = get_copy_matching_files();
                for i in 0..num_patterns {
                    let pattern_cstr =
                        std::ffi::CString::new(get_save_patterns()[i as usize].as_str()).unwrap();
                    let name_cstr = std::ffi::CString::new(name.clone()).unwrap();
                    if unsafe { fnmatch(pattern_cstr.as_ptr(), name_cstr.as_ptr(), 0) } == 0 {
                        skip_file = !get_copy_matching_files();
                        break;
                    }
                }
            }
        }

        if skip_file {
            if file_hdr.c_nlink > 1
                && (get_archive_format() == ArchiveFormat::Newascii
                    || get_archive_format() == ArchiveFormat::Crcascii)
            {
                if create_defered_links_to_skipped(
                    &mut output_tape,
                    &mut input_tape,
                    &mut file_hdr,
                    &mut in_file_des,
                ) < 0
                {
                    tape_toss_input(
                        &mut input_tape,
                        &mut in_file_des,
                        file_hdr.c_filesize as i32,
                    );
                    tape_skip_padding(
                        &mut input_tape,
                        &mut in_file_des,
                        file_hdr.c_filesize as u64,
                    );
                }
            } else {
                tape_toss_input(
                    &mut input_tape,
                    &mut in_file_des,
                    file_hdr.c_filesize as i32,
                );
                tape_skip_padding(
                    &mut input_tape,
                    &mut in_file_des,
                    file_hdr.c_filesize as u64,
                );
            }
        } else if get_table_flag() {
            list_file(&mut input_tape, &mut file_hdr, &mut in_file_des);
        } else if get_append_flag() {
            tape_toss_input(
                &mut input_tape,
                &mut in_file_des,
                file_hdr.c_filesize as i32,
            );
            tape_skip_padding(
                &mut input_tape,
                &mut in_file_des,
                file_hdr.c_filesize as u64,
            );
        } else if get_only_verify_crc_flag() {
            if (file_hdr.c_mode & CP_IFMT) == CP_IFLNK
                && get_archive_format() != ArchiveFormat::Tar
                && get_archive_format() != ArchiveFormat::Ustar
            {
                tape_toss_input(
                    &mut input_tape,
                    &mut in_file_des,
                    file_hdr.c_filesize as i32,
                );
                tape_skip_padding(
                    &mut input_tape,
                    &mut in_file_des,
                    file_hdr.c_filesize as u64,
                );
                continue;
            }
            let crc = 0;
            tape_toss_input(
                &mut input_tape,
                &mut in_file_des,
                file_hdr.c_filesize as i32,
            );
            tape_skip_padding(
                &mut input_tape,
                &mut in_file_des,
                file_hdr.c_filesize as u64,
            );
            if crc != file_hdr.c_chksum {
                eprintln!(
                    "{}: checksum error (0x{:x}, should be 0x{:x})",
                    file_hdr.get_c_name(),
                    crc,
                    file_hdr.c_chksum
                );
            }
            if get_verbose_flag() {
                eprintln!("{}", file_hdr.get_c_name());
            }
            if get_dot_flag() {
                eprint!(".");
            }
        } else {
            if (get_rename_flag() || get_rename_batch_file().is_some())
                && query_rename(
                    &mut file_hdr,
                    tty_in.as_mut().unwrap(),
                    tty_out.as_mut().unwrap(),
                    rename_in.as_mut().unwrap(),
                ) < 0
            {
                tape_toss_input(
                    &mut input_tape,
                    &mut in_file_des,
                    file_hdr.c_filesize as i32,
                );
                tape_skip_padding(
                    &mut input_tape,
                    &mut in_file_des,
                    file_hdr.c_filesize as u64,
                );
                continue;
            }

            copyin_file(
                &mut output_tape,
                &mut input_tape,
                &mut file_hdr,
                &mut in_file_des,
            );

            if get_verbose_flag() {
                eprintln!("{}", file_hdr.get_c_name());
            }
            if get_dot_flag() {
                eprint!(".");
            }
        }
    }

    if get_dot_flag() {
        eprintln!();
    }

    replace_symlink_placeholders();
    apply_delayed_set_stat();

    if !get_append_flag() {
        if get_archive_format() == ArchiveFormat::Newascii
            || get_archive_format() == ArchiveFormat::Crcascii
        {
            create_final_defers();
        }
        if !get_quiet_flag() {
            let blocks = (input_tape.input_bytes + get_io_block_size() as usize - 1)
                / get_io_block_size() as usize;
            eprintln!("{} block{}", blocks, if blocks == 1 { "" } else { "" });
        }
    }

    input_tape.free();

    Ok(())
}

#[test]
fn test_format_time() {
    // Test Unix epoch
    assert_eq!(format_time(0), "Thu Jan 01 00:00:00 1970");

    // Test a specific date
    assert_eq!(
        format_time(1625097600), // 2021-06-30 00:00:00 UTC
        "Wed Jun 30 00:00:00 2021"
    );

    // Test leap second (though timestamp_opt will normalize it)
    assert_eq!(
        format_time(1483228799), // 2016-12-31 23:59:59 UTC
        "Sat Dec 31 23:59:59 2016"
    );

    // Test future date
    assert_eq!(
        format_time(2524608000), // 2050-01-01 00:00:00 UTC
        "Sat Jan 01 00:00:00 2050"
    );
}
