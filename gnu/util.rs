/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::io;
use std::path::{Path, PathBuf};

pub fn validate_and_sanitize_path(path_str: &str) -> io::Result<PathBuf> {
    // 检查空路径
    if path_str.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Empty path"));
    }

    // 检查是否包含换行符（可能用于命令注入）
    if path_str.contains('\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path contains newline",
        ));
    }

    // 检查是否包含空字节（null byte）
    if path_str.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path contains null byte",
        ));
    }

    // 创建路径对象
    let path = Path::new(path_str);

    // 检查路径是否包含危险的遍历序列
    let path_components: Vec<_> = path.components().collect();

    // 检查是否包含 ".." 组件（路径遍历攻击）
    if path_components
        .iter()
        .any(|comp| matches!(comp, std::path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path traversal not allowed",
        ));
    }

    // // 检查是否以绝对路径开始（可能绕过安全限制）
    // if path.is_absolute() {
    //     return Err(io::Error::new(
    //         io::ErrorKind::InvalidInput,
    //         "Absolute paths not allowed"
    //     ));
    // }

    // 规范化路径
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // 额外的安全检查：确保路径不会太长
    if canonical_path.to_string_lossy().len() > 4096 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Path too long"));
    }

    Ok(canonical_path)
}
