/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use nix::sys::stat::{Mode, SFlag};

pub fn mode_string(mode: u32, str: &mut [char]) {
    str[0] = match SFlag::from_bits_truncate(mode) {
        SFlag::S_IFBLK => 'b',
        SFlag::S_IFCHR => 'c',
        SFlag::S_IFDIR => 'd',
        SFlag::S_IFREG => '-',
        SFlag::S_IFIFO => 'p',
        SFlag::S_IFLNK => 'l',
        SFlag::S_IFSOCK => 's',
        _ => '?',
    };

    str[1] = if mode & Mode::S_IRUSR.bits() != 0 {
        'r'
    } else {
        '-'
    };
    str[2] = if mode & Mode::S_IWUSR.bits() != 0 {
        'w'
    } else {
        '-'
    };
    str[3] = if mode & Mode::S_IXUSR.bits() != 0 {
        if mode & Mode::S_ISUID.bits() != 0 {
            's'
        } else {
            'x'
        }
    } else if mode & Mode::S_ISUID.bits() != 0 {
        'S'
    } else {
        '-'
    };

    str[4] = if mode & Mode::S_IRGRP.bits() != 0 {
        'r'
    } else {
        '-'
    };
    str[5] = if mode & Mode::S_IWGRP.bits() != 0 {
        'w'
    } else {
        '-'
    };
    str[6] = if mode & Mode::S_IXGRP.bits() != 0 {
        if mode & Mode::S_ISGID.bits() != 0 {
            's'
        } else {
            'x'
        }
    } else if mode & Mode::S_ISGID.bits() != 0 {
        'S'
    } else {
        '-'
    };

    str[7] = if mode & Mode::S_IROTH.bits() != 0 {
        'r'
    } else {
        '-'
    };
    str[8] = if mode & Mode::S_IWOTH.bits() != 0 {
        'w'
    } else {
        '-'
    };
    str[9] = if mode & Mode::S_IXOTH.bits() != 0 {
        if mode & Mode::S_ISVTX.bits() != 0 {
            't'
        } else {
            'x'
        }
    } else if mode & Mode::S_ISVTX.bits() != 0 {
        'T'
    } else {
        '-'
    };
}

// pub fn filemodestring(stat: &libc::stat, str: &mut [char]) {
//     mode_string(stat.st_mode as u32, str);
// }
