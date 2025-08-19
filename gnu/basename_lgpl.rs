/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(
    clippy::while_let_on_iterator,
    clippy::iter_nth_zero,
    clippy::manual_range_contains
)]

const DOUBLE_SLASH_IS_DISTINCT_ROOT: bool = false;
const FILE_SYSTEM_DRIVE_PREFIX_CAN_BE_RELATIVE: bool = true;

pub fn last_component(name: &str) -> &str {
    let base = &name[file_system_prefix_len(name)..];
    let mut p = base.chars();
    let mut last_was_slash = false;
    let mut result = base;

    while let Some(c) = p.next() {
        if is_slash(c) {
            last_was_slash = true;
        } else if last_was_slash {
            result = &base[base.find(c).unwrap()..];
            last_was_slash = false;
        }
    }

    result
}

pub fn base_len(name: &str) -> usize {
    let mut len = name.len();
    let prefix_len = file_system_prefix_len(name);

    while len > 1 && is_slash(name.chars().nth(len - 1).unwrap()) {
        len -= 1;
    }

    if DOUBLE_SLASH_IS_DISTINCT_ROOT
        && len == 1
        && is_slash(name.chars().nth(0).unwrap())
        && is_slash(name.chars().nth(1).unwrap())
        && name.chars().nth(2).is_none()
    {
        return 2;
    }

    if FILE_SYSTEM_DRIVE_PREFIX_CAN_BE_RELATIVE
        && prefix_len > 0
        && len == prefix_len
        && is_slash(name.chars().nth(prefix_len).unwrap())
    {
        return prefix_len + 1;
    }

    len
}

fn file_system_prefix_len(name: &str) -> usize {
    if has_device(name) {
        2
    } else {
        0
    }
}

fn has_device(name: &str) -> bool {
    if name.len() >= 2 {
        let first_char = name.chars().next().unwrap();
        let second_char = name.chars().nth(1).unwrap();

        let first_char_lower = first_char.to_ascii_lowercase();

        if first_char_lower >= 'a' && first_char_lower <= 'z' && second_char == ':' {
            return true;
        }
    }
    false
}

// 模拟的函数，需要根据实际情况修改
fn is_slash(c: char) -> bool {
    c == '/' || c == '\\'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_component() {
        // 测试普通路径
        assert_eq!(last_component("a/b/c"), "c");
        assert_eq!(last_component("a/b/c/"), "c");
        assert_eq!(last_component("a\\b\\c"), "c");

        // 测试带设备前缀的路径
        assert_eq!(last_component("C:/a/b"), "b");
        assert_eq!(last_component("C:\\a\\b"), "b");

        // 测试根路径
        assert_eq!(last_component("/"), "/");
        assert_eq!(last_component("\\"), "\\");
        assert_eq!(last_component("C:/"), "/");

        // 测试空路径
        assert_eq!(last_component(""), "");
    }

    #[test]
    fn test_base_len() {
        // 测试普通路径
        assert_eq!(base_len("a/b/c"), 5);
        assert_eq!(base_len("a/b/c/"), 5);
        assert_eq!(base_len("a\\b\\c"), 5);

        // 测试带斜杠结尾的路径
        assert_eq!(base_len("a/b/"), 3);
        assert_eq!(base_len("a/b////"), 3);

        // 测试带设备前缀的路径
        assert_eq!(base_len("C:/a/b"), 6);
        assert_eq!(base_len("C:\\a\\b"), 6);
        assert_eq!(base_len("C:/"), 2);

        // 测试双斜杠根路径
        assert_eq!(base_len("//"), 2);

        // 测试空路径
        assert_eq!(base_len(""), 0);
    }

    #[test]
    fn test_file_system_prefix_len() {
        // 测试带设备前缀的路径
        assert_eq!(file_system_prefix_len("C:"), 2);
        assert_eq!(file_system_prefix_len("c:"), 2);
        assert_eq!(file_system_prefix_len("Z:/path"), 2);

        // 测试不带设备前缀的路径
        assert_eq!(file_system_prefix_len("/path"), 0);
        assert_eq!(file_system_prefix_len("path"), 0);
        assert_eq!(file_system_prefix_len(""), 0);
    }

    #[test]
    fn test_has_device() {
        // 有效设备前缀
        assert!(has_device("C:"));
        assert!(has_device("c:"));
        assert!(has_device("Z:/path"));

        // 无效设备前缀
        assert!(!has_device("1:"));
        assert!(!has_device(":"));
        assert!(!has_device("/path"));
        assert!(!has_device(""));
    }
}
