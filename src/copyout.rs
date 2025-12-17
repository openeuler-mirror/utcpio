// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

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
use crate::tar::*;
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

fn field_width_error(filename: &str, fieldname: &str, value: u64, width: usize, nul: bool) {
    eprintln!(
        "{}: value {} {} out of allowed range 0..{}",
        filename,
        fieldname,
        value,
        (1 << ((width - nul as usize) * 3)) - 1
    );
}

fn field_width_warning(filename: &str, fieldname: &str) {
    if get_warn_option() as usize & CPIO_WARN_TRUNCATE != 0 {
        error(0, 0, format_args!("{}: truncating {}", filename, fieldname));
    }
}

fn to_ascii_or_warn(
    where_: &mut [u8],
    n: u64,
    digits: usize,
    logbase: u32,
    filename: &str,
    fieldname: &str,
) {
    if to_ascii(where_, n, digits, logbase, false) {
        field_width_warning(filename, fieldname);
    }
}

fn to_ascii_or_error(
    where_: &mut [u8],
    n: u64,
    digits: usize,
    logbase: u32,
    filename: &str,
    fieldname: &str,
) -> bool {
    if to_ascii(where_, n, digits, logbase, false) {
        field_width_error(filename, fieldname, n, digits, false);
        true
    } else {
        false
    }
}

fn write_out_new_ascii_header(
    output_tape: &mut MutexGuard<TapeOutput>,
    magic_string: &str,
    file_hdr: &mut CpioFileStat,
    out_des: &mut File,
) -> i32 {
    let mut ascii_header = [0u8; 110];

    let c_name = file_hdr.get_c_name();

    // Write magic string directly to the array
    ascii_header[0..6].copy_from_slice(magic_string.as_bytes());
    to_ascii_or_warn(
        &mut ascii_header[6..14],
        file_hdr.c_ino,
        8,
        4,
        &c_name,
        "inode number",
    );
    to_ascii_or_warn(
        &mut ascii_header[14..22],
        file_hdr.c_mode as u64,
        8,
        4,
        &c_name,
        "file mode",
    );
    to_ascii_or_warn(
        &mut ascii_header[22..30],
        file_hdr.c_uid as u64,
        8,
        4,
        &c_name,
        "uid",
    );
    to_ascii_or_warn(
        &mut ascii_header[30..38],
        file_hdr.c_gid as u64,
        8,
        4,
        &c_name,
        "gid",
    );
    to_ascii_or_warn(
        &mut ascii_header[38..46],
        file_hdr.c_nlink as u64,
        8,
        4,
        &c_name,
        "number of links",
    );
    to_ascii_or_warn(
        &mut ascii_header[46..54],
        file_hdr.c_mtime as u64,
        8,
        4,
        &c_name,
        "modification time",
    );
    if to_ascii_or_error(
        &mut ascii_header[54..62],
        file_hdr.c_filesize as u64,
        8,
        4,
        &c_name,
        "file size",
    ) {
        return 1;
    }
    if to_ascii_or_error(
        &mut ascii_header[62..70],
        file_hdr.c_dev_maj as u64,
        8,
        4,
        &c_name,
        "device major number",
    ) {
        return 1;
    }
    if to_ascii_or_error(
        &mut ascii_header[70..78],
        file_hdr.c_dev_min as u64,
        8,
        4,
        &c_name,
        "device minor number",
    ) {
        return 1;
    }
    if to_ascii_or_error(
        &mut ascii_header[78..86],
        file_hdr.c_rdev_maj as u64,
        8,
        4,
        &c_name,
        "rdev major",
    ) {
        return 1;
    }
    if to_ascii_or_error(
        &mut ascii_header[86..94],
        file_hdr.c_rdev_min as u64,
        8,
        4,
        &c_name,
        "rdev minor",
    ) {
        return 1;
    }
    if to_ascii_or_error(
        &mut ascii_header[94..102],
        file_hdr.c_namesize as u64,
        8,
        4,
        &c_name,
        "name size",
    ) {
        return 1;
    }
    to_ascii(
        &mut ascii_header[102..110],
        file_hdr.c_chksum as u64,
        8,
        4,
        false,
    );

    tape_buffered_write(
        output_tape,
        &mut ascii_header.to_vec(),
        out_des,
        ascii_header.len(),
    );
    tape_buffered_write(
        output_tape,
        &mut file_hdr.c_name.clone(),
        out_des,
        file_hdr.c_namesize,
    );

    tape_pad_output(
        output_tape,
        out_des,
        file_hdr.c_namesize as u64 + ascii_header.len() as u64,
    );
    0
}

