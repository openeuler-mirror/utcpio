/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::process;

use crate::paxlib::*;
pub fn fatal_exit() {
    process::exit(PAXEXIT_FAILURE);
}
