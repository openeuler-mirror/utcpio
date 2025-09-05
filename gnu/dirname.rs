/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */
use crate::dirname_lgpl::*;
use crate::xalloc_die::*;
pub fn dir_name(name: &str) -> Option<String> {
    let result = mdir_name(name);
    if result.is_empty() {
        xalloc_die();
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_name_success() {
        // 测试正常路径
        let result = dir_name("/path/to/file");
        assert!(result.is_some());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    #[should_panic]
    fn test_dir_name_empty() {
        // 测试空字符串，预期会panic
        dir_name("");
    }

    #[test]
    fn test_dir_name_root() {
        // 测试根路径
        let result = dir_name("/");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "/\0");
    }

    #[test]
    fn test_dir_name_no_slash() {
        // 测试没有斜杠的路径
        let result = dir_name("filename");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), ".\0");
    }
}
