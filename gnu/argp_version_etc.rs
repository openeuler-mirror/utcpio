/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use crate::argp::*;
use crate::version_etc::*;
use std::io::Write;
use std::sync::OnceLock;

static PROGRAM_CANONICAL_NAME: OnceLock<&'static str> = OnceLock::new();
static PROGRAM_AUTHORS: OnceLock<&'static [&'static str]> = OnceLock::new();

fn version_etc_hook(stream: Option<&mut dyn Write>, _state: &mut ArgpState) {
    if let Some(stream) = stream {
        version_etc(
            stream,
            PROGRAM_CANONICAL_NAME.get().copied(),
            PACKAGE_NAME,
            VERSION,
            PROGRAM_AUTHORS.get().copied().unwrap_or(&[]),
        );
    }
}

/// Set up the version information for Argp.
pub fn argp_version_setup(name: Option<&'static str>, authors: Option<&'static [&'static str]>) {
    ARGP_PROGRAM_VERSION_HOOK.set(version_etc_hook).unwrap();

    if let Some(name) = name {
        PROGRAM_CANONICAL_NAME.set(name).unwrap();
    }
    if let Some(authors) = authors {
        PROGRAM_AUTHORS.set(authors).unwrap();
    }
}
