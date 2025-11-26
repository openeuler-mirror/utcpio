/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */


#![allow(dead_code)]

use std::fs::File;
use std::sync::MutexGuard;

use gnu::error::*;

use crate::idcache::*;

use crate::appargs::*;
use crate::copyin::{from_octal, warn_junk_bytes};
use crate::copyout::to_ascii;
use crate::cpiohdr::*;
use crate::externs::*;
use crate::filetype::*;
use crate::global::*;
use crate::util::{cpio_set_c_name, tape_buffered_read, tape_buffered_write};

pub const TMAGIC: &[u8] = b"ustar";
pub const TMAGLEN: usize = 6;

//pub const MODE_ALL: u32 = S_ISUID | S_ISGID | S_ISVTX | MODE_RWX;

pub const TVERSION: &[u8] = b"00";
//pub const TVERSLEN: usize = 2;

// Type flags
pub const REGTYPE: u8 = b'0';
pub const AREGTYPE: u8 = 0;
pub const LNKTYPE: u8 = b'1';
pub const SYMTYPE: u8 = b'2';
pub const CHRTYPE: u8 = b'3';
pub const BLKTYPE: u8 = b'4';
pub const DIRTYPE: u8 = b'5';
pub const FIFOTYPE: u8 = b'6';
pub const CONTTYPE: u8 = b'7';

