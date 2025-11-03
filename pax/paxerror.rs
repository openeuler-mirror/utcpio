/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use crate::paxlib::*;
use gnu::error::*;
use libc::{gid_t, mode_t, uid_t};

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

pub fn close_error(name: &str) {
    call_arg_error("close", name);
}

pub fn close_warn(name: &str) {
    call_arg_warn("close", name);
}

pub fn exec_fatal(name: &str) {
    call_arg_fatal("exec", name);
}

pub fn link_error(target: &str, source: &str) {
    let e = errno();
    ERROR(
        e,
        format_args!(
            "{}: Cannot create link to {}",
            quotearg_colon(source),
            quote_n(1, target)
        ),
    );
}

pub fn mkdir_error(name: &str) {
    call_arg_error("mkdir", name);
}

pub fn mkfifo_error(name: &str) {
    call_arg_error("mkfifo", name);
}

pub fn mknod_error(name: &str) {
    call_arg_error("mknod", name);
}

pub fn open_error(name: &str) {
    call_arg_error("open", name);
}

pub fn open_fatal(name: &str) {
    call_arg_fatal("open", name);
}

pub fn open_warn(name: &str) {
    call_arg_warn("open", name);
}

pub fn read_error(name: &str) {
    call_arg_error("read", name);
}

pub fn read_error_details(name: &str, offset: i64, size: usize) {
    let e = errno();

    ERROR(
        e,
        format_args!(
            "{}: Read error at byte {}, while reading {} bytes",
            quotearg_colon(name),
            offset,
            size
        ),
    );
}
pub fn read_warn_details(name: &str, offset: i64, size: usize) {
    let e = errno();
    WARN(
        e,
        format_args!(
            "{}: Warning: Read error at byte {}, while reading {} byte",
            quotearg_colon(name),
            offset,
            size
        ),
    );
}

pub fn read_fatal(name: &str) {
    call_arg_fatal("read", name);
}

pub fn read_fatal_details(name: &str, offset: i64, size: usize) {
    let e = errno();
    FATAL_ERROR(
        e,
        format_args!(
            "{}: Read error at byte {}, while reading {} byte",
            quotearg_colon(name),
            offset,
            size
        ),
    );
}

pub fn readlink_error(name: &str) {
    call_arg_error("readlink", name);
}

pub fn readlink_warn(name: &str) {
    call_arg_warn("readlink", name);
}

pub fn rmdir_error(name: &str) {
    call_arg_error("rmdir", name);
}

pub fn savedir_error(name: &str) {
    call_arg_error("savedir", name);
}

pub fn savedir_warn(name: &str) {
    call_arg_warn("savedir", name);
}

pub fn seek_error(name: &str) {
    call_arg_error("seek", name);
}

pub fn seek_error_details(name: &str, offset: i64) {
    let e = errno();
    ERROR(
        e,
        format_args!("{}: Cannot seek to {}", quotearg_colon(name), offset),
    );
}

pub fn seek_warn(name: &str) {
    call_arg_warn("seek", name);
}

pub fn seek_warn_details(name: &str, offset: i64) {
    let e = errno();
    WARN(
        e,
        format_args!(
            "{}: Warning: Cannot seek to {}",
            quotearg_colon(name),
            offset
        ),
    );
}

pub fn symlink_error(contents: &str, name: &str) {
    let e = errno();
    ERROR(
        e,
        format_args!(
            "{}: Cannot create symlink to {}",
            quotearg_colon(name),
            quote_n(1, contents)
        ),
    );
}

pub fn chmod_error_details(name: &str, mode: mode_t) {
    let e = errno();

    ERROR(
        e,
        format_args!(
            "{}: Cannot change mode to {}",
            quotearg_colon(name),
            pax_decode_mode(mode)
        ),
    );
}

pub fn chown_error_details(name: &str, uid: uid_t, gid: gid_t) {
    let e = errno();
    ERROR(
        e,
        format_args!(
            "{}: Cannot change ownership to uid {}, gid {}",
            quotearg_colon(name),
            uid,
            gid
        ),
    );
}

pub fn stat_fatal(name: &str) {
    call_arg_fatal("stat", name);
}

pub fn stat_error(name: &str) {
    call_arg_error("stat", name);
}

pub fn stat_warn(name: &str) {
    call_arg_warn("stat", name);
}

pub fn truncate_error(name: &str) {
    call_arg_error("truncate", name);
}

pub fn truncate_warn(name: &str) {
    call_arg_warn("truncate", name);
}

pub fn unlink_error(name: &str) {
    call_arg_error("unlink", name);
}

pub fn utime_error(name: &str) {
    call_arg_error("utime", name);
}

pub fn waitpid_error(name: &str) {
    call_arg_error("waitpid", name);
}

pub fn write_error(name: &str) {
    call_arg_error("write", name);
}

pub fn write_error_details(name: &str, status: usize, size: usize) {
    if status == 0 {
        write_error(name);
    } else {
        ERROR(
            0,
            format_args!("{}: Wrote only {} of {} byte", name, status, size),
        );
    }
}

pub fn chdir_fatal(name: &str) {
    call_arg_fatal("chdir", name);
}
