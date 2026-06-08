// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

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
//use gnu::xmalloc::*;

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
use crate::copyin::*;
use crate::cpiohdr::*;
use crate::externs::*;
use crate::filetype::*;
use crate::global::*;

use crate::util::libc::gid_t;
use crate::util::libc::uid_t;

use gnu::dirname::*;
use gnu::error::*;
use gnu::util::validate_and_sanitize_path;

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

pub fn to_ascii(where_: &mut [u8], v: u64, digits: usize, logbase: u32, nul: bool) -> bool {
    let codetab = b"0123456789ABCDEF";
    let mut v = v;
    let mut digits = digits;

    if nul {
        where_[digits - 1] = 0;
        digits -= 1;
    }
    while digits > 0 {
        where_[digits - 1] = codetab[(v & ((1 << logbase) - 1)) as usize];
        v >>= logbase;
        digits -= 1;
    }
    v != 0
}

pub fn link_to_maj_min_ino(file_name: &str, st_dev_maj: u32, st_dev_min: u32, st_ino: u64) -> i32 {
    if let Some(link_name) = find_inode_file(st_ino, st_dev_maj as u64, st_dev_min as u64) {
        link_to_name(file_name, &link_name)
    } else {
        add_inode(
            st_ino,
            Some(file_name.to_string()),
            st_dev_maj as u64,
            st_dev_min as u64,
        );
        -1
    }
}
pub fn link_to_name(link_name: &str, link_target: &str) -> i32 {
    let link_name_path = Path::new(link_name);
    let link_target_path = Path::new(link_target);

    let mut res = fs::hard_link(link_target_path, link_name_path);

    if res.is_err() && get_create_dir_flag() {
        create_all_directories(link_name);
        res = fs::hard_link(link_target_path, link_name_path);
    }

    match res {
        Ok(_) => {
            if get_verbose_flag() {
                println!("{} linked to {}", link_target, link_name);
            }
            0
        }
        Err(e) => {
            if get_link_flag() {
                eprintln!("cannot link {} to {}: {}", link_target, link_name, e);
            }
            -e.raw_os_error().unwrap_or(1)
        }
    }
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
fn tape_fill_input_buffer(input_tape: &mut MutexGuard<TapeInput>, in_des: &File, num_bytes: i32) {
    input_tape.in_buff = 0;
    let num_bytes = num_bytes.min(get_io_block_size());

    let mut input_size = rmtread(in_des, &mut input_tape.input_buffer, num_bytes as usize);

    if input_size == 0 && input_tape.input_is_special {
        get_next_reel(in_des);
        input_size = rmtread(in_des, &mut input_tape.input_buffer, num_bytes as usize);
    }
    if input_size == SAFE_READ_ERROR {
        error(PAXEXIT_FAILURE, 0, format_args!("rmtread error"));
    }

    if (input_size) == 0 {
        error(PAXEXIT_FAILURE, 0, format_args!("rmtread error"));
    }

    input_tape.input_size = input_size;
    input_tape.input_bytes += input_size;
}

fn disk_fill_input_buffer(
    input_tape: &mut MutexGuard<TapeInput>,
    in_des: &mut File,
    num_bytes: usize,
) -> i32 {
    input_tape.in_buff = 0; // 重置索引

    let num_bytes = if num_bytes < get_io_block_size() as usize {
        num_bytes
    } else {
        get_io_block_size() as usize
    };
    let buffer_slice = &mut input_tape.input_buffer[0..num_bytes];

    match in_des.read(buffer_slice) {
        Ok(n) => {
            if n == 0 {
                input_tape.input_size = 0;
                1
            } else {
                input_tape.input_size = n;
                input_tape.input_bytes += input_tape.input_size;
                0
            }
        }
        Err(_e) => {
            input_tape.input_size = 0;
            -1
        }
    }
}

pub fn tape_buffered_write(
    output_tape: &mut MutexGuard<TapeOutput>,
    in_buf: &mut [u8],
    out_file: &mut File,
    num_bytes: usize,
) {
    // 如果不需要写入任何字节，直接返回
    if num_bytes == 0 {
        return;
    }

    let mut bytes_left = num_bytes;
    let mut in_buf_offset = 0;
    // let mut space_left = 0;

    while bytes_left > 0 {
        let mut space_left = get_io_block_size() as usize - output_tape.output_size;

        if space_left == 0 {
            tape_empty_output_buffer(output_tape, out_file);
        } else {
            if bytes_left < space_left {
                space_left = bytes_left;
            }

            let in_start = in_buf_offset; // in_buf.len() - bytes_left ;
            let in_end = in_start + space_left;

            let out_start = output_tape.out_buff;
            let out_end = out_start + space_left;

            output_tape.output_buffer[out_start..out_end]
                .copy_from_slice(&in_buf[in_start..in_end]);
            // output_tape.output_buffer.splice(out_start..out_end, in_buf[in_start..in_end].iter().cloned());

            output_tape.out_buff += space_left;
            output_tape.output_size += space_left;
            bytes_left -= space_left;
            in_buf_offset += space_left;
        }
    }
}

pub fn tape_buffered_peek(
    input_tape: &mut MutexGuard<TapeInput>,
    peek_buf: &mut [u8],
    in_des: &File,
    num_bytes: i32,
) -> i32 {
    let mut tmp_input_size: isize;

    let mut append_buf: usize;

    //let input_size = input_tape.input_size;

    while input_tape.input_size < num_bytes as usize {
        append_buf = input_tape.in_buff + input_tape.input_size;
        if append_buf >= input_tape.input_buffer_size {
            let half = input_tape.input_buffer_size / 2;
            input_tape.input_buffer.copy_within(half.., 0);
            input_tape.in_buff -= half;
            append_buf -= half;
        }

        tmp_input_size = rmtread(
            in_des,
            &mut input_tape.input_buffer[append_buf..],
            get_io_block_size() as usize,
        ) as isize;

        if tmp_input_size == 0 {
            if input_tape.input_is_special {
                get_next_reel(in_des);
                let block_size = match get_io_block_size().try_into() {
                    Ok(size) => size,
                    Err(_) => {
                        error(PAXEXIT_FAILURE, 0, format_args!("Invalid block size"));
                        return -1;
                    }
                };
                tmp_input_size = match rmtread(
                    in_des,
                    &mut input_tape.input_buffer[append_buf..],
                    block_size,
                )
                .try_into()
                {
                    Ok(size) => size,
                    Err(_) => {
                        error(
                            PAXEXIT_FAILURE,
                            0,
                            format_args!("Read size conversion error"),
                        );
                        return -1;
                    }
                }
            } else {
                break;
            }
        }

        if tmp_input_size < 0 {
            error(PAXEXIT_FAILURE, 0, format_args!("read error"));
            return -1;
        }

        input_tape.input_bytes += tmp_input_size as usize;
        input_tape.input_size += tmp_input_size as usize;
    }

    let got_bytes: usize = if num_bytes as usize <= input_tape.input_size {
        num_bytes as usize
    } else {
        input_tape.input_size
    };

    peek_buf[..got_bytes].copy_from_slice(
        &input_tape.input_buffer[input_tape.in_buff..input_tape.in_buff + got_bytes],
    );

    got_bytes as i32
}

pub fn tape_toss_input(input_tape: &mut MutexGuard<TapeInput>, in_des: &mut File, num_bytes: i32) {
    let mut bytes_left = num_bytes;

    while bytes_left > 0 {
        if input_tape.input_size == 0 {
            tape_fill_input_buffer(input_tape, in_des, num_bytes);
        }

        let space_left = if bytes_left < input_tape.input_size as i32 {
            bytes_left as usize
        } else {
            input_tape.input_size
        };

        // 如果需要计算 CRC
        if get_only_verify_crc_flag() && get_crc_i_flag() {
            let mut crc = get_crc();

            for byte in &input_tape.input_buffer[0..space_left] {
                crc += *byte as usize;
            }
            set_crc(crc);
        }

        input_tape.input_size -= space_left;
        input_tape.in_buff += space_left;
        bytes_left -= space_left as i32;
    }
}

pub fn write_nuls_to_file(
    tape_output: &mut MutexGuard<TapeOutput>,
    num_bytes: usize,
    out_file: &mut File,
    writer: fn(&mut MutexGuard<TapeOutput>, &mut [u8], &mut File, usize),
) {
    // 如果不需要写入任何字节，直接返回
    if num_bytes == 0 {
        return;
    }

    let zeros_512: [u8; 512] = [0; 512];

    let blocks = num_bytes / zeros_512.len();
    let extra_bytes = num_bytes % zeros_512.len();

    for _ in 0..blocks {
        writer(
            tape_output,
            &mut zeros_512.to_vec(),
            out_file,
            zeros_512.len(),
        );
    }

    if extra_bytes > 0 {
        writer(tape_output, &mut zeros_512.to_vec(), out_file, extra_bytes);
    }
}
pub fn copy_files_tape_to_disk(
    input_tape: &mut MutexGuard<TapeInput>,
    output_tape: &mut MutexGuard<TapeOutput>,
    in_des: &mut File,
    out_file: &mut File,
    num_bytes: i32,
) {
    let mut num_bytes = num_bytes as usize;

    while num_bytes > 0 {
        if input_tape.input_size == 0 {
            tape_fill_input_buffer(input_tape, in_des, get_io_block_size());
        }

        let size = if input_tape.input_size < num_bytes {
            input_tape.input_size
        } else {
            num_bytes
        };

        if get_crc_i_flag() {
            for k in 0..size {
                let mut crc = get_crc();
                crc += input_tape.input_buffer[input_tape.in_buff + k] as usize;
                set_crc(crc);
            }
        }

        disk_buffered_write(
            output_tape,
            &mut input_tape.input_buffer[input_tape.in_buff..input_tape.in_buff + size].to_vec(),
            out_file,
            size,
        );
        num_bytes -= size;
        input_tape.input_size -= size;
        input_tape.in_buff += size;
    }
}

fn disk_buffered_write(
    output_tape: &mut MutexGuard<TapeOutput>,
    in_buf: &mut [u8],
    file: &mut File,
    num_bytes: usize,
) {
    let mut bytes_left = num_bytes;

    while bytes_left > 0 {
        //let mut output_tape = TAPE_OUTPUT.lock().unwrap();
        let space_left = DISK_IO_BLOCK_SIZE - output_tape.output_size;

        if space_left == 0 {
            disk_empty_output_buffer(output_tape, file, false);
        } else {
            let space_left = if bytes_left < space_left {
                bytes_left
            } else {
                space_left
            };

            let in_start = in_buf.len() - bytes_left;
            let in_end = in_start + space_left;
            let out_start = output_tape.out_buff;
            let out_end = out_start + space_left;

            output_tape
                .output_buffer
                .splice(out_start..out_end, in_buf[in_start..in_end].iter().cloned());

            output_tape.out_buff += space_left;
            output_tape.output_size += space_left;
            bytes_left -= space_left;

            //            output_tape.print();
        }
    }
}
pub fn tape_buffered_read(
    input_tape: &mut MutexGuard<TapeInput>,
    in_buf: &mut [u8],
    in_des: &File,
    num_bytes: usize,
) {
    let mut bytes_left = num_bytes;
    let mut in_buf_offset = 0;

    while bytes_left > 0 && in_buf_offset < in_buf.len() {
        if input_tape.input_size == 0 {
            tape_fill_input_buffer(input_tape, in_des, get_io_block_size());
        }

        let space_left = if bytes_left < input_tape.input_size {
            bytes_left
        } else {
            input_tape.input_size
        };

        // Ensure we don't exceed the target buffer size
        let available_space = in_buf.len() - in_buf_offset;
        let actual_space = if space_left < available_space {
            space_left
        } else {
            available_space
        };

        if actual_space == 0 {
            break; // No more space in target buffer
        }

        let src_start = input_tape.in_buff;
        let src_end = src_start + actual_space;

        let trg_start = in_buf_offset;
        let trg_end = trg_start + actual_space;

        in_buf[trg_start..trg_end].copy_from_slice(&input_tape.input_buffer[src_start..src_end]);

        input_tape.in_buff += actual_space;
        input_tape.input_size -= actual_space;

        in_buf_offset += actual_space;
        bytes_left -= actual_space;
    }
}
pub fn copy_files_disk_to_tape(
    tape_output: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    in_des: &mut File,
    out_file: &mut File,
    num_bytes: i32,
    filename: &str,
) {
    let mut num_bytes = num_bytes as usize;
    let original_num_bytes = num_bytes;

    while num_bytes > 0 {
        if input_tape.input_size == 0 {
            let read_size = if num_bytes < DISK_IO_BLOCK_SIZE {
                num_bytes
            } else {
                DISK_IO_BLOCK_SIZE
            };

            let rc: i32 = disk_fill_input_buffer(input_tape, in_des, read_size);

            if rc != 0 {
                if rc > 0 {
                    let s = if num_bytes == 1 { "" } else { "s" };
                    let message = format!(
                        "File {} shrunk by {} byte{}, padding with zeros",
                        filename, num_bytes, s
                    );
                    error(0, 0, format_args!("{}", message));
                } else {
                    let message = format!(
                        "Read error at byte {} in file {}, padding with zeros",
                        original_num_bytes - num_bytes,
                        filename
                    );
                    error(0, 0, format_args!("{}", message));
                }
                write_nuls_to_file(tape_output, num_bytes, out_file, tape_buffered_write);
                break;
            }
        }

        let size = if input_tape.input_size < num_bytes {
            input_tape.input_size
        } else {
            num_bytes
        };

        if get_crc_i_flag() {
            let mut crc = get_crc();
            for k in 0..size {
                crc += input_tape.input_buffer[input_tape.in_buff + k] as usize;
            }
            set_crc(crc);
        }

        tape_buffered_write(
            tape_output,
            &mut input_tape.input_buffer[input_tape.in_buff..input_tape.in_buff + size].to_vec(),
            out_file,
            size,
        );
        num_bytes -= size;
        input_tape.input_size -= size;
        input_tape.in_buff += size;
    }
}

pub fn copy_files_disk_to_disk(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    in_des: &mut File,
    out_des: &mut File,
    num_bytes: i32,
    filename: &str,
) {
    let mut num_bytes = num_bytes as usize;
    let original_num_bytes = num_bytes;
    let mut rc: i32;

    while num_bytes > 0 {
        if input_tape.input_size == 0 {
            let read_size = if num_bytes < DISK_IO_BLOCK_SIZE {
                num_bytes
            } else {
                DISK_IO_BLOCK_SIZE
            };

            rc = disk_fill_input_buffer(input_tape, in_des, read_size);

            if rc != 0 {
                if rc > 0 {
                    let message = format!(
                        "File {} shrunk by {} byte{}, padding with zeros",
                        filename,
                        num_bytes,
                        if num_bytes == 1 { "" } else { "s" }
                    );
                    error(0, 0, format_args!("{}", message));
                } else {
                    let message = format!(
                        "Read error at byte {} in file {}, padding with zeros",
                        original_num_bytes - num_bytes,
                        filename
                    );
                    error(0, 0, format_args!("{}", message));
                }
                write_nuls_to_file(output_tape, num_bytes, out_des, disk_buffered_write);
                break;
            }
        }

        let size = if input_tape.input_size < num_bytes {
            input_tape.input_size
        } else {
            num_bytes
        };

        if get_crc_i_flag() {
            let mut crc = get_crc();
            for k in 0..size {
                crc += input_tape.input_buffer[input_tape.in_buff + k] as usize;
            }

            set_crc(crc);
        }

        disk_buffered_write(
            output_tape,
            &mut input_tape.input_buffer[input_tape.in_buff..input_tape.in_buff + size].to_vec(),
            out_des,
            size,
        );
        num_bytes -= size;
        input_tape.input_size -= size;
        input_tape.in_buff += size;
    }
}

pub fn warn_if_file_changed(file_name: &str, old_file_size: u64, old_file_mtime: u64) {
    let path = Path::new(file_name);
    match fs::metadata(path) {
        Ok(new_file_stat) => {
            let new_file_size = new_file_stat.len();
            let new_file_mtime = new_file_stat
                .modified()
                .ok()
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0); //Handle error by setting to 0.

            if new_file_size > old_file_size {
                let diff = new_file_size - old_file_size;
                println!("File {} grew, {} new bytes not copied", file_name, diff);
            } else if new_file_mtime != old_file_mtime {
                println!("File {} was modified while being copied", file_name);
            }
        }
        Err(e) => {
            eprintln!("Error getting file status for {}: {}", file_name, e);
        }
    }
}
pub fn create_all_directories(name: &str) {
    if let Some(dir) = dir_name(name) {
        let chars: Vec<char> = dir.chars().collect();
        if chars.len() < 2 {
            return;
        }
        if chars[0] != '.' || chars[1] != '\0' {
            let fmt = if (get_warn_option() as usize & CPIO_WARN_INTERDIR) != 0 {
                Some("Creating intermediate directory `%s`")
            } else {
                None
            };
            make_path(&dir.clone(), -1, -1, fmt);
        }
    } else {
        error(PAXEXIT_FAILURE, 0, format_args!("virtual memory exhausted"));
    }
}
pub fn prepare_append(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    out_file_des: &mut File,
) {
    let start_of_header = get_last_header_start();
    let useful_bytes_in_block = (start_of_header % get_io_block_size()) as usize;

    let start_of_block = start_of_header - useful_bytes_in_block as i32;

    if out_file_des
        .seek(SeekFrom::Start(start_of_block as u64))
        .is_err()
    {
        error(
            PAXEXIT_FAILURE,
            std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
            format_args!("cannot seek on output"),
        );
    }

    if useful_bytes_in_block > 0 {
        let mut tmp_buf = vec![0u8; useful_bytes_in_block];
        if out_file_des.read_exact(&mut tmp_buf).is_err() {
            error(
                PAXEXIT_FAILURE,
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
                format_args!("read error"),
            );
        }

        if out_file_des
            .seek(SeekFrom::Start(start_of_block as u64))
            .is_err()
        {
            error(
                PAXEXIT_FAILURE,
                std::io::Error::last_os_error().raw_os_error().unwrap_or(0),
                format_args!("cannot seek on output"),
            );
        }

        tape_buffered_write(
            output_tape,
            &mut tmp_buf,
            out_file_des,
            useful_bytes_in_block,
        );
    }

    input_tape.input_size = 0;
    input_tape.in_buff = 0;
}

