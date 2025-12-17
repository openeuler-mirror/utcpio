// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

use std::process;

pub fn xalloc_die() {
    eprintln!("Error: memory exhausted");

    process::abort();
}
