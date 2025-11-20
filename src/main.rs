/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

mod cpiohdr;
mod dstring;
mod externs;
mod filemode;
mod filetype;
mod global;
mod idcache;
mod initramfs;
mod userspec;
mod appargs;
mod util;
mod copyin;

use global::*;
use initramfs::*;
use userspec::*;
fn main() {
    println!("Hello, world!");
}
