// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

#![allow(
    clippy::type_complexity,
    clippy::manual_strip,
    dead_code,
    clippy::unnecessary_mut_passed
)]

use core::str::FromStr;
use nix::libc::{gid_t, uid_t};

// 模拟 passwd 和 group 结构（纯 Rust）
#[derive(Clone)]
pub struct Passwd {
    pub pw_name: String,
    pub pw_uid: uid_t,
    pub pw_gid: gid_t,
}

#[derive(Clone)]
pub struct Group {
    pub gr_name: String,
    pub gr_gid: gid_t,
}

fn getpwnam(name: &str) -> Option<Passwd> {
    // 这里是模拟实现，实际应从系统或其他数据源获取
    match name {
        "root" => Some(Passwd {
            pw_name: "root".to_string(),
            pw_uid: 0,
            pw_gid: 0,
        }),
        "user1" => Some(Passwd {
            pw_name: "user1".to_string(),
            pw_uid: 1000,
            pw_gid: 1000,
        }),
        _ => None,
    }
}

fn getgrgid(gid: gid_t) -> Option<Group> {
    // 模拟实现
    match gid {
        0 => Some(Group {
            gr_name: "root".to_string(),
            gr_gid: 0,
        }),
        1000 => Some(Group {
            gr_name: "users".to_string(),
            gr_gid: 1000,
        }),
        _ => None,
    }
}

fn getgrnam(name: &str) -> Option<Group> {
    // 模拟实现
    match name {
        "root" => Some(Group {
            gr_name: "root".to_string(),
            gr_gid: 0,
        }),
        "users" => Some(Group {
            gr_name: "users".to_string(),
            gr_gid: 1000,
        }),
        _ => None,
    }
}

// 检查字符串是否只包含数字（替代 isnumber_p）
fn is_number(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

pub fn parse_user_spec(
    spec: &str,
) -> Result<(uid_t, gid_t, Option<String>, Option<String>), &'static str> {
    static TIRED: &str = "virtual memory exhausted";
    let mut error_msg = None;

    let mut username = None;
    let mut groupname = None;
    let mut uid: uid_t = 0;
    let mut gid: gid_t = 0;

    // 分割用户和组部分
    let (user_part, group_part) = match spec.split_once([':', '.']) {
        Some((u, g)) => (u, if g.is_empty() { None } else { Some(g) }),
        None => (spec, None),
    };

    if user_part.is_empty() && group_part.is_none() {
        return Err("can not omit both user and group");
    }

    // 处理用户部分
    if !user_part.is_empty() {
        let (skip_lookup, user_str) = if user_part.starts_with('+') {
            (true, &user_part[1..])
        } else {
            (false, user_part)
        };

        let pwd = if skip_lookup {
            None
        } else {
            getpwnam(user_str)
        };

        if let Some(p) = pwd {
            uid = p.pw_uid;
            if group_part.is_none() && spec.contains([':', '.']) {
                gid = p.pw_gid;
                if let Some(g) = getgrgid(p.pw_gid) {
                    groupname = Some(g.gr_name);
                } else {
                    groupname = Some(p.pw_gid.to_string());
                }
            }
        } else if is_number(user_str) {
            if group_part.is_none() && spec.contains([':', '.']) {
                error_msg = Some("cannot get the login group of a numeric UID");
            } else if let Ok(n) = u32::from_str(user_str) {
                uid = n;
            }
        } else {
            error_msg = Some("invalid user");
        }

        if error_msg.is_none() {
            username = Some(user_str.to_string());
        }
    }

    // 处理组部分
    if let Some(g) = group_part {
        if error_msg.is_none() {
            let (skip_lookup, group_str) = if g.starts_with('+') {
                (true, &g[1..])
            } else {
                (false, g)
            };

            let grp = if skip_lookup {
                None
            } else {
                getgrnam(group_str)
            };

            if let Some(g) = grp {
                gid = g.gr_gid;
                groupname = Some(g.gr_name);
            } else if is_number(group_str) {
                if let Ok(n) = u32::from_str(group_str) {
                    gid = n;
                    groupname = Some(group_str.to_string());
                }
            } else {
                error_msg = Some("invalid group");
            }
        }
    }

    // 检查内存分配失败（这里用 Option 模拟）
    if error_msg.is_none() {
        if username.is_none() && !user_part.is_empty() {
            error_msg = Some(TIRED);
        }
        if groupname.is_none() && group_part.is_some() {
            error_msg = Some(TIRED);
        }
    }

    match error_msg {
        Some(err) => Err(err),
        None => Ok((uid, gid, username, groupname)),
    }
}