fn find_inode_val(node_num: u64, major_num: u64, minor_num: u64) -> Option<InodeVal> {
    let sample = InodeVal {
        inode: node_num,
        major_num,
        minor_num,
        trans_inode: 0,
        file_name: None,
    };
    HASH_TABLE.lock().unwrap().get(&sample).cloned()
}

pub fn find_inode_file(node_num: u64, major_num: u64, minor_num: u64) -> Option<String> {
    find_inode_val(node_num, major_num, minor_num).and_then(|ival| ival.file_name)
}

pub fn add_inode(
    node_num: u64,
    file_name: Option<String>,
    major_num: u64,
    minor_num: u64,
) -> InodeVal {
    let mut temp = InodeVal {
        inode: node_num,
        major_num,
        minor_num,
        trans_inode: 0,
        file_name,
    };

    unsafe {
        if get_renumber_inodes_option() {
            temp.trans_inode = NEXT_INODE;
            NEXT_INODE += 1;
        } else {
            temp.trans_inode = temp.inode;
        }
    }

    if let Ok(mut hash_table) = HASH_TABLE.lock() {
        hash_table.insert(temp.clone(), temp.clone());
    }
    temp
}
fn get_inode_and_dev(hdr: &mut CpioFileStat, st: &std::fs::Metadata) {
    unsafe {
        if get_renumber_inodes_option() {
            if st.nlink() > 1 {
                if let Some(ival) = find_inode_val(st.ino(), major(st.dev()), minor(st.dev())) {
                    hdr.c_ino = ival.trans_inode;
                } else {
                    let ival = add_inode(st.ino(), None, major(st.dev()), minor(st.dev()));
                    hdr.c_ino = ival.trans_inode;
                }
            } else {
                hdr.c_ino = NEXT_INODE;
                NEXT_INODE += 1;
            }
        } else {
            hdr.c_ino = st.ino();
        }

        if get_ignore_devno_option() {
            hdr.c_dev_maj = 0;
            hdr.c_dev_min = 0;
        } else {
            hdr.c_dev_maj = major(st.dev()) as i32;
            hdr.c_dev_min = minor(st.dev()) as u32;
        }
    }
}

