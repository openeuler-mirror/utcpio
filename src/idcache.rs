/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use libc::{gid_t, uid_t};
use std::{
    collections::HashMap,
    sync::{OnceLock, RwLock},
};
use users::{get_group_by_gid, get_group_by_name, get_user_by_name, get_user_by_uid};

// // Cache structures using HashMap instead of linked lists
// static mut USER_CACHE: Option<HashMap<uid_t, String>> = None;
// static mut NOUSER_CACHE: Option<HashMap<String, ()>> = None;
// static mut GROUP_CACHE: Option<HashMap<gid_t, String>> = None;
// static mut NOGROUP_CACHE: Option<HashMap<String, ()>> = None;
struct Cache {
    user_cache: RwLock<HashMap<uid_t, String>>,
    nouser_cache: RwLock<HashMap<String, ()>>,
    group_cache: RwLock<HashMap<gid_t, String>>,
    nogroup_cache: RwLock<HashMap<String, ()>>,
}

static CACHE: OnceLock<Cache> = OnceLock::new();

fn get_cache() -> &'static Cache {
    CACHE.get_or_init(|| Cache {
        user_cache: RwLock::new(HashMap::new()),
        nouser_cache: RwLock::new(HashMap::new()),
        group_cache: RwLock::new(HashMap::new()),
        nogroup_cache: RwLock::new(HashMap::new()),
    })
}
pub fn getuser(uid: uid_t) -> String {
    let cache = get_cache();

    // 检查缓存
    if let Ok(user_cache) = cache.user_cache.read() {
        if let Some(name) = user_cache.get(&uid) {
            return name.clone();
        }
    }

    // 查询系统
    let name = match get_user_by_uid(uid) {
        Some(user) => user.name().to_string_lossy().into_owned(),
        None => uid.to_string(),
    };

    // 更新缓存
    if let Ok(mut user_cache) = cache.user_cache.write() {
        user_cache.insert(uid, name.clone());
    }

    name
}
pub fn getuidbyname(user: &str) -> Option<uid_t> {
    let cache = get_cache();

    // 检查用户缓存
    if let Ok(user_cache) = cache.user_cache.read() {
        for (&uid, name) in user_cache.iter() {
            if name == user {
                return Some(uid);
            }
        }
    }

    // 检查不存在用户缓存
    if let Ok(nouser_cache) = cache.nouser_cache.read() {
        if nouser_cache.contains_key(user) {
            return None;
        }
    }

    // 查询系统
    match get_user_by_name(user) {
        Some(user_info) => {
            let uid = user_info.uid();
            if let Ok(mut user_cache) = cache.user_cache.write() {
                user_cache.insert(uid, user.to_string());
            }
            Some(uid)
        }
        None => {
            if let Ok(mut nouser_cache) = cache.nouser_cache.write() {
                nouser_cache.insert(user.to_string(), ());
            }
            None
        }
    }
}

pub fn getgroup(gid: gid_t) -> String {
    let cache = get_cache();

    // 检查缓存
    if let Ok(group_cache) = cache.group_cache.read() {
        if let Some(name) = group_cache.get(&gid) {
            return name.clone();
        }
    }

    // 查询系统
    let name = match get_group_by_gid(gid) {
        Some(group) => group.name().to_string_lossy().into_owned(),
        None => gid.to_string(),
    };

    // 更新缓存
    if let Ok(mut group_cache) = cache.group_cache.write() {
        group_cache.insert(gid, name.clone());
    }

    name
}
pub fn getgidbyname(group: &str) -> Option<gid_t> {
    let cache = get_cache();

    // 检查组缓存
    if let Ok(group_cache) = cache.group_cache.read() {
        for (&gid, name) in group_cache.iter() {
            if name == group {
                return Some(gid);
            }
        }
    }

    // 检查不存在组缓存
    if let Ok(nogroup_cache) = cache.nogroup_cache.read() {
        if nogroup_cache.contains_key(group) {
            return None;
        }
    }

    // 查询系统
    match get_group_by_name(group) {
        Some(group_info) => {
            let gid = group_info.gid();
            if let Ok(mut group_cache) = cache.group_cache.write() {
                group_cache.insert(gid, group.to_string());
            }
            Some(gid)
        }
        None => {
            if let Ok(mut nogroup_cache) = cache.nogroup_cache.write() {
                nogroup_cache.insert(group.to_string(), ());
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_getuser() {
        let uid = 0; // root
        let name = getuser(uid);
        assert!(!name.is_empty());
    }

    #[test]
    fn test_getuidbyname() {
        let name = "root";
        let uid = getuidbyname(name);
        assert!(uid.is_some());
        assert_eq!(uid.unwrap(), 0);
    }

    #[test]
    fn test_getgroup() {
        let gid = 0; // root
        let name = getgroup(gid);
        assert!(!name.is_empty());
    }

    #[test]
    fn test_getgidbyname() {
        let name = "root";
        let gid = getgidbyname(name);
        assert!(gid.is_some());
        assert_eq!(gid.unwrap(), 0);
    }
}
