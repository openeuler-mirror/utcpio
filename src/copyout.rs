/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

 #![allow(
    clippy::redundant_closure,
    unused_assignments,
    clippy::unnecessary_mut_passed,
    clippy::useless_format
)]

use std::ffi::{CStr, CString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::sync::MutexGuard;
use std::{ptr, slice};

use libc::mkstemp;
use nix::libc::{self, dev_t};

use pax::paxerror::*;
use pax::rmt::isrmt;

use crate::appargs::*;
use crate::copyin::process_copy_in;
use crate::cpiohdr::*;
use crate::dstring::*;
use crate::externs::*;
use crate::filetype::*;
use crate::filetype::{CP_IFBLK, CP_IFCHR, CP_IFIFO, CP_IFMT, CP_IFSOCK};
use crate::global::*;
use crate::initramfs::*;
// use crate::tar::*;
use crate::util::*;

use gnu::error::*;
use gnu::umaxtostr::*;

fn read_for_checksum(in_file_des: &mut File, file_size: u64, file_name: &str) -> u32 {
    let mut crc = 0;
    let mut buf = [0u8; 1024];
    let mut remaining = file_size;

    while remaining > 0 {
        let bytes_read = in_file_des.read(&mut buf);
        match bytes_read {
            Ok(bytes_read) => {
                for &byte in &buf[..bytes_read] {
                    crc += byte as u32;
                }
                remaining -= bytes_read as u64;
            }
            Err(e) => {
                eprintln!("Error reading file {}: {}", file_name, e);
                return 0;
            }
        }
    }
    match in_file_des.seek(io::SeekFrom::Start(0)) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error seeking in file {}: {}", file_name, e);
            return 0;
        }
    }

    crc
}

fn tape_clear_rest_of_block(output_tape: &mut MutexGuard<TapeOutput>, out_file_des: &mut File) {
    let num_bytes = get_io_block_size() as usize - output_tape.output_size;

    // 如果缓冲区已经满了，不需要填充
    if num_bytes == 0 {
        return;
    }

    write_nuls_to_file(output_tape, num_bytes, out_file_des, tape_buffered_write);
}

fn tape_pad_output(output_tape: &mut MutexGuard<TapeOutput>, out_file_des: &mut File, offset: u64) {
    let pad = match get_archive_format() {
        ArchiveFormat::Newascii | ArchiveFormat::Crcascii => (4 - (offset % 4)) % 4,
        ArchiveFormat::Tar | ArchiveFormat::Ustar => (512 - (offset % 512)) % 512,
        _ => (2 - (offset % 2)) % 2,
    };
    if pad != 0 {
        write_nuls_to_file(output_tape, pad as usize, out_file_des, tape_buffered_write);
    }
}

pub fn count_defered_links_to_dev_ino(file_hdr: &CpioFileStat) -> usize {
    let global_deferments = GLOBAL_DEFERMENTS.lock().unwrap();
    count_defered_links_to_dev_ino_with_lock(file_hdr, &global_deferments)
}

pub fn count_defered_links_to_dev_ino_with_lock(
    file_hdr: &CpioFileStat,
    global_deferments: &std::sync::MutexGuard<Vec<Deferment>>,
) -> usize {
    let mut count = 0;

    for deferment in global_deferments.iter() {
        if deferment.header.c_ino == file_hdr.c_ino
            && deferment.header.c_dev_maj == file_hdr.c_dev_maj
            && deferment.header.c_dev_min == file_hdr.c_dev_min
        {
            count += 1;
        }
    }

    count
}

fn last_link(file_hdr: &CpioFileStat) -> bool {
    file_hdr.c_nlink == count_defered_links_to_dev_ino(file_hdr) + 1
}
pub fn add_link_defer(file_hdr: &CpioFileStat) {
    let mut global_deferments = GLOBAL_DEFERMENTS.lock().unwrap();
    let deferment = Deferment::new(file_hdr);

    global_deferments.insert(0, deferment);
}

