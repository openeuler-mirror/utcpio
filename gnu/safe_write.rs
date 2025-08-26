/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::io::{self, Write};
use std::os::unix::io::{FromRawFd, RawFd};

const SYS_BUFSIZE_MAX: usize = 2_146_435_072;

pub fn safe_write(fd: &mut RawFd, buf: &[u8], len: i32) -> io::Result<usize> {
    if *fd < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid file descriptor",
        ));
    }

    let mut file = unsafe { std::fs::File::from_raw_fd(*fd) };
    let mut count = std::cmp::min(len as usize, buf.len()); // 使用 len 和 buf.len() 的最小值

    loop {
        match file.write(&buf[..count]) {
            Ok(bytes_written) if bytes_written > 0 => {
                return Ok(bytes_written); // Write succeeded
            }
            Ok(0) => return Ok(0), // End of file (EOF) or no data written
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                continue; // If interrupted by signal, try again
            }
            Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
                // If buffer too large, reduce it to SYS_BUFSIZE_MAX
                if count > SYS_BUFSIZE_MAX {
                    count = SYS_BUFSIZE_MAX;
                } else {
                    return Err(e); // Return other errors directly
                }
            }
            Err(e) => return Err(e), // Return other errors
            _ => {}                  // Ensure all cases are covered
        }
    }
}
