/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::collapsible_if, clippy::partialeq_to_none)]

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::OpenOptionsExt;
use std::sync::atomic::Ordering;

use libc::{access, stat};

use crate::paxlib::*;
use crate::rtapelib::*;
use gnu::error::set_errno;
use gnu::util::validate_and_sanitize_path;

const REM_BIAS: i32 = 1 << 30;
//  const O_CREAT: i32 = 0o1000;
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
