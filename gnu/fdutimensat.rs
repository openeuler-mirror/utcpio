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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, OpenOptions};
    use std::os::unix::io::AsRawFd;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_fdutimensat_with_file() {
        let file = File::open("/dev/null").unwrap();
        let now = SystemTime::now();
        let ts = [timespec_from_systemtime(now), timespec_from_systemtime(now)];

        let result = fdutimensat(Some(&file), -1, None, &ts, 0);
        assert!(result.is_ok());
    }

    #[test]
    fn test_fdutimensat_with_file_name() {
        let temp_file = "/tmp/test_file";
        let _ = File::create(temp_file).unwrap();

        let now = SystemTime::now();
        let ts = [timespec_from_systemtime(now), timespec_from_systemtime(now)];

        let result = fdutimensat(None, libc::AT_FDCWD, Some(temp_file), &ts, 0);
        assert!(result.is_ok());

        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_fdutimensat_with_file_fallback_to_file_name() {
        let temp_file = "/tmp/test_file_fallback";
        let file = File::create(temp_file).unwrap();
        let now = SystemTime::now();
        let ts = [timespec_from_systemtime(now), timespec_from_systemtime(now)];

        // Force ENOSYS error
        unsafe { *libc::__errno_location() = libc::ENOSYS };

        let result = fdutimensat(Some(&file), libc::AT_FDCWD, Some(temp_file), &ts, 0);
        assert!(result.is_ok());

        std::fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_fdutimensat_invalid_file_name() {
        let now = SystemTime::now();
        let ts = [timespec_from_systemtime(now), timespec_from_systemtime(now)];

        let result = fdutimensat(None, libc::AT_FDCWD, Some("\0"), &ts, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_timespec_from_systemtime() {
        let time = UNIX_EPOCH + std::time::Duration::new(123456789, 987654321);
        let ts = timespec_from_systemtime(time);

        assert_eq!(ts.tv_sec, 123456789);
        assert_eq!(ts.tv_nsec, 987654321);
    }
}
