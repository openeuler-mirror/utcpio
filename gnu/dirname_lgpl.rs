/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

pub fn dir_len(file: &str) -> usize {
    let mut prefix_length: usize = 0;
    let mut length: usize;

    prefix_length += if prefix_length != 0 {
        if file.chars().nth(prefix_length).map_or(false, |c| c == '/') {
            1
        } else {
            0
        }
    } else if file.chars().next().map_or(false, |c| c == '/') {
        if file.chars().nth(1).map_or(false, |c| c == '/')
            && file.chars().nth(2).map_or(true, |c| c != '/')
        {
            2
        } else {
            1
        }
    } else {
        0
    };

    length = file.rfind('/').map_or(file.len(), |i| i);

    while prefix_length < length {
        if file.chars().nth(length - 1).map_or(false, |c| c == '/') {
            length -= 1;
        } else {
            break;
        }
    }
    length
}

pub fn mdir_name(file: &str) -> String {
    let length = dir_len(file);
    let append_dot = length == 0;
    // || (length == 0
    //     && file.chars().nth(2).map_or(false, |c| c != '\0')
    //     && file.chars().nth(2).map_or(true, |c| c != '/'));

    let mut dir = String::with_capacity(length + if append_dot { 1 } else { 0 } + 1);
    dir.push_str(&file[..length]);

    if append_dot {
        dir.push('.');
    }
    // dir.push('\0');  rust 不需要处理的空. push 会自动处理
    dir
}
