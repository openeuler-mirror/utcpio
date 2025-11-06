/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

mod filemode;
mod filetype;
mod global;
mod idcache;
mod initramfs;
mod userspec;

use global::*;
use initramfs::*;
use userspec::*;
fn main() {
    println!("Hello, world!");
}
