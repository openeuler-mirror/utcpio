/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(
    clippy::match_like_matches_macro,
    clippy::needless_late_init,
    clippy::type_complexity,
    dead_code
)]

use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::fs::Metadata;
use std::io;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::MutexGuard;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use gnu::error::ENOSPC;
use gnu::error::ENXIO;
use gnu::fdutimensat::*;
use gnu::quotearg::*;
use gnu::safe_read::SAFE_READ_ERROR;
use gnu::stripslash::*;
use gnu::xmalloc::*;

use nix::libc;
use nix::libc::chown;
use nix::libc::fchmod;
use nix::libc::fchown;
use nix::libc::mode_t;
use nix::libc::time_t;
use nix::libc::timespec;
use nix::libc::AT_FDCWD;
use nix::libc::EIO;

use nix::sys::stat::major;
use nix::sys::stat::minor;

use pax::paxerror::*;
use pax::paxlib::*;
use pax::paxnames::*;
use pax::rmt::*;

use crate::appargs::*;
// use crate::copyin::*;
use crate::cpiohdr::*;
use crate::externs::*;
use crate::filetype::*;
use crate::global::*;

use crate::util::libc::gid_t;
use crate::util::libc::uid_t;

use gnu::dirname::*;
use gnu::error::*;
use gnu::util::validate_and_sanitize_path;
use crate::copyin::process_copy_in;

use lazy_static::lazy_static;
use std::sync::Mutex;

static mut REEL_NUMBER: i32 = 1;
const DISKBLOCKSIZE: i32 = 512;
static mut NEXT_INODE: u64 = 0;

// fn raw_fd_to_file(raw_fd: i32) -> File {
//     unsafe { File::from_raw_fd(raw_fd) }
// }

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct InodeVal {
    pub inode: u64,                // ino_t
    pub major_num: u64,            // unsigned long
    pub minor_num: u64,            // unsigned long
    pub trans_inode: u64,          // ino_t
    pub file_name: Option<String>, // char*
}

// Inode hash table. Allocated by first call to add_inode.
lazy_static! {
    static ref HASH_TABLE: Mutex<HashMap<InodeVal, InodeVal>> = Mutex::new(HashMap::new());
}

// Placeholder constants
const MODE_RW: u32 = 0o666; // Example mode, adjust as needed

// Placeholder variables and functions (using Mutex for thread safety)

lazy_static! {
    static ref COPY_FUNCTION: Mutex<Option<fn() -> io::Result<()>>> = Mutex::new(None);
    static ref APPEND_FLAG: Mutex<bool> = Mutex::new(false);
    static ref RSH_COMMAND_OPTION: Mutex<Option<String>> = Mutex::new(None);
}

pub fn tape_empty_output_buffer(output_tape: &mut MutexGuard<TapeOutput>, out_file: &mut File) {
    // let mut output_tape = TAPE_OUTPUT.lock().unwrap();
    let output_size = output_tape.output_size;

    // 如果输出缓冲区为空，直接返回
    if output_size == 0 {
        return;
    }

    let bytes_written = rmtwrite(out_file, &output_tape.output_buffer, output_size);

    if bytes_written != output_size {
        if output_tape.output_is_special
            && (bytes_written != 0
                || (io::Error::last_os_error().raw_os_error() == Some(ENOSPC)
                    || io::Error::last_os_error().raw_os_error() == Some(EIO)
                    || io::Error::last_os_error().raw_os_error() == Some(ENXIO)))
        {
            get_next_reel(out_file);
            let rest_output_size = if bytes_written > 0 {
                output_size - bytes_written
            } else {
                output_size
            };

            let rest_bytes_written = rmtwrite(
                out_file,
                &output_tape.output_buffer[bytes_written..],
                rest_output_size,
            );

            if rest_bytes_written != rest_output_size {
                error(
                    PAXEXIT_FAILURE,
                    io::Error::last_os_error().raw_os_error().unwrap_or(0),
                    format_args!("write error"),
                );
            }
        } else {
            error(
                PAXEXIT_FAILURE,
                io::Error::last_os_error().raw_os_error().unwrap_or(0),
                format_args!("write error"),
            );
        }
    }
    output_tape.output_bytes += output_size;
    output_tape.out_buff = 0;
    output_tape.output_size = 0;
}

fn swab_array(ptr: &mut [u8], count: usize) {
    let mut current_ptr = 0;
    for _ in 0..count {
        if current_ptr + 1 < ptr.len() {
            ptr.swap(current_ptr, current_ptr + 1);
            current_ptr += 2;
        } else {
            break; // 防止越界
        }
    }
}

