/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::io::{self, Read};
use std::os::unix::io::{FromRawFd, RawFd};

pub const SYS_BUFSIZE_MAX: usize = 2_146_435_072;
pub const SAFE_READ_ERROR: usize = usize::MAX;

pub fn safe_read(fd: RawFd, buf: &mut [u8], read_size: usize) -> io::Result<usize> {
    let buf_len = buf.len(); // 获取缓冲区大小
    let mut count = std::cmp::min(read_size, buf_len); // 初始化 count 为读取大小或缓冲区大小，取较小值

    loop {
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) }; // 将 RawFd 转换为 File

        match file.read(&mut buf[..count]) {
            Ok(bytes_read) => return Ok(bytes_read),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == io::ErrorKind::InvalidInput && count > SYS_BUFSIZE_MAX => {
                count = SYS_BUFSIZE_MAX;
            }
            Err(e) => return Err(e),
        }
    }
}
