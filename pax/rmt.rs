// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::collapsible_if, clippy::partialeq_to_none)]

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::Ordering;

use libc::{access, creat, dup, fcntl, fstat, isatty, lstat, mode_t, stat};

use crate::paxlib::*;
use crate::rtapelib::*;
use gnu::error::set_errno;
use gnu::util::validate_and_sanitize_path;

const REM_BIAS: i32 = 1 << 30;
const O_CREAT: i32 = 0o1000;
const EOPNOTSUPP: i32 = 95;

pub fn remdev(dev_name: &str) -> bool {
    if FORCE_LOCAL_OPTION.load(Ordering::Relaxed) {
        return false;
    }

    if let Some(colon_pos) = dev_name.find(':') {
        if colon_pos > 0 {
            if dev_name[..colon_pos].find('/') == None {
                let rmt_dev_name = dev_name.to_string();
                set_rmt_dev_name(Some(rmt_dev_name));
                return true;
            }
        }
    }
    false
}

pub fn isrmt(fd: &File) -> bool {
    let fd = fd.as_raw_fd();
    fd >= REM_BIAS
}

pub fn rmtopen(dev_name: &str, oflag: i32, mode: u32, command: &str) -> io::Result<File> {
    if remdev(dev_name) {
        // 远程设备：调用 rmt_open__ 获取文件描述符
        let fd = rmt_open__(dev_name, oflag, REM_BIAS, Some(command));
        if fd < 0 {
            // 如果 rmt_open__ 返回负值，表示错误
            return Err(io::Error::from_raw_os_error(fd.abs()));
        }
        // 将文件描述符转换为 File
        let file = unsafe { File::from_raw_fd(fd) };
        Ok(file)
    } else {
        // 本地设备：验证和清理路径，然后使用 OpenOptions 打开文件
        let safe_path = validate_and_sanitize_path(dev_name)?;
        let mut options = OpenOptions::new();

        // 根据 oflag 设置打开模式
        if oflag != 0 {
            options.read(true);
        }
        if oflag & libc::O_WRONLY != 0 {
            options.write(true);
        }
        if oflag & libc::O_CREAT != 0 {
            options.create(true);
        }
        options.mode(mode);

        // 打开文件并返回 File 对象
        let file = options.open(safe_path)?;
        Ok(file)
    }
}

pub fn rmtaccess(dev_name: &str, amode: i32) -> i32 {
    if remdev(dev_name) {
        0
    } else {
        // 验证和清理路径
        match validate_and_sanitize_path(dev_name) {
            Ok(safe_path) => match CString::new(safe_path.to_string_lossy().as_ref()) {
                Ok(c_dev_name) => unsafe { access(c_dev_name.as_ptr(), amode) },
                Err(_) => {
                    set_errno(libc::EINVAL);
                    -1
                }
            },
            Err(_) => {
                // 返回错误码表示路径无效
                set_errno(libc::EINVAL);
                -1
            }
        }
    }
}

pub fn rmtstat(dev_name: &str, buffer: &mut libc::stat) -> i32 {
    if remdev(dev_name) {
        let _ = io::Error::from_raw_os_error(EOPNOTSUPP);
        -1
    } else {
        // 验证和清理路径
        match validate_and_sanitize_path(dev_name) {
            Ok(safe_path) => match CString::new(safe_path.to_string_lossy().as_ref()) {
                Ok(c_dev_name) => unsafe { stat(c_dev_name.as_ptr(), buffer) },
                Err(_) => {
                    set_errno(libc::EINVAL);
                    -1
                }
            },
            Err(_) => {
                set_errno(libc::EINVAL);
                -1
            }
        }
    }
}

pub fn rmtcreat(dev_name: &str, mode: u32, command: &str) -> i32 {
    if remdev(dev_name) {
        rmt_open__(dev_name, O_CREAT | libc::O_WRONLY, REM_BIAS, Some(command))
    } else {
        // 验证和清理路径
        match validate_and_sanitize_path(dev_name) {
            Ok(safe_path) => match CString::new(safe_path.to_string_lossy().as_ref()) {
                Ok(c_dev_name) => unsafe { creat(c_dev_name.as_ptr(), mode as mode_t) },
                Err(_) => {
                    set_errno(libc::EINVAL);
                    -1
                }
            },
            Err(_) => {
                set_errno(libc::EINVAL);
                -1
            }
        }
    }
}