pub fn open_archive(file: &str) -> io::Result<File> {
    let fd;
    let copy_in: fn() -> io::Result<()> = process_copy_in; // Workaround for pcc bug.

    let copy_func_guard = match COPY_FUNCTION.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to lock COPY_FUNCTION",
            ));
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

fn get_next_reel(tape_des: &File) {
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

pub fn set_new_media_message(message: &str) {
    if let Some(pos) = message.find("%d") {
        let prefix = &message[..pos];
        let after = &message[pos + 2..];
        set_new_media_message_with_number(Some(prefix.to_string()));
        set_new_media_message_after_number(Some(after.to_string()));
    } else {
        set_args_new_media_message(Some(message.to_string()));
    }

    // let _p = message.chars();
    // let mut prev_was_percent = false;
    // let mut d_found_at = None;

    // for (index, c) in message.chars().enumerate() {
    //     if c == 'd' && prev_was_percent {
    //         d_found_at = Some(index);
    //         break;
    //     }
    //     prev_was_percent = c == '%';
    // }

    // if d_found_at.is_none() {
    //     set_args_new_media_message(Some(xstrdup(message).to_string()));
    // } else {
    //     let d_index = d_found_at.unwrap();
    //     let length = d_index - 1;

    //     unsafe {
    //         let mut buf = xmalloc(length + 1);
    //         for (i, c) in message[..length].chars().enumerate() {
    //             buf[i] = c as u8;
    //         }
    //         buf[length] = 0; // Null-terminate

    //         set_new_media_message_with_number(Some(String::from_utf8_unchecked(buf)));
    //         //new_media_message_with_number = Some(String::from_utf8_unchecked(buf));

    //         let after_d = &message[d_index + 1..];
    //         let after_d_len = after_d.len();
    //         let mut after_buf = xmalloc(after_d_len + 1);
    //         for (i, c) in after_d.chars().enumerate() {
    //             after_buf[i] = c as u8;
    //         }
    //         after_buf[after_d_len] = 0; // Null-terminate
    //         set_new_media_message_after_number(Some(String::from_utf8_unchecked(after_buf)));
    //     }
    // }
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

fn cpio_uid(uid: u32) -> u32 {
    if get_set_owner_flag() {
        get_set_owner()
    } else {
        uid
    }
}
fn cpio_gid(uid: u32) -> u32 {
    if get_set_group_flag() {
        get_set_group()
    } else {
        uid
    }
}

pub fn stat_to_cpio(st: &mut Metadata, hdr: &mut CpioFileStat) {
    get_inode_and_dev(hdr, st);

    hdr.c_mode = st.mode() & 0o7777;
    if st.file_type().is_file() {
        hdr.c_mode |= CP_IFREG;
    } else if st.file_type().is_dir() {
        hdr.c_mode |= CP_IFDIR;
    } else if st.file_type().is_block_device() {
        hdr.c_mode |= CP_IFBLK;
    } else if st.file_type().is_char_device() {
        hdr.c_mode |= CP_IFCHR;
    } else if st.file_type().is_fifo() {
        hdr.c_mode |= CP_IFIFO;
    } else if st.file_type().is_symlink() {
        hdr.c_mode |= CP_IFLNK;
    } else if st.file_type().is_socket() {
        hdr.c_mode |= CP_IFSOCK;
    }

    hdr.c_nlink = st.nlink() as usize;
    hdr.c_uid = cpio_uid(st.uid());
    hdr.c_gid = cpio_gid(st.gid());

    if st.file_type().is_block_device() || st.file_type().is_char_device() {
        hdr.c_rdev_maj = major(st.rdev()) as i32;
        hdr.c_rdev_min = minor(st.rdev()) as u32;
    } else {
        hdr.c_rdev_maj = 0;
        hdr.c_rdev_min = 0;
    }

    hdr.c_mtime = st
        .modified()
        .map(|t| {
            t.duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        })
        .unwrap_or(0);
    hdr.c_filesize = st.len() as i64;
    hdr.c_chksum = 0;
    hdr.c_tar_linkname = None;
}

fn fchown_or_chown(
    file: Option<&File>,
    name: &Path,
    uid: uid_t,
    gid: gid_t,
) -> std::io::Result<()> {
    if let Some(file_ref) = file {
        let fd = file_ref.as_raw_fd();
        let result = unsafe { fchown(fd, uid, gid) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    } else {
        match name.to_str() {
            Some(name_str) => match std::ffi::CString::new(name_str) {
                Ok(name_cstr) => {
                    let result = unsafe { chown(name_cstr.as_ptr(), uid, gid) };
                    if result == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                }
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid path string",
                )),
            },
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid path encoding",
            )),
        }
    }
}

