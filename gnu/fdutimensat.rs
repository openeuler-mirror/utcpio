/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use libc::timespec;
use std::ffi::CString;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::io::RawFd;
use std::time::SystemTime;

pub fn fdutimensat(
    file: Option<&File>,
    dir: RawFd,
    file_name: Option<&str>,
    ts: &[timespec; 2],
    atflag: i32,
) -> std::io::Result<()> {
    let result = if let Some(file_ref) = file {
        let fd = file_ref.as_raw_fd();
        unsafe { libc::futimens(fd, ts.as_ptr()) }
    } else {
        -1
    };

    if let Some(file_name_str) = file_name {
        if file.is_none() || (result == -1 && unsafe { *libc::__errno_location() } == libc::ENOSYS)
        {
            let file_cstr = CString::new(file_name_str).map_err(|_| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, "Invalid file name")
            })?;

            let utimensat_result =
                unsafe { libc::utimensat(dir, file_cstr.as_ptr(), ts.as_ptr(), atflag) };

            if utimensat_result == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }
        return Ok(());
    }

    if result == -1 {
        return Err(std::io::Error::last_os_error());
    }

    Ok(())
}

pub fn timespec_from_systemtime(time: SystemTime) -> timespec {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time before UNIX epoch");

    timespec {
        tv_sec: duration.as_secs() as libc::c_long,
        tv_nsec: duration.subsec_nanos() as libc::c_long,
    }
}
