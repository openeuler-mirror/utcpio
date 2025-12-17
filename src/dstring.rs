// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::map_entry, clippy::while_let_loop)]

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::os::fd::{AsRawFd, RawFd};
use std::sync::Mutex;

use gnu::xmalloc::x2nrealloc;
use libc::EOF;

lazy_static! {
    static ref READER_CACHE: Mutex<HashMap<RawFd, BufReader<File>>> = Mutex::new(HashMap::new());
}
pub struct DynamicString {
    pub ds_size: usize,
    pub ds_idx: usize,
    pub ds_string: Vec<u8>,
}

pub const DYNAMIC_STRING_INITIALIZER: DynamicString = DynamicString {
    ds_size: 0,
    ds_idx: 0,
    ds_string: Vec::new(),
};

// pub fn ds_init(string: &mut DynamicString) {
//     string.ds_size = 0;
//     string.ds_idx = 0;
//     string.ds_string = Vec::new();
// }

// pub fn ds_free(string: &mut DynamicString) {
//     string.ds_string.clear();
//     string.ds_size = 0;
//     string.ds_idx = 0;
// }

fn ds_resize(string: &mut DynamicString, len: usize) {
    let needed_size = len + string.ds_idx;
    while needed_size >= string.ds_size {
        string.ds_string = x2nrealloc(string.ds_string.to_vec(), &mut string.ds_size, 1);
    }
}

pub fn ds_reset(s: &mut DynamicString, len: usize) {
    ds_resize(s, len);
    s.ds_idx = len;
}

/// 读取字符串到 DynamicString 中
///增加 reader 的目的是为了避免多次打开文件描述符
pub fn ds_fgetstr_common<'a>(
    f: Option<&'a mut File>,
    input_string: Option<&'a [u8]>,
    s: &'a mut DynamicString,
    eos: u8,
) -> Option<&'a [u8]> {
    // 初始化 DynamicString
    s.ds_idx = 0;
    s.ds_string.clear();
    s.ds_size = 0;

    // 处理输入源
    if let Some(file) = f {
        // 使用文件描述符获取或创建 BufReader，只获取一次锁
        let fd = file.as_raw_fd();
        let mut cache = READER_CACHE.lock().unwrap();

        // 如果缓存中没有对应的 BufReader，创建一个新的
        if !cache.contains_key(&fd) {
            let reader = BufReader::new(file.try_clone().unwrap());
            cache.insert(fd, reader);
        }

        // 开始读取循环
        let reader = cache.get_mut(&fd).unwrap();
        let mut buf = [0; 1];

        // 尝试读取第一个字符
        let first_ch = match reader.read_exact(&mut buf) {
            Ok(_) => buf[0],
            Err(_) => {
                // EOF，释放锁并返回None
                drop(cache);
                return None;
            }
        };

        // 如果第一个字符就是结束符，返回None
        if first_ch == eos || first_ch == EOF as u8 {
            drop(cache);
            return None;
        }

        // 存储第一个字符
        ds_resize(s, 0);
        s.ds_string[s.ds_idx] = first_ch;
        s.ds_idx += 1;

        // 继续读取剩余字符
        loop {
            match reader.read_exact(&mut buf) {
                Ok(_) => {
                    let ch = buf[0];
                    if ch == eos || ch == EOF as u8 {
                        break;
                    }

                    // 确保有足够空间并存储字符
                    ds_resize(s, 0);
                    s.ds_string[s.ds_idx] = ch;
                    s.ds_idx += 1;
                }
                Err(_) => {
                    // EOF，退出循环
                    break;
                }
            }
        }

        // 释放锁
        drop(cache);
    } else if let Some(input) = input_string {
        // 处理字符串输入
        for &ch in input {
            if ch == eos || ch == EOF as u8 {
                break;
            }

            // 确保有足够空间并存储字符
            ds_resize(s, 0);
            s.ds_string[s.ds_idx] = ch;
            s.ds_idx += 1;
        }
    } else {
        return None;
    }

    // 添加字符串结束符
    ds_resize(s, 0);
    s.ds_string[s.ds_idx] = b'\0';

    // 返回结果
    if s.ds_idx == 0 {
        None
    } else {
        Some(&s.ds_string[..s.ds_idx])
    }
}
// 添加清理函数
pub fn clear_reader_cache() {
    READER_CACHE.lock().unwrap().clear();
}

pub fn ds_append(s: &mut DynamicString, c: u8) {
    ds_resize(s, 0);
    s.ds_string[s.ds_idx] = c;
    if c != 0 {
        s.ds_idx += 1;
        ds_resize(s, 0);
        s.ds_string[s.ds_idx] = 0;
    }
}

pub fn ds_concat(s: &mut DynamicString, str: &str) {
    let len = str.len();
    ds_resize(s, len);
    s.ds_string[s.ds_idx..s.ds_idx + len].copy_from_slice(str.as_bytes());
    s.ds_idx += len;
    s.ds_string[s.ds_idx] = 0;
}

pub fn ds_fgetstr<'a>(f: &'a mut File, s: &'a mut DynamicString, eos: u8) -> Option<&'a [u8]> {
    ds_fgetstr_common(Some(f), None, s, eos)
}

pub fn ds_fgets<'a>(f: &'a mut File, s: &'a mut DynamicString) -> Option<&'a [u8]> {
    ds_fgetstr(f, s, b'\n')
}

// pub fn ds_fgetname<'a>(f: &'a mut File, s: &'a mut DynamicString) -> Option<&'a [u8]> {
//     ds_fgetstr(f, s, 0)
// }

pub fn ds_sgetstr<'a>(
    input_string: &'a [u8],
    s: &'a mut DynamicString,
    eos: u8,
) -> Option<&'a [u8]> {
    ds_fgetstr_common(None, Some(input_string), s, eos)
}

pub fn ds_endswith(s: &DynamicString, c: u8) -> bool {
    s.ds_idx > 0 && s.ds_string[s.ds_idx - 1] == c
}

pub fn ds_len(s: &DynamicString) -> usize {
    s.ds_idx
}
