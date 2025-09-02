/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use crate::basename_lgpl::*;

pub fn strip_trailing_slashes(file: &mut String) -> bool {
    let base = last_component(file);

    let base_start = if base.is_empty() {
        0
    } else {
        file.rfind(base).unwrap()
    };

    let base_lim = base_start + base_len(base);
    let had_slash = file.len() > base_lim;

    file.truncate(base_lim);
    had_slash
}