fn write_out_old_ascii_header(
    output_tape: &mut MutexGuard<TapeOutput>,
    dev: u64,
    rdev: u64,
    file_hdr: &mut CpioFileStat,
    out_des: &mut File,
) -> i32 {
    let mut ascii_header = [0u8; 76];
    let p = &mut ascii_header[..];

    to_ascii(&mut p[0..6], file_hdr.c_magic as u64, 6, 3, false);
    to_ascii_or_warn(
        &mut p[6..12],
        dev,
        6,
        3,
        &file_hdr.get_c_name(),
        "device number",
    );
    to_ascii_or_warn(
        &mut p[12..18],
        file_hdr.c_ino,
        6,
        3,
        &file_hdr.get_c_name(),
        "inode number",
    );
    to_ascii_or_warn(
        &mut p[18..24],
        file_hdr.c_mode as u64,
        6,
        3,
        &file_hdr.get_c_name(),
        "file mode",
    );
    to_ascii_or_warn(
        &mut p[24..30],
        file_hdr.c_uid as u64,
        6,
        3,
        &file_hdr.get_c_name(),
        "uid",
    );
    to_ascii_or_warn(
        &mut p[30..36],
        file_hdr.c_gid as u64,
        6,
        3,
        &file_hdr.get_c_name(),
        "gid",
    );
    to_ascii_or_warn(
        &mut p[36..42],
        file_hdr.c_nlink as u64,
        6,
        3,
        &file_hdr.get_c_name(),
        "number of links",
    );
    to_ascii_or_warn(&mut p[42..48], rdev, 6, 3, &file_hdr.get_c_name(), "rdev");
    to_ascii_or_warn(
        &mut p[48..59],
        file_hdr.c_mtime as u64,
        11,
        3,
        &file_hdr.get_c_name(),
        "modification time",
    );
    if to_ascii_or_error(
        &mut p[59..65],
        file_hdr.c_namesize as u64,
        6,
        3,
        &file_hdr.get_c_name(),
        "name size",
    ) {
        return 1;
    }
    if to_ascii_or_error(
        &mut p[65..76],
        file_hdr.c_filesize as u64,
        11,
        3,
        &file_hdr.get_c_name(),
        "file size",
    ) {
        return 1;
    }

    tape_buffered_write(
        output_tape,
        &mut ascii_header.to_vec(),
        out_des,
        ascii_header.len(),
    );
    tape_buffered_write(
        output_tape,
        &mut file_hdr.c_name.clone(),
        out_des,
        file_hdr.c_namesize,
    );

    0
}

fn hp_compute_dev(file_hdr: &mut CpioFileStat, pdev: &mut dev_t, prdev: &mut dev_t) {
    match file_hdr.c_mode & CP_IFMT {
        CP_IFCHR | CP_IFBLK | CP_IFSOCK | CP_IFIFO => {
            file_hdr.c_filesize =
                makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8) as i64;
            *pdev = makedev(0, 1) as dev_t;
            *prdev = makedev(0, 1) as dev_t;
        }
        _ => {
            *pdev = makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8) as dev_t;
            *prdev = makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8) as dev_t;
        }
    }
}

