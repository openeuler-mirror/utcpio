// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

use nix::errno::Errno;
use nix::unistd::{getegid, geteuid, getgid, getuid, pipe, setgid, setuid, Gid, Uid};
use nix::Result;
use std::os::fd::AsRawFd;
use std::process::Command;
use users::{get_group_by_gid, get_user_by_uid};

use std::os::unix::io::RawFd;

// 使用 nix::unistd::getuid
pub fn get_uid() -> Uid {
    getuid()
}

// 使用 nix::unistd::getgid
pub fn get_gid() -> Gid {
    getgid()
}

// 使用 users crate 获取用户名
pub fn get_pwuid(uid: Uid) -> Option<String> {
    get_user_by_uid(uid.as_raw()).map(|user| user.name().to_string_lossy().to_string())
}

// 模拟 initgroups，由于 nix 没有直接提供 initgroups 的功能，我们使用 users crate 和外部命令
pub fn initgroups(username: &str, gid: Gid) -> Result<()> {
    // 模拟 initgroups，这里简化为检查用户是否属于 gid 组
    if let Some(group) = get_group_by_gid(gid.as_raw()) {
        if Command::new("groups")
            .arg(username)
            .output()
            .map_or(false, |output| {
                String::from_utf8_lossy(&output.stdout).contains(&*group.name().to_string_lossy())
            })
        {
            Ok(())
        } else {
            Err(Errno::EPERM)
        }
    } else {
        Err(Errno::ENOENT)
    }
}

// 使用 nix::unistd::getegid
pub fn get_egid() -> Gid {
    getegid()
}

// 使用 nix::unistd::setgid
pub fn set_gid(gid: Gid) -> Result<()> {
    setgid(gid)
}

// 使用 nix::unistd::geteuid
pub fn get_euid() -> Uid {
    geteuid()
}

// 使用 nix::unistd::setuid
pub fn set_uid(uid: Uid) -> Result<()> {
    setuid(uid)
}

pub fn sys_reset_uid_gid() -> Result<()> {
    let uid = get_uid();
    let gid = get_gid();

    let username = get_pwuid(uid).ok_or(Errno::ENOENT)?;

    initgroups(&username, gid)?;

    if gid != get_egid() {
        set_gid(gid)?;
    }

    if uid != get_euid() {
        set_uid(uid)?;
    }

    Ok(())
}

pub fn create_pipe() -> (RawFd, RawFd) {
    match pipe() {
        Ok((read_fd, write_fd)) => (read_fd.as_raw_fd(), write_fd.as_raw_fd()),
        Err(_) => (-1, -1),
    }
}
