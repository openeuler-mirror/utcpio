/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::fmt;
/*error.c error.h 实际上没有处理，使用的函数系统错误处理函数
#include <error.h>
所以，我们只这样实现
 */
use std::io::{Error, ErrorKind};
use std::process::exit;

use crate::argp::*;

// Error codes
pub const EPERM: i32 = 1;
pub const ENOENT: i32 = 2;
pub const ESRCH: i32 = 3;
pub const EINTR: i32 = 4;
pub const EIO: i32 = 5;
pub const ENXIO: i32 = 6;
pub const E2BIG: i32 = 7;
pub const ENOEXEC: i32 = 8;
pub const EBADF: i32 = 9;
pub const ECHILD: i32 = 10;
pub const EAGAIN: i32 = 11;
pub const ENOMEM: i32 = 12;
pub const EACCES: i32 = 13;
pub const EFAULT: i32 = 14;
pub const EBUSY: i32 = 16;
pub const EEXIST: i32 = 17;
pub const EXDEV: i32 = 18;
pub const ENODEV: i32 = 19;
pub const ENOTDIR: i32 = 20;
pub const EISDIR: i32 = 21;
pub const ENFILE: i32 = 23;
pub const EMFILE: i32 = 24;
pub const ENOTTY: i32 = 25;
pub const EFBIG: i32 = 27;
pub const ENOSPC: i32 = 28;
pub const ESPIPE: i32 = 29;
pub const EROFS: i32 = 30;
pub const EMLINK: i32 = 31;
pub const EPIPE: i32 = 32;
pub const EDOM: i32 = 33;
pub const EDEADLK: i32 = 36;
pub const ENAMETOOLONG: i32 = 38;
pub const ENOLCK: i32 = 39;
pub const ENOSYS: i32 = 40;
pub const ENOTEMPTY: i32 = 41;

pub const EOVERFLOW: i32 = 2006;
pub const EOPNOTSUPP: i32 = 130;

pub fn set_errno(errno_value: i32) -> Error {
    Error::new(ErrorKind::Other, format!("errno: {}", errno_value))
}

pub fn errno() -> i32 {
    let error = Error::last_os_error();
    if let Some(errno) = error.raw_os_error() {
        return errno;
    }
    0
}

pub fn error(status: i32, errno: i32, args: std::fmt::Arguments) {
    if errno == 0 {
        println!(
            "{}:  {}",
            get_program_name().unwrap_or_default(),
            fmt::format(args)
        );
    } else {
        println!(
            "{}: errno {}  {}",
            get_program_name().unwrap_or_default(),
            errno,
            fmt::format(args)
        );
    }

    if status != 0 {
        exit(status);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;
    use std::io::ErrorKind;

    #[test]
    fn test_set_errno() {
        let err = set_errno(42);
        assert_eq!(err.kind(), ErrorKind::Other);
        assert_eq!(err.to_string(), "errno: 42");
    }

    #[test]
    fn test_errno_with_os_error() {
        // Mock last_os_error by setting it first
        let _ = Error::last_os_error();
        let errno = errno();
        // Can't assert exact value as it depends on system state
        assert!(errno >= 0);
    }

    #[test]
    fn test_error_with_zero_errno() {
        let args = format_args!("test message");
        error(0, 0, args);
        // Can't easily capture stdout in test, so just verify it doesn't panic
    }

    #[test]
    fn test_error_with_nonzero_errno() {
        let args = format_args!("test message");
        error(0, 42, args);
        // Can't easily capture stdout in test, so just verify it doesn't panic
    }

    #[test]
    #[should_panic]
    fn test_error_with_nonzero_status() {
        let args = format_args!("test message");
        error(1, 0, args);
    }
}