pub fn writeout_other_defers(
    output_tape: &mut MutexGuard<TapeOutput>,
    file_hdr: &CpioFileStat,
    out_des: &mut File,
) {
    let ino = file_hdr.c_ino;
    let maj = file_hdr.c_dev_maj;
    let min = file_hdr.c_dev_min;

    let mut deferments = GLOBAL_DEFERMENTS.lock().unwrap();
    let mut prev_index: Option<usize> = None;
    let mut i = 0;

    while i < deferments.len() {
        let current_deferment = deferments[i].clone();

        if current_deferment.header.c_ino == ino
            && current_deferment.header.c_dev_maj == maj
            && current_deferment.header.c_dev_min == min
        {
            let mut removed_deferment = deferments.remove(i);
            removed_deferment.header.c_filesize = 0;
            write_out_header(output_tape, &mut removed_deferment.header, out_des);
            // free_deferment(removed_deferment);
            if let Some(prev_idx) = prev_index {
                if let Some(next_idx) = deferments.get(prev_idx).and_then(|d| d.next_index) {
                    if next_idx > i {
                        deferments.get_mut(prev_idx).unwrap().next_index = Some(next_idx - 1);
                    }
                }
            }
        } else {
            if let Some(prev_idx) = prev_index {
                deferments.get_mut(prev_idx).unwrap().next_index = Some(i);
            }
            prev_index = Some(i);
            i += 1;
        }
    }
}

pub fn writeout_defered_file(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    header: &mut CpioFileStat,
    out_file_des: &mut File,
) {
    let mut file_hdr = header.clone();

    let c_name = header.get_c_name();

    let path = Path::new(&c_name);
    let mut in_file_des = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_RDONLY)
        .open(path)
        .unwrap();

    if get_archive_format() == ArchiveFormat::Crcascii {
        file_hdr.c_chksum =
            read_for_checksum(&mut in_file_des, file_hdr.c_filesize as u64, &c_name);
    }

    if write_out_header(output_tape, &mut file_hdr, out_file_des) != 0 {
        return;
    }

    copy_files_disk_to_tape(
        output_tape,
        input_tape,
        &mut in_file_des,
        out_file_des,
        file_hdr.c_filesize as i32,
        c_name.as_str(),
    );

    warn_if_file_changed(
        c_name.as_str(),
        file_hdr.c_filesize as u64,
        file_hdr.c_mtime as u64,
    );

    if get_archive_format() == ArchiveFormat::Tar || get_archive_format() == ArchiveFormat::Ustar {
        add_inode(
            file_hdr.c_ino,
            Some(c_name.clone()),
            file_hdr.c_dev_maj as u64,
            file_hdr.c_dev_min as u64,
        );
    }

    tape_pad_output(output_tape, out_file_des, file_hdr.c_filesize as u64);

    if get_reset_time_flag() {
        set_file_times(
            Some(&in_file_des),
            c_name.as_str(),
            file_hdr.c_mtime,
            file_hdr.c_mtime,
            0,
        );
    }
}

pub fn writeout_final_defers(
    output_tape: &mut MutexGuard<TapeOutput>,
    input_tape: &mut MutexGuard<TapeInput>,
    out_des: &mut File,
) {
    let global_deferments = GLOBAL_DEFERMENTS.lock().unwrap();

    for deferment in global_deferments.iter() {
        let other_count =
            count_defered_links_to_dev_ino_with_lock(&deferment.header, &global_deferments);

        if other_count == 1 {
            let mut header = deferment.header.clone();
            writeout_defered_file(output_tape, input_tape, &mut header, out_des);
        } else {
            let d = deferment.clone();
            let mut file_hdr = d.header;
            file_hdr.c_filesize = 0;
            write_out_header(output_tape, &mut file_hdr, out_des);
        }
    }
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

fn write_out_header(
    output_tape: &mut MutexGuard<TapeOutput>,
    file_hdr: &mut CpioFileStat,
    out_des: &mut File,
) -> i32 {
    0
}