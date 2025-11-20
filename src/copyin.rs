/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(
    dead_code,
    clippy::needless_late_init,
    clippy::manual_memcpy,
    clippy::large_enum_variant,
    clippy::unnecessary_mut_passed,
    clippy::if_same_then_else,
    unused_mut
)]

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::str;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use chrono::{TimeZone, Utc};
use lazy_static::lazy_static;

use libc::{dev_t, fnmatch, lchown, symlink, timespec, umask, unlink};
use nix::sys::stat::fstat;
use nix::unistd::{Gid, Uid};

use pax::paxerror::*;
use pax::paxlib::PAXEXIT_FAILURE;
use pax::rmt::*;

use crate::appargs::*;
// use crate::copypass::*;
use crate::cpiohdr::*;
use crate::dstring::*;
use crate::externs::*;
use crate::filemode::*;
use crate::filetype::*;
use crate::filetype::{CP_IFBLK, CP_IFCHR, CP_IFIFO, CP_IFMT, CP_IFSOCK};
use crate::global::*;
use crate::idcache::*;
// use crate::tar::*;
use crate::util::*;

use gnu::error::*;
use gnu::gettime::*;
use gnu::quotearg::*;

pub fn process_copy_in() -> io::Result<()> {
    

    Ok(())
}