pub fn rmtlstat(dev_name: &str, buffer: &mut libc::stat) -> i32 {
    if remdev(dev_name) {
        let _ = io::Error::from_raw_os_error(EOPNOTSUPP);
        -1
    } else {
        // 验证和清理路径
        match validate_and_sanitize_path(dev_name) {
            Ok(safe_path) => match CString::new(safe_path.to_string_lossy().as_ref()) {
                Ok(c_dev_name) => unsafe { lstat(c_dev_name.as_ptr(), buffer) },
                Err(_) => {
                    set_errno(libc::EINVAL);
                    -1
                }
            },
            Err(_) => {
                set_errno(libc::EINVAL);
                -1
            }
        }
    }
}

pub fn rmtread(file: &File, buffer: &mut [u8], length: usize) -> usize {
    let fd: i32 = file.as_raw_fd();
    if isrmt(file) {
        rmt_read__((fd - REM_BIAS) as usize, buffer, length)
    } else if fd == libc::STDIN_FILENO {
        let mut stdin = io::stdin();
        match stdin.read(&mut buffer[..length]) {
            Ok(read_bytes) => read_bytes,
            Err(e) => {
                eprintln!("Error reading from stdin: {:?}", e);
                0
            }
        }
    } else {
        let mut file_ref = file;
        match (&mut file_ref).read(&mut buffer[..length]) {
            Ok(read_bytes) => read_bytes,
            Err(e) => {
                eprintln!("Error reading from file descriptor {}: {:?}", fd, e);
                0
            }
        }
    }
}

pub fn rmtwrite(file: &mut File, buffer: &[u8], length: usize) -> usize {
    if isrmt(file) {
        let fd: i32 = file.as_raw_fd();
        rmt_write__((fd - REM_BIAS) as usize, buffer, length)
    } else {
        file.write(buffer).unwrap_or(0)
    }
}

pub fn rmtlseek(file: &mut File, offset: i64, whence: i32) -> i64 {
    let pos = match whence {
        libc::SEEK_SET => SeekFrom::Start(offset as u64),
        libc::SEEK_CUR => SeekFrom::Current(offset),
        libc::SEEK_END => SeekFrom::End(offset),
        _ => return -1,
    };
    let fd: i32 = file.as_raw_fd();
    if isrmt(file) {
        rmt_lseek__((fd - REM_BIAS) as usize, offset, pos) // Corrected SeekFrom usage
    } else {
        match file.seek(pos) {
            Ok(offset) => offset.try_into().unwrap_or(-1),
            Err(_) => -1,
        }
    }
}

pub fn rmtclose(file: &File) -> i32 {
    let fd: i32 = file.as_raw_fd();

    if isrmt(file) {
        rmt_close__((fd - REM_BIAS) as usize)
    } else {
        let _file = unsafe { File::from_raw_fd(fd) };
        0
    }
}

pub fn rmtioctl(file: &File, request: u64, argument: &mut [u8]) -> i32 {
    let fd: i32 = file.as_raw_fd();

    if isrmt(file) {
        rmt_ioctl__((fd - REM_BIAS) as usize, request, argument)
    } else {
        unsafe { libc::ioctl(fd, request, argument as *mut _) }
    }
}

pub fn rmtdup(file: &File) -> i32 {
    if isrmt(file) {
        let _ = io::Error::from_raw_os_error(EOPNOTSUPP);
        -1
    } else {
        let fd: i32 = file.as_raw_fd();
        unsafe { dup(fd) }
    }
}

pub fn rmtfstat(file: &File, buffer: &mut libc::stat) -> i32 {
    if isrmt(file) {
        let _ = io::Error::from_raw_os_error(EOPNOTSUPP);
        -1
    } else {
        let fd: i32 = file.as_raw_fd();
        unsafe { fstat(fd, buffer) }
    }
}

pub fn rmtfcntl(file: &File, command: i32, argument: i32) -> i32 {
    if isrmt(file) {
        let _ = io::Error::from_raw_os_error(EOPNOTSUPP);
        -1
    } else {
        let fd: i32 = file.as_raw_fd();
        unsafe { fcntl(fd, command, argument) }
    }
}

pub fn rmtisatty(file: &File) -> i32 {
    if isrmt(file) {
        0
    } else {
        let fd: i32 = file.as_raw_fd();
        unsafe { isatty(fd) }
    }
}
