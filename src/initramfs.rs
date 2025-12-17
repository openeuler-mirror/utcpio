// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

pub const METADATA_FILENAME: &str = "METADATA!!!";

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum MetadataTypes {
    TypeNone,
    TypeXattr,
}

#[repr(C, packed)] // 使用 repr(C) 和 packed 确保结构体布局与 C 相同
pub struct MetadataHdr {
    pub c_size: [u8; 8],       // 使用固定大小的数组代替 char c_size[8]
    pub c_version: u8,         // 使用 u8 代替 char
    pub c_type: MetadataTypes, // 使用 u8 代替 char
    pub c_metadata: [u8; 0],   // 使用零大小数组代替 char c_metadata[]
}
impl Default for MetadataHdr {
    fn default() -> Self {
        Self {
            c_version: 1,
            c_type: MetadataTypes::TypeXattr,
            c_size: [0; 8],
            c_metadata: [0; 0],
        }
    }
}