pub fn disk_empty_output_buffer(
    output_tape: &mut MutexGuard<TapeOutput>,
    file: &mut File,
    flush: bool,
) {
    //    let mut output_tape = TAPE_OUTPUT.lock().unwrap();

    if get_swapping_halfwords() || get_swapping_bytes() {
        if get_swapping_halfwords() {
            let complete_words = output_tape.output_size / 4;
            swahw_array(&mut output_tape.output_buffer, complete_words);
            if get_swapping_bytes() {
                swab_array(&mut output_tape.output_buffer, 2 * complete_words);
            }
        } else {
            let complete_halfwords = output_tape.output_size / 2;
            swab_array(&mut output_tape.output_buffer, complete_halfwords);
        }
    }

    //    let mut file: File = raw_fd_to_file(out_des);

    let bytes_written;

    if get_sparse_flag() {
        bytes_written = sparse_write(
            file,
            &output_tape.output_buffer,
            output_tape.output_size,
            flush,
        );
    } else {
        // 这个数据是从缓冲区中读取的，所以需要从缓冲区的有效起始位置开始写到盘，而不是从缓冲区的当前位置开始写盘
        bytes_written = file
            .write(&output_tape.output_buffer[0..output_tape.output_size])
            .unwrap();
    };

    if bytes_written != output_tape.output_size {
        if bytes_written == usize::MAX {
            error(
                PAXEXIT_FAILURE,
                io::Error::last_os_error().raw_os_error().unwrap_or(0),
                format_args!("write error"),
            );
        } else {
            error(
                PAXEXIT_FAILURE,
                0,
                format_args!("write error: partial write"),
            );
        }
    }
    output_tape.out_buff = 0;
    output_tape.output_bytes += output_tape.output_size;
    output_tape.output_size = 0;
}

fn swahw_array(ptr: &mut [u8], count: usize) {
    for i in 0..count {
        let base = i * 4; // 计算每个4字节块的起始位置

        if base + 3 < ptr.len() {
            // 确保不会越界
            ptr.swap(base, base + 2);
            ptr.swap(base + 1, base + 3);
        }
    }
}
pub fn get_next_reel(tape_des: &File) {
    let mut reel_number;
    unsafe {
        reel_number = REEL_NUMBER;
    }

    let mut tty_in = File::open(TTY_NAME)
        .map(BufReader::new)
        .unwrap_or_else(|e| {
            error(
                PAXEXIT_FAILURE,
                e.raw_os_error().unwrap_or(0),
                format_args!("{}", TTY_NAME),
            );
            panic!("Unreachable");
        });

    let mut tty_out = File::create(TTY_NAME).unwrap_or_else(|e| {
        error(
            PAXEXIT_FAILURE,
            e.raw_os_error().unwrap_or(0),
            format_args!("{}", TTY_NAME),
        );
        panic!("Unreachable");
    });

    let old_tape_des = tape_des;
    // tape_offline(tape_des);
    //  rmtclose(tape_des); //自动关闭

    let mut new_tape: File = tape_des
        .try_clone()
        .expect("Failed to clone file descriptor");

    unsafe {
        reel_number += 1;
        REEL_NUMBER = reel_number;
    }

    if let Some(msg) = get_new_media_message() {
        write!(tty_out, "{}", msg).unwrap();
    } else if let (Some(prefix), Some(suffix)) = (
        get_args_new_media_message_with_number(),
        get_new_media_message_after_number(),
    ) {
        write!(tty_out, "{}{}{}", prefix, reel_number, suffix).unwrap();
    } else if let Some(name) = get_archive_name() {
        write!(
            tty_out,
            "Found end of tape {}. Load next tape and press RETURN. ",
            name
        )
        .unwrap();
    } else {
        writeln!(
            tty_out,
            "Found end of tape. To continue, type device/file name when ready."
        )
        .unwrap();
    }

    tty_out.flush().unwrap();

    if let Some(name) = get_archive_name() {
        //        let _line = String::new();
        tty_in.lines().next().unwrap().unwrap();

        let new_tape_des = open_archive(name.as_str());
        match new_tape_des {
            Ok(file) => {
                new_tape = file;
            }
            Err(_e) => {
                open_error(name.as_str());
            }
        }
    } else {
        loop {
            let mut line = String::new();
            tty_in.read_line(&mut line).unwrap();
            let next_archive_name = line.trim();

            let new_tape_des = open_archive(next_archive_name);
            match new_tape_des {
                Ok(file) => {
                    new_tape = file;
                    break; // 成功打开文件后退出循环
                }
                Err(_e) => {
                    write!(tty_out, "To continue, type device/file name when ready.").unwrap();
                    tty_out.flush().unwrap();
                    // 继续循环，等待用户输入正确的文件名
                }
            }
        }
    }

    if new_tape.as_raw_fd() != old_tape_des.as_raw_fd() {
        error(
            PAXEXIT_FAILURE,
            0,
            format_args!(
                "internal error: tape descriptor changed from {} to {}",
                old_tape_des.as_raw_fd(),
                new_tape.as_raw_fd()
            ),
        );
    }
}