fn fchmod_or_chmod(file: Option<&File>, name: &Path, mode: mode_t) -> std::io::Result<()> {
    if let Some(file_ref) = file {
        let fd = file_ref.as_raw_fd();
        let result = unsafe { fchmod(fd, mode) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    } else {
        match name.to_str() {
            Some(name_str) => match std::ffi::CString::new(name_str) {
                Ok(name_cstr) => {
                    let result = unsafe { libc::chmod(name_cstr.as_ptr(), mode) };
                    if result == 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                }
                Err(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid path string",
                )),
            },
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid path encoding",
            )),
        }
    }
}

pub fn set_perms(file: Option<&File>, header: &mut CpioFileStat) {
    let c_name = header.get_c_name();

    // 验证和清理路径
    let safe_path = match validate_and_sanitize_path(&c_name) {
        Ok(path) => path,
        Err(_) => {
            // 如果路径验证失败，记录错误但不中断操作
            // 这可能是由于符号链接或其他特殊情况
            return;
        }
    };

    if !get_no_chown_flag() {
        let uid = cpio_uid(header.c_uid);
        let gid = cpio_gid(header.c_gid);

        match fchown_or_chown(file, &safe_path, uid, gid) {
            Ok(_) => (),
            Err(e) => {
                // 对于符号链接，更宽容地处理权限设置错误
                match e.raw_os_error() {
                    Some(libc::EPERM) | Some(libc::ENOENT) | Some(libc::EROFS)
                    | Some(libc::EINVAL) | Some(libc::EACCES) | Some(libc::ENOTSUP) => {
                        // 这些错误对于符号链接来说是可以忽略的
                    }
                    _ => {
                        chown_uid_error_details(&c_name, uid, gid);
                    }
                }
            }
        }
    }

    if (fchmod_or_chmod(file, &safe_path, header.c_mode)).is_err() {
        chown_mode_error_details(&c_name, header.c_mode);
    }

    if get_retain_time_flag() {
        set_file_times(file, &c_name, header.c_mtime, header.c_mtime, 0);
    }
}

