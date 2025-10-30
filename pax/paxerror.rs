/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use crate::paxlib::*;
use gnu::error::*;

fn quotearg_colon(input: &str) -> String {
    input.replace(":", "\\:") // 替换冒号为转义后的形式
}
fn quote_n(_n: i32, input: &str) -> String {
    // 假设 n 用于控制引号类型，这里简化为始终使用双引号
    format!("\"{}\"", input)
}

fn pax_decode_mode(mode: u32) -> String {
    let user_r = if mode & 0o400 != 0 { 'r' } else { '-' };
    let user_w = if mode & 0o200 != 0 { 'w' } else { '-' };
    let user_x = if mode & 0o100 != 0 { 'x' } else { '-' };
    let user_s = if mode & 0o4000 != 0 {
        if mode & 0o100 != 0 {
            's'
        } else {
            'S'
        }
    } else {
        user_x
    };

    let group_r = if mode & 0o040 != 0 { 'r' } else { '-' };
    let group_w = if mode & 0o020 != 0 { 'w' } else { '-' };
    let group_x = if mode & 0o010 != 0 { 'x' } else { '-' };
    let group_s = if mode & 0o2000 != 0 {
        if mode & 0o010 != 0 {
            's'
        } else {
            'S'
        }
    } else {
        group_x
    };

    let other_r = if mode & 0o004 != 0 { 'r' } else { '-' };
    let other_w = if mode & 0o002 != 0 { 'w' } else { '-' };
    let other_x = if mode & 0o001 != 0 { 'x' } else { '-' };
    let other_t = if mode & 0o1000 != 0 {
        if mode & 0o001 != 0 {
            't'
        } else {
            'T'
        }
    } else {
        other_x
    };

    format!(
        "{}{}{}{}{}{}{}{}{}",
        user_r, user_w, user_s, group_r, group_w, group_s, other_r, other_w, other_t
    )
}

pub fn call_arg_error(call: &str, name: &str) {
    let e = errno();
    ERROR(e, format_args!("{}: Cannot {}", quotearg_colon(name), call));
}

pub fn call_arg_fatal(call: &str, name: &str) {
    let e = errno();
    FATAL_ERROR(e, format_args!("{}: Cannot {}", quotearg_colon(name), call));
}
pub fn call_arg_warn(call: &str, name: &str) {
    let e = errno();
    WARN(
        e,
        format_args!("{}: Waring Cannot {}", quotearg_colon(name), call),
    );
}

pub fn chown_mode_error_details(name: &str, mode: u32) {
    let e = errno();
    let buf = pax_decode_mode(mode);
    ERROR(
        e,
        format_args!("{}: Cannot change mode to {}", quotearg_colon(name), buf),
    );
}

pub fn chown_uid_error_details(name: &str, uid: u32, gid: u32) {
    let e = errno();
    ERROR(
        e,
        format_args!(
            "{}: Cannot change ownership to uid {} gid {}",
            quotearg_colon(name),
            uid,
            gid
        ),
    );
}
