/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::io::SeekFrom;

pub fn rmt_open__(file_name: &str, open_mode: i32, bias: i32, remote_shell: Option<&str>) -> i32 {
    0
}

pub fn rmt_read__(handle: usize, buffer: &mut [u8], length: usize) -> usize {
    0
}

pub fn rmt_write__(handle: usize, buffer: &[u8], length: usize) -> usize {
    0
}

pub fn rmt_lseek__(handle: usize, offset: i64, whence: SeekFrom) -> i64 {
    0
}

pub fn rmt_close__(handle: usize) -> i32 {
    0
}

pub fn rmt_ioctl__(handle: usize, operation: u64, argument: &mut [u8]) -> i32 {
    0
}