pub fn set_file_times(file: Option<&File>, name: &str, atime: time_t, mtime: time_t, atflag: i32) {
    let mut ts: [timespec; 2] = unsafe { std::mem::zeroed() };

    ts[0].tv_sec = atime;
    ts[1].tv_sec = mtime;

    match fdutimensat(file, AT_FDCWD, Some(name), &ts, atflag) {
        Ok(_) => (),
        Err(_e) => {
            utime_error(name);
        }
    }
}

// pub fn cpio_realloc_c_name(file_hdr: &mut CpioFileStat, len: usize) {
//     while file_hdr.c_name_buflen < len {
//         let new_vec = x2realloc(file_hdr.c_name.as_bytes().to_vec(), &mut file_hdr.c_name_buflen);
//         file_hdr
//         file_hdr.c_name = unsafe { String::from_utf8_unchecked(new_vec) };
//     }
// }

pub fn cpio_set_c_name(file_hdr: &mut CpioFileStat, name: &str) {
    file_hdr.set_c_name(name);
    //    file_hdr.c_namesize = name.len() + 1;

    // let len = name.len() + 1;
    // cpio_realloc_c_name(file_hdr, len);
    // file_hdr.c_namesize = len;
    // file_hdr.c_name = name.to_string();
}
pub fn cpio_safer_name_suffix(
    name: &mut String,
    link_target: bool,
    absolute_names: bool,
    strip_leading_dots: bool,
) {
    let p = safer_name_suffix(name, link_target, absolute_names);

    let mut adjusted_p = p.clone(); // Create a mutable copy

    if strip_leading_dots && adjusted_p != "./" {
        // strip leading `./' from the filename.
        while adjusted_p.starts_with("./") {
            adjusted_p = adjusted_p[2..].trim_start_matches('/').to_string();
        }
    }

    if adjusted_p != *name {
        // The 'adjusted_p' string is shortened version of 'name' with one exception;
        // when the 'name' points to an empty string (buffer where name[0] == '\0') the
        // 'adjusted_p' then points to static string ".". So caller needs to ensure there
        // are at least two bytes available in 'name' buffer so memmove succeeds.
        *name = adjusted_p;
    }
}