// Size of `name' field.
pub const TARNAMESIZE: usize = 100;
pub const TARLINKNAMESIZE: usize = 100;
pub const TARPREFIXSIZE: usize = 155;
pub const TARRECORDSIZE: usize = 512;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TarHeader {
    pub name: [u8; TARNAMESIZE],
    pub mode: [u8; 8],
    pub uid: [u8; 8],
    pub gid: [u8; 8],
    pub size: [u8; 12],
    pub mtime: [u8; 12],
    pub chksum: [u8; 8],
    pub typeflag: u8,
    pub linkname: [u8; TARLINKNAMESIZE],
    pub magic: [u8; 6],
    pub version: [u8; 2],
    pub uname: [u8; 32],
    pub gname: [u8; 32],
    pub devmajor: [u8; 8],
    pub devminor: [u8; 8],
    pub prefix: [u8; TARPREFIXSIZE],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union TarRecord {
    pub header: TarHeader,
    pub buffer: [u8; TARRECORDSIZE],
}

// Create a new empty TAR header
pub fn new_tar_header() -> TarHeader {
    TarHeader {
        name: [0; TARNAMESIZE],
        mode: [0; 8],
        uid: [0; 8],
        gid: [0; 8],
        size: [0; 12],
        mtime: [0; 12],
        chksum: [0; 8],
        typeflag: 0,
        linkname: [0; TARLINKNAMESIZE],
        magic: [0; 6],
        version: [0; 2],
        uname: [0; 32],
        gname: [0; 32],
        devmajor: [0; 8],
        devminor: [0; 8],
        prefix: [0; TARPREFIXSIZE],
    }
}
pub fn to_oct_or_error(
    value: u64,
    digits: usize,
    where_: &mut [u8],
    field: &str,
    file: &str,
) -> i32 {
    if to_ascii(where_, value, digits, LG_8, true) {
        error(
            1,
            0,
            format_args!("{}: {}: {}: value too large", file, field, value),
        );
    }
    0
}

macro_rules! to_oct {
    ($file_hdr:expr, $c_fld:ident, $mask:expr, $digits:expr, $tar_hdr:expr, $tar_field:ident) => {
        if to_oct_or_error(
            $file_hdr.$c_fld as u64 & $mask as u64,
            $digits,
            &mut $tar_hdr.$tar_field,
            stringify!($tar_field),
            $file_hdr.get_c_name().as_str(),
        ) != 0
        {
            return 1;
        }
    };
}

// Check if a block is all zeros
pub fn null_block(block: &[u8]) -> bool {
    block.iter().all(|&b| b == 0)
}

// Calculate checksum for a TAR header
pub fn tar_checksum(tar_hdr: &mut TarHeader) -> u32 {
    let mut sum: u32 = 0;
    let p_start = tar_hdr as *const _ as *const u8;
    let chksum_offset = unsafe { (&tar_hdr.chksum as *const _ as *const u8).offset_from(p_start) };
    let mut p = p_start;
    let q = unsafe { p.add(TARRECORDSIZE) };

    // Sum up to checksum field
    unsafe {
        while p < p_start.offset(chksum_offset) {
            sum += (*p as u32) & 0xff;
            p = p.add(1);
        }
    }

    // Add spaces for checksum field
    for _ in 0..8 {
        sum += b' ' as u32;
        unsafe {
            p = p.add(1);
        }
    }

    // Sum up rest of header
    unsafe {
        while p < q {
            sum += (*p as u32) & 0xff;
            p = p.add(1);
        }
    }

    sum
}

// Split a long filename into prefix and name parts
pub fn split_long_name(name: &str) -> (String, String) {
    let bytes = name.as_bytes();
    if bytes.len() <= TARNAMESIZE {
        return (String::new(), name.to_string());
    }

    let max_len = bytes.len().min(TARPREFIXSIZE + 1);
    let split_pos = bytes[..max_len]
        .iter()
        .rposition(|&b| b == b'/')
        .unwrap_or(0);

    if split_pos == 0 || bytes.len() - split_pos - 1 > TARNAMESIZE {
        (String::new(), name.to_string())
    } else {
        (
            String::from_utf8_lossy(&bytes[..split_pos]).to_string(),
            String::from_utf8_lossy(&bytes[split_pos + 1..]).to_string(),
        )
    }
}

// Check if a filename is too long for TAR format
pub fn is_tar_filename_too_long(name: &str) -> bool {
    let name_len = name.len();

    if name_len <= TARNAMESIZE {
        return false;
    }

    if get_archive_format() != ArchiveFormat::Ustar {
        return true;
    }

    if name_len > TARNAMESIZE + TARPREFIXSIZE + 1 {
        return true;
    }

    let (prefix, name) = split_long_name(name);
    prefix.is_empty() || name.len() > TARNAMESIZE
}

// Write out a TAR header
pub fn write_out_tar_header(
    output_tape: &mut MutexGuard<TapeOutput>,
    file_hdr: &mut CpioFileStat,
    out_des: &mut File,
) -> i32 {
    let mut tar_hdr = new_tar_header();

    let c_name = file_hdr.get_c_name();
    // Process filename
    let (prefix, name) = split_long_name(&c_name);
    copys_with_nul(name.as_bytes(), &mut tar_hdr.name);
    if !prefix.is_empty() {
        copys_with_nul(prefix.as_bytes(), &mut tar_hdr.prefix);
    }

    to_oct!(file_hdr, c_mode, MODE_ALL, 8, tar_hdr, mode);
    to_oct!(file_hdr, c_uid, !0u64, 8, tar_hdr, uid);
    to_oct!(file_hdr, c_gid, !0u64, 8, tar_hdr, gid);
    to_oct!(file_hdr, c_filesize, !0u64, 12, tar_hdr, size);
    to_oct!(file_hdr, c_mtime, !0u64, 12, tar_hdr, mtime);

    // Set type flag
    tar_hdr.typeflag = match file_hdr.c_mode & CP_IFMT {
        CP_IFREG => {
            if file_hdr.c_tar_linkname.is_some() {
                LNKTYPE
            } else {
                REGTYPE
            }
        }
        CP_IFDIR => DIRTYPE,
        CP_IFCHR => CHRTYPE,
        CP_IFBLK => BLKTYPE,
        CP_IFIFO => FIFOTYPE,
        CP_IFLNK => SYMTYPE,
        _ => REGTYPE,
    };

    // Handle links
    if let Some(ref link) = file_hdr.c_tar_linkname {
        copys_with_nul(link.as_bytes(), &mut tar_hdr.linkname);
        if tar_hdr.typeflag == SYMTYPE || tar_hdr.typeflag == LNKTYPE {
            tar_hdr.size.fill(b'0');
        }
    }

    // Set USTAR fields
    if get_archive_format() == ArchiveFormat::Ustar {
        tar_hdr.magic.copy_from_slice(TMAGIC);
        tar_hdr.version.copy_from_slice(TVERSION);

        let mut name = getuser(file_hdr.c_uid);
        copys_with_nul(name.as_bytes(), &mut tar_hdr.uname);

        name = getgroup(file_hdr.c_gid);
        copys_with_nul(name.as_bytes(), &mut tar_hdr.gname);

        to_oct!(file_hdr, c_rdev_maj, !0u64, 8, tar_hdr, devmajor);
        to_oct!(file_hdr, c_rdev_min, !0u64, 8, tar_hdr, devminor);
    }
    let checksum = tar_checksum(&mut tar_hdr);
    to_ascii(&mut tar_hdr.chksum, checksum as u64, 8, LG_8, true);

    let ptr = &tar_hdr as *const TarHeader as *const u8;
    let len = size_of::<TarHeader>();
    let mut buf = unsafe { Vec::from_raw_parts(ptr as *mut u8, len, len) };

    tape_buffered_write(output_tape, &mut buf, out_des, TARRECORDSIZE);

    0
}

// Helper function to copy bytes with null termination
fn copys_with_nul(src: &[u8], dest: &mut [u8]) {
    let len = src.len().min(dest.len() - 1);
    dest[..len].copy_from_slice(&src[..len]);
    dest[len] = 0;
}

// Check if a buffer contains a valid TAR header
pub fn is_tar_header(buf: &[u8]) -> i32 {
    let mut tar_hdr: TarHeader = unsafe { *(buf.as_ptr() as *mut TarHeader) };

    //let chk_sum_str = bytes_to_string(&tar_hdr.chksum);

    let chk_sum = from_octal(&tar_hdr.chksum.to_vec());
    let actual_sum = tar_checksum(&mut tar_hdr); // Now pass the mutable reference

    if chk_sum != actual_sum as u64 {
        return 0;
    }

    if tar_hdr.magic[..TMAGLEN - 1] == TMAGIC[..TMAGLEN - 1] {
        return 2;
    }

    1
}
fn stash_tar_filename(prefix: Option<&str>, filename: &str) -> String {
    let mut hold_tar_filename = String::with_capacity(TARNAMESIZE + TARPREFIXSIZE + 2);

    if let Some(p) = prefix {
        // 如果 prefix 存在
        hold_tar_filename.push_str(&p[..TARPREFIXSIZE]);
        hold_tar_filename.push('/');
        hold_tar_filename.push_str(&filename[..TARNAMESIZE]);
    } else {
        // 如果 prefix 为 None 或者空字符串
        hold_tar_filename.push_str(&filename[..TARNAMESIZE]);
    }

    // 返回生成的文件名字符串
    hold_tar_filename
}

fn bytes_to_string(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes)
        .unwrap_or("0") // 如果转换失败，默认值为 "0"
        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
}

