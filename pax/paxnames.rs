/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::redundant_closure)]

use lazy_static::lazy_static;
use std::collections::HashSet;
use std::path::{Component, Path};
use std::sync::Mutex;

lazy_static! {
    static ref PREFIX_TABLES: [Mutex<HashSet<String>>; 2] =
        [Mutex::new(HashSet::new()), Mutex::new(HashSet::new()),];
}

fn is_slash(c: char) -> bool {
    c == '/'
}

fn file_system_prefix_len(file_name: &str) -> usize {
    let path = Path::new(file_name);
    let mut len = 0;

    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                len = prefix.as_os_str().len();
            }
            Component::RootDir => {
                len = 1;
            }
            _ => break,
        }
    }
    len
}

fn compute_safe_prefix_len(file_name: &str, initial: usize) -> usize {
    let mut max_len = initial;
    let chars: Vec<char> = file_name[initial..].chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if i + 1 < chars.len() && chars[i] == '.' && chars[i + 1] == '.' {
            let next = i + 2;
            max_len = initial + next;

            if next < chars.len() && is_slash(chars[next]) {
                max_len += 1;
                i = next + 1;
            } else {
                i = next;
            }
        } else {
            while i < chars.len() && !is_slash(chars[i]) {
                i += 1;
            }
            while i < chars.len() && is_slash(chars[i]) {
                i += 1;
            }
        }
    }

    let mut safe_len = max_len;
    while safe_len < file_name.len() {
        let c = file_name.as_bytes()[safe_len];
        if c != b'/' && c != b'\\' {
            break;
        }
        safe_len += 1;
    }

    safe_len
}

pub fn removed_prefixes_p() -> bool {
    !PREFIX_TABLES[0].lock().unwrap().is_empty() || !PREFIX_TABLES[1].lock().unwrap().is_empty()
}

pub fn safer_name_suffix(file_name: &str, link_target: bool, absolute_names: bool) -> String {
    let prefix_len = if absolute_names {
        0
    } else {
        let fs_prefix = file_system_prefix_len(file_name);
        let computed = compute_safe_prefix_len(file_name, fs_prefix);
        let mut final_len = computed;

        // Skip leading slashes after prefix
        while final_len < file_name.len() {
            let c = file_name.as_bytes()[final_len];
            if c != b'/' && c != b'\\' {
                break;
            }
            final_len += 1;
        }
        final_len
    };

    // Handle prefix insertion and warnings
    if prefix_len > 0 {
        let prefix = &file_name[..prefix_len];
        let table_idx = link_target as usize;
        let inserted = PREFIX_TABLES[table_idx]
            .lock()
            .unwrap()
            .insert(prefix.to_string());

        if inserted {
            let msg = if link_target {
                "Removing leading `{}' from hard link targets"
            } else {
                "Removing leading `{}' from member names"
            };
            eprintln!("{} {}", msg, prefix);
        }
    }

    // Get the suffix part
    let suffix = &file_name[prefix_len..];
    if suffix.is_empty() {
        ".".to_string()
    } else {
        suffix.replace(|c| is_slash(c), "/")
    }
}