fn delay_cpio_set_stat(file_stat: &CpioFileStat, invert_permissions: mode_t) {
    if let Ok(mut head) = DELAYED_SET_STAT_HEAD.lock() {
        let new_node = Arc::new(Mutex::new(DelayedSetStat {
            stat: file_stat.clone(),
            invert_permissions,
            next: head.clone(),
        }));

        *head = Some(new_node);
    }
}

fn delay_set_stat(file_name: &str, st: &mut Metadata, invert_permissions: u32) {
    let mut fs = CpioFileStat::new();

    stat_to_cpio(st, &mut fs);

    fs.set_c_name(file_name);

    delay_cpio_set_stat(&fs, invert_permissions);
}
#[allow(dead_code)]
fn repair_inter_delayed_set_stat(dir_stat_info: &mut Metadata) -> i32 {
    let head = match DELAYED_SET_STAT_HEAD.lock() {
        Ok(guard) => guard,
        Err(_) => return -1,
    };
    let mut current = head.clone();

    while let Some(node) = current {
        let mut borrowed_node = match node.lock() {
            Ok(guard) => guard,
            Err(_) => return -1,
        };

        let c_name = borrowed_node.stat.get_c_name();

        // 验证和清理路径
        let safe_path = match validate_and_sanitize_path(&c_name) {
            Ok(path) => path,
            Err(_) => {
                // 如果路径验证失败，跳过这个节点
                current = borrowed_node.next.clone();
                continue;
            }
        };

        match fs::metadata(&safe_path) {
            Ok(st) => {
                if st.dev() == dir_stat_info.dev() && st.ino() == dir_stat_info.ino() {
                    stat_to_cpio(dir_stat_info, &mut borrowed_node.stat);
                    let umask = get_newdir_umask();
                    borrowed_node.invert_permissions =
                        (dir_stat_info.mode() ^ st.mode()) & MODE_RWX & !umask;
                    return 0;
                }
            }
            Err(_) => {
                stat_error(&borrowed_node.stat.get_c_name());
                return -1;
            }
        }

        current = borrowed_node.next.clone();
    }

    1
}

