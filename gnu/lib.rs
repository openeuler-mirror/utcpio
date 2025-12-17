// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

pub fn gnu_function() {
    println!("This is a function from libgnu.");
}

pub mod argp;
pub mod argp_version_etc;
pub mod basename_lgpl;
pub mod dirname;
pub mod dirname_lgpl;
pub mod error;
pub mod fdutimensat;
pub mod full_write;
pub mod gettime;
pub mod intprops;
pub mod progname;
pub mod quotearg;
pub mod safe_read;
pub mod safe_write;
pub mod stripslash;
pub mod umaxtostr;
pub mod util;
pub mod version_etc;
pub mod xalloc_die;
pub mod xmalloc;