pub fn write_out_binary_header(
    output_tape: &mut MutexGuard<TapeOutput>,
    rdev: dev_t,
    file_hdr: &mut CpioFileStat,
    out_des: &mut File,
) -> i32 {
    let mut short_hdr = OldCpioHeader::new();

    short_hdr.c_magic = 0o070707;
    short_hdr.c_dev = makedev(file_hdr.c_dev_maj as u8, file_hdr.c_dev_min as u8) as u16;

    let c_name = file_hdr.get_c_name().clone();

    if (get_warn_option() as usize & CPIO_WARN_TRUNCATE) != 0 && (file_hdr.c_ino >> 16) != 0 {
        error(0, 0, format_args!("{}: truncating inode number", c_name));
    }

    short_hdr.c_ino = (file_hdr.c_ino & 0xFFFF) as u16;
    if (short_hdr.c_ino as u64) != file_hdr.c_ino {
        field_width_warning(&c_name, "inode number");
    }

    short_hdr.c_mode = (file_hdr.c_mode & 0xFFFF) as u16;
    if (short_hdr.c_mode as u32) != file_hdr.c_mode {
        field_width_warning(&c_name, "file mode");
    }

    short_hdr.c_uid = (file_hdr.c_uid & 0xFFFF) as u16;
    if (short_hdr.c_uid as u32) != file_hdr.c_uid {
        field_width_warning(&c_name, "uid");
    }

    short_hdr.c_gid = (file_hdr.c_gid & 0xFFFF) as u16;
    if (short_hdr.c_gid as u32) != file_hdr.c_gid {
        field_width_warning(&c_name, "gid");
    }

    short_hdr.c_nlink = (file_hdr.c_nlink & 0xFFFF) as u16;
    if (short_hdr.c_nlink as usize) != file_hdr.c_nlink {
        field_width_warning(&c_name, "number of links");
    }

    short_hdr.c_rdev = rdev as u16;
    short_hdr.c_mtimes[0] = (file_hdr.c_mtime >> 16) as u16;
    short_hdr.c_mtimes[1] = (file_hdr.c_mtime & 0xFFFF) as u16;

    short_hdr.c_namesize = (file_hdr.c_namesize & 0xFFFF) as u16;
    if (short_hdr.c_namesize as usize) != file_hdr.c_namesize {
        //let maxbuf_size = int_bufsize_bound::<u32>() + 1;
        //        let  maxbuf = vec![0; maxbuf_size];
        let s: String = umaxtostr(file_hdr.c_namesize as u32);
        error(
            0,
            0,
            format_args!(
                "{}: value {} {} out of allowed range 0..{}",
                c_name, "name size", s, 0xFFFFu16
            ),
        );
        return 1;
    }

    short_hdr.c_filesizes[0] = (file_hdr.c_filesize >> 16) as u16;
    short_hdr.c_filesizes[1] = (file_hdr.c_filesize & 0xFFFF) as u16;

    if ((short_hdr.c_filesizes[0] as i64) << 16) + (short_hdr.c_filesizes[1] as i64)
        != file_hdr.c_filesize
    {
        //        let maxbuf_size = int_bufsize_bound::<u32>() + 1;

        //      let mut maxbuf = vec![0; maxbuf_size as u32];
        let s: String = umaxtostr(file_hdr.c_filesize as u32);

        error(
            0,
            0,
            format_args!(
                "{}: value {} {} out of allowed range 0..{}",
                c_name, "file size", s, 0xFFFFu32
            ),
        );
        return 1;
    }

    // Output the file header.
    let short_hdr_bytes = unsafe {
        std::slice::from_raw_parts(
            &short_hdr as *const OldCpioHeader as *const u8,
            std::mem::size_of::<OldCpioHeader>(),
        )
    };
    tape_buffered_write(output_tape, &mut short_hdr_bytes.to_vec(), out_des, 26);

    // Write file name to output.
    tape_buffered_write(
        output_tape,
        &mut file_hdr.c_name.to_vec(),
        out_des,
        file_hdr.c_namesize,
    );

    tape_pad_output(output_tape, out_des, file_hdr.c_namesize as u64 + 26);
    0
}