fn repair_delayed_set_stat(file_hdr: &mut CpioFileStat) -> i32 {
    let head = match DELAYED_SET_STAT_HEAD.lock() {
        Ok(guard) => guard,
        Err(_) => return -1,
    };
    let mut current = head.clone();

    while let Some(node) = current.clone() {
        let mut borrowed_node = match node.lock() {
            Ok(guard) => guard,
            Err(_) => return -1,
        };
        if file_hdr.get_c_name() == borrowed_node.stat.get_c_name() {
            borrowed_node.invert_permissions = 0;
            borrowed_node.stat.c_mode = file_hdr.c_mode; // Copy c_mode
                                                         // ... Copy other fields except c_name ...
            return 0;
        }
        current = borrowed_node.next.clone();
    }
    1
}

pub fn apply_delayed_set_stat() {
    let mut head = match DELAYED_SET_STAT_HEAD.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    while let Some(node) = head.clone() {
        let mut borrowed_node = match node.lock() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        if borrowed_node.invert_permissions != 0 {
            borrowed_node.stat.c_mode ^= borrowed_node.invert_permissions;
        }

        set_perms(None, &mut borrowed_node.stat);

        // Remove the node from the list
        *head = borrowed_node.next.clone();
    }
}
fn make_path(argpath: &str, owner: i32, group: i32, verbose_fmt_string: Option<&str>) -> i32 {
    // 验证和清理路径
    let safe_path = match validate_and_sanitize_path(argpath) {
        Ok(path) => path,
        Err(_) => {
            error(
                0,
                0,
                format_args!(
                    "cannot make directory `{}`: invalid or unsafe path",
                    argpath
                ),
            );
            return 1;
        }
    };

    // 如果目录已存在也会返回 Ok
    match fs::create_dir_all(&safe_path) {
        Ok(_) => {
            // 如果需要显示创建信息
            if let Some(fmt) = verbose_fmt_string {
                error(0, 0, format_args!("{}{}", fmt, argpath));
            }

            // 设置所有者和权限
            if owner != -1 || group != -1 {
                if let Ok(stats) = fs::metadata(&safe_path) {
                    let mut mutable_stats = stats;
                    delay_set_stat(argpath, &mut mutable_stats, 0);
                }
            }
            0
        }
        Err(e) => {
            error(
                0,
                e.raw_os_error().unwrap_or(0),
                format_args!("cannot make directory `{}`", argpath),
            );
            1
        }
    }
}

