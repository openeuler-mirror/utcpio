/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */



use libc::{fcntl, F_GETFD, F_SETFD, FD_CLOEXEC};

fn set_cloexec_flag(desc: i32, value: bool) -> i32 {
    let flags = unsafe { fcntl(desc, F_GETFD, 0) };
    if flags < 0 {
        return -1;
    }

    let new_flags = if value {
        flags | FD_CLOEXEC
    } else {
        flags & !FD_CLOEXEC
    };

    if flags == new_flags || unsafe { fcntl(desc, F_SETFD, new_flags) } != -1 {
        return 0;
    } else {
        return -1;
    }
}



// Helper function to duplicate a file descriptor and set the CLOEXEC flag.
pub fn dup_cloexec(fd: i32) -> i32 {
    return set_cloexec_flag(fd, true);
}