// Read a TAR header from input
pub fn read_in_tar_header(
    input_tape: &mut MutexGuard<TapeInput>,
    file_hdr: &mut CpioFileStat,
    in_des: &mut File,
) -> i32 {
    let mut bytes_skipped = 0;
    let mut warned = false;
    let mut tar_rec = TarRecord {
        buffer: [0; TARRECORDSIZE],
    };

    // Read header block
    tape_buffered_read(
        input_tape,
        &mut unsafe { tar_rec.buffer.to_vec() },
        in_des,
        TARRECORDSIZE,
    );

    // Check for a block of 0's
    if null_block(&unsafe { tar_rec.buffer }) {
        file_hdr.set_c_name(CPIO_TRAILER_NAME);
        return 0;
    }

    loop {
        // Safe because we're reading from a union where both variants are POD types
        let mut tar_hdr = unsafe { tar_rec.header };

        //  let chk_sum_str = bytes_to_string(&tar_hdr.chksum);

        let chk_sum = from_octal(&tar_hdr.chksum.to_vec());

        if chk_sum != tar_checksum(&mut tar_hdr) as u64 {
            if !warned {
                warn_junk_bytes(bytes_skipped);
                warned = true;
            }
            // Skip 1 byte and try again
            unsafe {
                let buf = &mut tar_rec.buffer;
                buf.copy_within(1..TARRECORDSIZE, 0);
                tape_buffered_read(
                    input_tape,
                    &mut buf[TARRECORDSIZE - 1..TARRECORDSIZE].to_vec(),
                    in_des,
                    1,
                );
            }
            bytes_skipped += 1;
            continue;
        }

        // Process filename
        if get_archive_format() != ArchiveFormat::Ustar {
            let name_str = std::str::from_utf8(&tar_hdr.name)
                .unwrap_or("0") // 如果转换失败，默认值为 "0"
                .trim_matches(|c: char| c.is_whitespace() || c == '\0');

            let tar_name = stash_tar_filename(None, name_str);

            cpio_set_c_name(file_hdr, tar_name.as_str());

            // file_hdr.c_name = String::from_utf8_lossy(&tar_hdr.name)
            //     .trim_matches('\0')
            //     .to_string();
        } else {
            let prefix = bytes_to_string(&tar_hdr.prefix);
            let name = bytes_to_string(&tar_hdr.name);
            let c_name = if prefix.is_empty() {
                name.to_string()
            } else {
                format!("{}/{}", prefix, name)
            };
            file_hdr.set_c_name(&c_name);
        }

        // Set basic fields
        file_hdr.c_nlink = 1;

        //let mode_str = bytes_to_string(&tar_hdr.mode);

        file_hdr.c_mode = from_octal(&tar_hdr.mode.to_vec()) as u32 & 0o7777;

        // Handle UID/GID
        if get_archive_format() == ArchiveFormat::Ustar && !get_numeric_uid() {
            if let Some(uid) = getuidbyname(bytes_to_string(&tar_hdr.uname)) {
                file_hdr.c_uid = uid;
            } else {
                //  let uid_str: &str = bytes_to_string(&tar_hdr.uid);
                file_hdr.c_uid = from_octal(&tar_hdr.uid.to_vec()) as u32;
            }

            if let Some(gid) = getgidbyname(bytes_to_string(&tar_hdr.gname)) {
                file_hdr.c_gid = gid;
            } else {
                //  let gid_str = bytes_to_string(&tar_hdr.gid);

                file_hdr.c_gid = from_octal(&tar_hdr.gid.to_vec()) as u32;
            }
        } else {
            //let uid_str: &str = bytes_to_string(&tar_hdr.uid);
            //let gid_str = bytes_to_string(&tar_hdr.gid);

            file_hdr.c_uid = from_octal(&tar_hdr.uid.to_vec()) as u32;
            file_hdr.c_gid = from_octal(&tar_hdr.gid.to_vec()) as u32;
        }

        // Set remaining numeric fields
        file_hdr.c_filesize = from_octal(&tar_hdr.size.to_vec()) as i64;
        file_hdr.c_mtime = from_octal(&tar_hdr.mtime.to_vec()) as i64;
        file_hdr.c_rdev_maj = from_octal(&tar_hdr.devmajor.to_vec()) as i32;
        file_hdr.c_rdev_min = from_octal(&tar_hdr.devminor.to_vec()) as u32;
        file_hdr.c_tar_linkname = None;

        // Set file type and handle special cases
        file_hdr.c_mode &= !CP_IFMT;
        match tar_hdr.typeflag {
            REGTYPE | CONTTYPE => file_hdr.c_mode |= CP_IFREG,
            DIRTYPE => file_hdr.c_mode |= CP_IFDIR,
            CHRTYPE => {
                file_hdr.c_mode |= CP_IFCHR;
                file_hdr.c_tar_linkname = Some(
                    String::from_utf8_lossy(&tar_hdr.linkname)
                        .trim_matches('\0')
                        .to_string(),
                );
                file_hdr.c_filesize = 0;
            }
            BLKTYPE => {
                file_hdr.c_mode |= CP_IFBLK;
                file_hdr.c_tar_linkname = Some(
                    String::from_utf8_lossy(&tar_hdr.linkname)
                        .trim_matches('\0')
                        .to_string(),
                );
                file_hdr.c_filesize = 0;
            }
            FIFOTYPE => {
                file_hdr.c_mode |= CP_IFIFO;
                file_hdr.c_tar_linkname = Some(
                    String::from_utf8_lossy(&tar_hdr.linkname)
                        .trim_matches('\0')
                        .to_string(),
                );
                file_hdr.c_filesize = 0;
            }
            SYMTYPE => {
                file_hdr.c_mode |= CP_IFLNK;
                file_hdr.c_tar_linkname = Some(
                    String::from_utf8_lossy(&tar_hdr.linkname)
                        .trim_matches('\0')
                        .to_string(),
                );
                file_hdr.c_filesize = 0;
            }
            LNKTYPE => {
                file_hdr.c_mode |= CP_IFREG;
                file_hdr.c_tar_linkname = Some(
                    String::from_utf8_lossy(&tar_hdr.linkname)
                        .trim_matches('\0')
                        .to_string(),
                );
                file_hdr.c_filesize = 0;
            }
            AREGTYPE => {
                let c_name = file_hdr.get_c_name();
                if c_name.ends_with('/') {
                    file_hdr.c_mode |= CP_IFDIR;
                } else {
                    file_hdr.c_mode |= CP_IFREG;
                }
            }
            _ => file_hdr.c_mode |= CP_IFREG,
        }
        break;
    }

    if bytes_skipped > 0 {
        warn_junk_bytes(bytes_skipped);
    }

    0
}