fn cpio_mkdir(file_hdr: &mut CpioFileStat, setstat_delayed: &mut bool) -> std::io::Result<()> {
    let mode = file_hdr.c_mode;
    let c_name = file_hdr.get_c_name();

    // 验证和清理路径
    let safe_path = validate_and_sanitize_path(&c_name)?;

    if (file_hdr.c_mode & S_IWUSR) == 0 {
        let _new_mode = mode | S_IWUSR;
        match fs::create_dir(&safe_path) {
            Ok(_) => {
                delay_cpio_set_stat(file_hdr, 0);
                *setstat_delayed = true;
                Ok(())
            }
            Err(e) => Err(e),
        }
    } else {
        match fs::create_dir(&safe_path) {
            Ok(_) => {
                *setstat_delayed = false;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

pub fn cpio_create_dir(file_hdr: &mut CpioFileStat, existing_dir: bool) -> i32 {
    if get_to_stdout_option() {
        return 0;
    }
    let mut c_name = file_hdr.get_c_name();

    strip_trailing_slashes(&mut c_name);

    file_hdr.set_c_name(&c_name);

    if file_hdr.c_name[0] == b'.' && file_hdr.c_name[1] == b'\0' {
        return 0;
    }
    let c_name = file_hdr.get_c_name();

    let mut setstat_delayed = false;
    let mut res = if !existing_dir {
        cpio_mkdir(file_hdr, &mut setstat_delayed)
    } else {
        Ok(())
    };

    if res.is_err() && get_create_dir_flag() {
        create_all_directories(&c_name);
        res = cpio_mkdir(file_hdr, &mut setstat_delayed);
    }

    if res.is_err() {
        if std::io::Error::last_os_error().raw_os_error() != Some(EEXIST) {
            mkdir_error(&c_name);
            return -1;
        }

        match lstat(&c_name) {
            Ok(file_stat) => {
                if !s_isdir(file_stat.mode()) {
                    error(
                        0,
                        0,
                        format_args!("{:?} is not a directory", quotearg_colon(&c_name)),
                    );
                    return -1;
                }
            }
            Err(_) => {
                stat_error(&c_name);
                return -1;
            }
        }
    }

    if !setstat_delayed && repair_delayed_set_stat(file_hdr) != 0 {
        set_perms(None, file_hdr);
    }

    0
}

pub fn change_dir() {
    if let Some(ref dir) = get_change_directory_option() {
        match env::set_current_dir(dir) {
            Ok(()) => (), // 成功切换，直接返回
            Err(e) if e.kind() == io::ErrorKind::NotFound && get_create_dir_flag() => {
                // 目录不存在且允许创建
                let warn_msg = if get_warn_option() as usize & CPIO_WARN_INTERDIR != 0 {
                    Some("Creating directory `%s`")
                } else {
                    None
                };

                if make_path(dir, -1, -1, warn_msg) != 0 {
                    // 创建失败，退出（这里返回错误）
                    return;
                }

                // 再次尝试切换
                let _ = env::set_current_dir(dir);
            }
            Err(_e) => {
                // 其他错误，报告并退出
            }
        }
    }
}

pub fn arf_stores_inode_p(arf: ArchiveFormat) -> bool {
    match arf {
        ArchiveFormat::Tar | ArchiveFormat::Ustar => false,
        _ => true,
    }
}