fn buf_all_zeros(buf: &[u8], size: usize) -> bool {
    buf.iter().take(size).all(|&x| x == 0)
}

fn sparse_write(fildes: &mut File, buf: &[u8], nbytes: usize, flush: bool) -> usize {
    let mut nwritten = 0;
    let mut start_ptr = 0;
    static mut DELAYED_SEEK_COUNT: i64 = 0;
    let mut seek_count: i64 = 0;
    let mut state = if unsafe { DELAYED_SEEK_COUNT } != 0 {
        State::InZeros
    } else {
        State::Begin
    };
    let mut current_pos = 0;

    enum State {
        Begin,
        InZeros,
        NotInZeros,
    }

    while current_pos < nbytes {
        let rest = nbytes - current_pos;

        if rest < DISKBLOCKSIZE as usize {
            state = State::NotInZeros;
        } else if buf_all_zeros(&buf[current_pos..], rest) {
            if let State::NotInZeros = state {
                let bytes = current_pos - start_ptr + rest;
                if fildes
                    .write_all(&buf[start_ptr..current_pos + rest])
                    .is_err()
                {
                    return 0; // 发生错误，返回 0
                }
                nwritten += bytes;
                start_ptr = current_pos + rest;
            } else {
                seek_count += rest as i64;
            }
            state = State::InZeros;
        } else {
            seek_count += unsafe { DELAYED_SEEK_COUNT };
            if fildes.seek(SeekFrom::Current(seek_count)).is_err() {
                return 0; // 发生错误，返回 0
            }
            unsafe { DELAYED_SEEK_COUNT = 0 };
            seek_count = 0;
            state = State::NotInZeros;
            start_ptr = current_pos;
        }
        current_pos += rest;
    }

    if let State::NotInZeros = state {
        seek_count += unsafe { DELAYED_SEEK_COUNT };
        if seek_count != 0 && fildes.seek(SeekFrom::Current(seek_count)).is_err() {
            return 0; // 发生错误，返回 0
        }
        unsafe { DELAYED_SEEK_COUNT = 0 };
        seek_count = 0;
        if fildes.write_all(&buf[start_ptr..current_pos]).is_err() {
            return 0; // 发生错误，返回 0
        }
        nwritten += current_pos - start_ptr;
    }

    unsafe { DELAYED_SEEK_COUNT += seek_count };

    if flush && unsafe { DELAYED_SEEK_COUNT } != 0 {
        if fildes
            .seek(SeekFrom::Current(unsafe { DELAYED_SEEK_COUNT } - 1))
            .is_err()
        {
            return 0; // 发生错误，返回 0
        }
        if fildes.write_all(&[0]).is_err() {
            return 0; // 发生错误，返回 0
        }
        unsafe { DELAYED_SEEK_COUNT = 0 };
    }

    nwritten + seek_count as usize
}

pub fn open_archive(file: &str) -> io::Result<File> {
    let fd;
    let copy_in: fn() -> io::Result<()> = process_copy_in; // Workaround for pcc bug.

    let copy_func_guard = match COPY_FUNCTION.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return Err(io::Error::new(io::ErrorKind::Other, "Failed to lock COPY_FUNCTION"));
        }
    };

    if *copy_func_guard == Some(copy_in) {
        fd = rmtopen(
            file,
            libc::O_RDONLY,
            MODE_RW,
            get_rsh_command_option().as_deref().unwrap_or(""),
        );
    } else if !get_append_flag() {
        fd = rmtopen(
            file,
            libc::O_WRONLY | libc::O_CREAT | libc::O_TRUNC,
            MODE_RW,
            get_rsh_command_option().as_deref().unwrap_or(""),
        );
    } else {
        fd = rmtopen(
            file,
            libc::O_RDWR,
            MODE_RW,
            get_rsh_command_option().as_deref().unwrap_or(""),
        );
    }

    fd
}
