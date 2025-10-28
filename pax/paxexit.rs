/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::process;

use crate::paxexit_status::*;
pub fn pax_exit() {
    process::exit(get_exit_status());
}