fn write_out_header(
    output_tape: &mut MutexGuard<TapeOutput>,
    file_hdr: &mut CpioFileStat,
    out_des: &mut File,
) -> i32 {
    let mut dev: dev_t = 0;
    let mut rdev: dev_t = 0;

    match get_archive_format() {
        ArchiveFormat::Newascii => {
            write_out_new_ascii_header(output_tape, "070701", file_hdr, out_des)
        }
        ArchiveFormat::Crcascii => {
            write_out_new_ascii_header(output_tape, "070702", file_hdr, out_des)
        }
        ArchiveFormat::Oldascii => write_out_old_ascii_header(
            output_tape,
            makedev(file_hdr.c_dev_maj as u8, file_hdr.c_dev_min as u8) as u64,
            makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8) as u64,
            file_hdr,
            out_des,
        ) as i32,
        ArchiveFormat::Hpoldascii => {
            hp_compute_dev(file_hdr, &mut dev, &mut rdev);
            write_out_old_ascii_header(output_tape, dev, rdev, file_hdr, out_des)
        }
        ArchiveFormat::Tar | ArchiveFormat::Ustar => {
            if is_tar_filename_too_long(&file_hdr.get_c_name()) {
                eprintln!("{}: file name too long", file_hdr.get_c_name());
                return 1;
            }
            write_out_tar_header(output_tape, file_hdr, out_des)
        }
        ArchiveFormat::Binary => write_out_binary_header(
            output_tape,
            makedev(file_hdr.c_rdev_maj as u8, file_hdr.c_rdev_min as u8) as dev_t,
            file_hdr,
            out_des,
        ),
        ArchiveFormat::Hpbinary => {
            hp_compute_dev(file_hdr, &mut dev, &mut rdev);
            write_out_binary_header(output_tape, rdev, file_hdr, out_des)
        }
        _ => panic!("Unknown archive format"),
    }
}

pub fn assign_string(pvar: &mut String, value: &str) {
    pvar.clear();
    let trimmed_name = value.trim_end_matches('\0');
    pvar.push_str(trimmed_name);
}

pub fn write_xattrs(metadata_fd: RawFd, path: &str) -> i32 {
    if metadata_fd < 0 {
        return 0;
    }

    // Convert path to CString for FFI calls
    let c_path = match CString::new(path) {
        Ok(p) => p,
        Err(_) => return -libc::EINVAL,
    };

    // Get xattr list length
    let list_len = unsafe { libc::llistxattr(c_path.as_ptr(), ptr::null_mut(), 0) };

    if list_len <= 0 {
        return -libc::ENOENT;
    }

    // Allocate buffer for xattr list
    let mut xattr_list = vec![0u8; list_len as usize];

    // Get actual xattr list
    let len = unsafe {
        libc::llistxattr(
            c_path.as_ptr(),
            xattr_list.as_mut_ptr() as *mut libc::c_char,
            list_len as libc::size_t,
        )
    };

    if len != list_len {
        return -libc::EIO;
    }

    // Get file handle for metadata_fd
    let mut file = unsafe { File::from_raw_fd(metadata_fd) };

    // Truncate file
    if file.set_len(0).is_err() {
        return -libc::EIO;
    }

    // Seek to start
    if file.seek(SeekFrom::Start(0)).is_err() {
        return -libc::EIO;
    }

    let mut list_pos = 0;
    while list_pos < list_len as usize {
        // Get current xattr name
        let name = match CStr::from_bytes_until_nul(&xattr_list[list_pos..]) {
            Ok(n) => n,
            Err(_) => return -libc::EINVAL,
        };
        let name_len = name.to_bytes_with_nul().len();

        // Get value length
        let value_len =
            unsafe { libc::lgetxattr(c_path.as_ptr(), name.as_ptr(), ptr::null_mut(), 0) };

        if value_len < 0 {
            eprintln!("cannot get xattrs");
            break;
        }

        // Get value if it exists
        let value = if value_len > 0 {
            let mut buf = vec![0u8; value_len as usize];
            let len = unsafe {
                libc::lgetxattr(
                    c_path.as_ptr(),
                    name.as_ptr(),
                    buf.as_mut_ptr() as *mut libc::c_void,
                    value_len as libc::size_t,
                )
            };
            if len != value_len {
                break;
            }
            buf
        } else {
            Vec::new()
        };

        // Prepare header
        let mut hdr = MetadataHdr::default();
        let size = format!(
            "{:08x}",
            std::mem::size_of::<MetadataHdr>() + name_len + value_len as usize
        );
        hdr.c_size.copy_from_slice(size.as_bytes());

        // Write header
        if file
            .write_all(unsafe {
                slice::from_raw_parts(
                    &hdr as *const _ as *const u8,
                    std::mem::size_of::<MetadataHdr>(),
                )
            })
            .is_err()
        {
            break;
        }

        // Write name
        if file.write_all(name.to_bytes_with_nul()).is_err() {
            break;
        }

        // Write value
        if !value.is_empty() && file.write_all(&value).is_err() {
            break;
        }

        // Sync to disk
        if file.sync_all().is_err() {
            break;
        }

        list_pos += name_len;
    }

    if list_pos != list_len as usize {
        return -libc::EINVAL;
    }

    0
}

