/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

pub fn gnu_function() {
    println!("This is a function from libpax.");
}

pub mod fatal;
pub mod paxexit;
pub mod paxexit_status;
pub mod paxlib;
pub mod paxnames;
pub mod paxerror;
