/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::io::{self}; // Removed unused Write import
use std::os::fd::RawFd;

// Corrected import path for safe_write
use crate::safe_write::safe_write;
pub fn full_write(fd: &mut RawFd, buf: &[u8], len: usize) -> io::Result<usize> {
    let mut total_written = 0;
    let mut remaining = buf;
    let mut remaining_len: usize = len;

    while !remaining.is_empty() && remaining_len > 0 {
        let write_len = std::cmp::min(remaining.len(), remaining_len);
        let write_buf = &remaining[..write_len];

        match safe_write(fd, write_buf, write_len as i32) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "Write failed with no bytes written",
                ));
            }
            Ok(n_written) => {
                total_written += n_written;
                remaining = &remaining[n_written..];
                remaining_len = remaining_len.saturating_sub(n_written);
            }
            Err(e) => {
                return Err(e); // Return the error if it occurs
            }
        }
    }

    Ok(total_written)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 模拟的 safe_write 函数用于测试
    fn mock_safe_write(fd: &mut RawFd, buf: &[u8], len: i32) -> io::Result<usize> {
        // 模拟写入成功的情况
        if len > 0 {
            Ok(len as usize)
        } else {
            Ok(0)
        }
    }

    // 模拟会失败的 safe_write 函数
    fn mock_failing_safe_write(_fd: &mut RawFd, _buf: &[u8], _len: i32) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, "mock write error"))
    }

    #[test]
    fn test_full_write_success() {
        let mut fd = unsafe { std::os::unix::io::RawFd::from(1) };
        let buf = b"test data";
        let result = full_write(&mut fd, buf, buf.len());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), buf.len());
    }

    #[test]
    fn test_full_write_partial() {
        let mut fd = unsafe { std::os::unix::io::RawFd::from(1) };
        let buf = b"test data";
        let result = full_write(&mut fd, buf, 4); // 只写入前4字节
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 4);
    }

    #[test]
    fn test_full_write_zero_length() {
        let mut fd = unsafe { std::os::unix::io::RawFd::from(1) };
        let buf = b"";
        let result = full_write(&mut fd, buf, 0);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }
}