pub fn process_copy_out() -> io::Result<()> {
    let mut input_name = DYNAMIC_STRING_INITIALIZER;

    let mut file_hdr = CpioFileStat::new();

    let mut in_file_des: File;
    let mut out_file_des = get_archive_des()?;

    let mut orig_file_name = String::new();
    let mut template = "/tmp/cpio-metadata-XXXXXX".to_string();

    let mut ret: i32;
    let mut metadata_fd: RawFd = -1;
    let mut metadata = 0;
    let mut old_metadata;
    //let mut hard_link:i32;

    // let mut pos = 0;

    file_hdr.c_magic = 0o70707;

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

    if isrmt(&mut out_file_des) {
        output_tape.output_is_special = true;
        output_tape.output_is_seekable = false;
    } else if let Ok(metadata) = out_file_des.metadata() {
        output_tape.output_is_special =
            metadata.file_type().is_block_device() || metadata.file_type().is_char_device();
        output_tape.output_is_seekable = metadata.file_type().is_file();
    }

    if get_append_flag() {
        process_copy_in()?;
        prepare_append(&mut output_tape, &mut input_tape, &mut out_file_des);
    } else {
        change_dir();
    }

    if get_metadata_type() != MetadataTypes::TypeNone {
        metadata_fd = unsafe { mkstemp(template.as_mut_ptr() as *mut libc::c_char) };
        if metadata_fd < 0 {
            error(0, 0, format_args!("cannot create temporary file"));
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "cannot create temporary file",
            ));
        }
    }

    let mut stdin_file = unsafe { File::from_raw_fd(libc::STDIN_FILENO) };

    loop {
        old_metadata = metadata;
        let mut hard_link = 0;

        if metadata != 0 {
            metadata = 0;
            if get_metadata_type() != MetadataTypes::TypeXattr {
                error(0, 0, format_args!("metadata type not supported"));
                continue;
            }
            ret = write_xattrs(metadata_fd, &orig_file_name);
            if ret < 0 {
                continue;
            }
            ds_sgetstr(template.as_bytes(), &mut input_name, get_name_end() as u8);
        } else {
            // 读取文件名，如果EOF则退出循环
            if ds_fgetstr(&mut stdin_file, &mut input_name, get_name_end() as u8).is_none() {
                break;
            }
        }

        if input_name.ds_string[0] == 0 {
            error(0, 0, format_args!("blank line ignored"));
            continue;
        }

        let path_bytes = &input_name.ds_string[..input_name.ds_idx];
        let path = String::from_utf8_lossy(path_bytes).to_string();

        // 首先尝试获取符号链接的元数据，如果失败再尝试普通文件的元数据
        let mut file_stat = match fs::symlink_metadata(path.clone()) {
            Ok(stat) => stat,
            Err(_) => {
                // 如果symlink_metadata失败，尝试普通的metadata
                match fs::metadata(path.clone()) {
                    Err(_) => {
                        stat_error(path.as_str());
                        continue;
                    }
                    Ok(stat) => stat,
                }
            }
        };

        stat_to_cpio(&mut file_stat, &mut file_hdr);

        if (get_archive_format() == ArchiveFormat::Tar
            || get_archive_format() == ArchiveFormat::Ustar)
            && file_hdr.c_mode & CP_IFDIR != 0
            && !ds_endswith(&input_name, b'/')
        {
            ds_append(&mut input_name, b'/');
        }

        if old_metadata != 0 {
            assign_string(&mut orig_file_name, &template);
            ds_sgetstr(
                METADATA_FILENAME.as_bytes(),
                &mut input_name,
                get_name_end() as u8,
            );
            file_hdr.c_mode |= 0x10000;
        } else {
            assign_string(
                &mut orig_file_name,
                &String::from_utf8_lossy(&input_name.ds_string),
            );
        }

        // let mut input_name_string = String::from_utf8_lossy(&input_name.ds_string).into_owned();
        cpio_safer_name_suffix(&mut orig_file_name, false, !get_no_abs_paths_flag(), true);
        cpio_set_c_name(&mut file_hdr, orig_file_name.as_str());

        // Process file based on type
        match file_hdr.c_mode & CP_IFMT {
            CP_IFREG => {
                if get_archive_format() == ArchiveFormat::Tar
                    || get_archive_format() == ArchiveFormat::Ustar
                {
                    if let Some(otherfile) = find_inode_file(
                        file_hdr.c_ino,
                        file_hdr.c_dev_maj as u64,
                        file_hdr.c_dev_min as u64,
                    ) {
                        file_hdr.c_tar_linkname = Some(otherfile);
                        write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des);
                        continue;
                    }
                }

                if (get_archive_format() == ArchiveFormat::Newascii
                    || get_archive_format() == ArchiveFormat::Crcascii)
                    && file_hdr.c_nlink > 1
                {
                    if last_link(&file_hdr) {
                        writeout_other_defers(&mut output_tape, &file_hdr, &mut out_file_des);
                    } else {
                        add_link_defer(&file_hdr);
                        hard_link = 1;
                        continue;
                    }
                }

                let file = File::open(&orig_file_name);
                match file {
                    Ok(file) => in_file_des = file,
                    Err(_e) => {
                        continue;
                    }
                }

                if get_archive_format() == ArchiveFormat::Crcascii {
                    file_hdr.c_chksum = read_for_checksum(
                        &mut in_file_des,
                        file_hdr.c_filesize as u64,
                        &orig_file_name,
                    );
                }

                if write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des) != 0 {
                    continue;
                }

                copy_files_disk_to_tape(
                    &mut output_tape,
                    &mut input_tape,
                    &mut in_file_des,
                    &mut out_file_des,
                    file_hdr.c_filesize as i32,
                    &orig_file_name,
                );

                warn_if_file_changed(
                    &orig_file_name,
                    file_hdr.c_filesize as u64,
                    file_hdr.c_mtime as u64,
                );

                if get_archive_format() == ArchiveFormat::Tar
                    || get_archive_format() == ArchiveFormat::Ustar
                {
                    add_inode(
                        file_hdr.c_ino,
                        Some(orig_file_name.clone()),
                        file_hdr.c_dev_maj as u64,
                        file_hdr.c_dev_min as u64,
                    );
                }

                tape_pad_output(
                    &mut output_tape,
                    &mut out_file_des,
                    file_hdr.c_filesize as u64,
                );

                if get_reset_time_flag() {
                    set_file_times(
                        Some(&in_file_des),
                        &orig_file_name,
                        file_stat.atime() as i64,
                        file_stat.mtime() as i64,
                        0,
                    );
                }
            }

            CP_IFDIR => {
                file_hdr.c_filesize = 0;
                if get_ignore_dirnlink_option() {
                    file_hdr.c_nlink = 2;
                }
                if write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des) != 0 {
                    continue;
                }
            }

            CP_IFCHR | CP_IFBLK | CP_IFSOCK | CP_IFIFO => {
                if get_archive_format() == ArchiveFormat::Tar {
                    error(
                        0,
                        0,
                        format_args!("{} not dumped: not a regular file", orig_file_name),
                    );
                    continue;
                } else if get_archive_format() == ArchiveFormat::Ustar {
                    if let Some(otherfile) = find_inode_file(
                        file_hdr.c_ino,
                        file_hdr.c_dev_maj as u64,
                        file_hdr.c_dev_min as u64,
                    ) {
                        file_hdr.c_mode = (file_stat.mode() & 0o7777) as u32;
                        file_hdr.c_mode |= CP_IFREG;
                        file_hdr.c_tar_linkname = Some(otherfile);
                        if write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des) != 0
                        {
                            continue;
                        }
                    }
                    add_inode(
                        file_hdr.c_ino,
                        Some(orig_file_name.clone()),
                        file_hdr.c_dev_maj as u64,
                        file_hdr.c_dev_min as u64,
                    );
                    file_hdr.c_filesize = 0;
                }

                file_hdr.c_filesize = 0;
                if write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des) != 0 {
                    continue;
                }
            }

            CP_IFLNK => {
                let mut link_size: usize = 0;
                let mut link_name = String::new();
                let read_name = fs::read_link(&orig_file_name);
                match read_name {
                    Ok(name) => {
                        // 安全地转换为字符串，处理非UTF-8字符
                        match name.to_str() {
                            Some(s) => {
                                link_name = s.to_string();
                                link_size = link_name.len(); // 使用字符串长度而不是字节长度
                            }
                            None => {
                                // 如果无法转换为UTF-8，使用to_string_lossy
                                link_name = name.to_string_lossy().to_string();
                                link_size = link_name.len();
                            }
                        }
                    }
                    Err(e) => {
                        // 记录错误但不跳过文件，继续处理
                        eprintln!("Warning: Cannot read symlink {}: {}", orig_file_name, e);
                        // 尝试使用原始路径作为链接内容
                        link_name = format!("{}", orig_file_name);
                        link_size = link_name.len();
                    }
                }

                if link_size == 0 {
                    readlink_warn(orig_file_name.as_str());
                    continue;
                }

                cpio_safer_name_suffix(&mut link_name, false, !get_no_abs_paths_flag(), true);

                file_hdr.c_filesize = link_size as i64;

                if get_archive_format() == ArchiveFormat::Tar
                    || get_archive_format() == ArchiveFormat::Ustar
                {
                    if link_size + 1 > 100 {
                        error(
                            0,
                            0,
                            format_args!("{}: symbolic link too long", file_hdr.get_c_name()),
                        );
                    } else {
                        file_hdr.c_tar_linkname = Some(link_name);
                        if write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des) != 0
                        {
                            continue;
                        }
                    }
                } else {
                    if write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des) != 0 {
                        continue;
                    }
                    tape_buffered_write(
                        &mut output_tape,
                        unsafe { link_name.as_mut_vec() },
                        &mut out_file_des,
                        link_size,
                    );
                    tape_pad_output(&mut output_tape, &mut out_file_des, link_size as u64);
                }
            }
            _ => {
                break;
            }
        }

        if get_verbose_flag() {
            eprintln!("{}", orig_file_name);
        }
        if get_dot_flag() {
            eprint!(".");
        }
        if get_metadata_type() != MetadataTypes::TypeNone && old_metadata == 0 && hard_link == 0 {
            metadata = 1;
        }
    }

    writeout_final_defers(&mut output_tape, &mut input_tape, &mut out_file_des);

    file_hdr.c_ino = 0;
    file_hdr.c_mode = 0;
    file_hdr.c_uid = 0;
    file_hdr.c_gid = 0;
    file_hdr.c_nlink = 1;
    file_hdr.c_dev_maj = 0;
    file_hdr.c_dev_min = 0;
    file_hdr.c_rdev_maj = 0;
    file_hdr.c_rdev_min = 0;
    file_hdr.c_mtime = 0;
    file_hdr.c_chksum = 0;
    file_hdr.c_filesize = 0;
    file_hdr.set_c_name("TRAILER!!!");
    //    file_hdr.c_namesize = 0;

    cpio_set_c_name(&mut file_hdr, CPIO_TRAILER_NAME);

    if get_archive_format() != ArchiveFormat::Tar && get_archive_format() != ArchiveFormat::Ustar {
        write_out_header(&mut output_tape, &mut file_hdr, &mut out_file_des);
    } else {
        write_nuls_to_file(
            &mut output_tape,
            1024,
            &mut out_file_des,
            tape_buffered_write,
        );
    }

    tape_clear_rest_of_block(&mut output_tape, &mut out_file_des);
    tape_empty_output_buffer(&mut output_tape, &mut out_file_des);

    if get_dot_flag() {
        eprintln!();
    }
    if !get_quiet_flag() {
        let blocks = (output_tape.output_bytes as u64 + get_io_block_size() as u64 - 1)
            / get_io_block_size() as u64;
        eprintln!("{} block{}", blocks, if blocks == 1 { "" } else { "s" });
    }

    if get_metadata_type() != MetadataTypes::TypeNone {
        unsafe { libc::close(metadata_fd) };
        fs::remove_file(template)?;
    }

    Ok(())
}
