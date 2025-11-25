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

static mut CURRENT_TIME: timespec = timespec {
    tv_sec: 0,
    tv_nsec: 0,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DelayedLinkKey {
    pub dev: u64, // dev_t
    pub ino: u64, // ino_t
}

#[derive(Debug, Clone)]
struct DelayedLinkValue {
    pub mode: u32,  // mode_t
    pub uid: u32,   // uid_t
    pub gid: u32,   // gid_t
    pub mtime: i64, // time_t, representing seconds since epoch
    pub source: String,
    pub target: String,
}

struct DelayedLink {
    table: HashMap<DelayedLinkKey, DelayedLinkValue>,
}

impl DelayedLink {
    fn new() -> Self {
        DelayedLink {
            table: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
    // fn dl_hash(&self, entry: &DelayedLinkKey, table_size: usize) -> usize {
    //     let n = entry.dev;
    //     let nshift = (mem::size_of::<u64>() - mem::size_of::<u64>()) * 8; // CHAR_BIT is 8
    //     let shifted_n = if nshift > 0 { n << nshift } else { n };
    //     (shifted_n ^ entry.ino) as usize % table_size
    // }

    // fn dl_compare(&self, a: &DelayedLinkKey, b: &DelayedLinkKey) -> bool {
    //     a.dev == b.dev && a.ino == b.ino
    // }
    // fn get_first(&self) -> Option<(&DelayedLinkKey, &DelayedLinkValue)> {
    //     self.table.iter().next()
    // }

    // fn get_next<'a>(
    //     &'a self,
    //     current_key: &'a DelayedLinkKey,
    // ) -> Option<(&'a DelayedLinkKey, &'a DelayedLinkValue)> {
    //     let mut iter = self.table.iter();
    //     while let Some((key, _)) = iter.next() {
    //         if key == current_key {
    //             return iter.next();
    //         }
    //     }
    //     None
    // }

    fn insert(&mut self, key: DelayedLinkKey, value: DelayedLinkValue) {
        self.table.insert(key, value);
    }

    // fn get(&self, key: &DelayedLinkKey) -> Option<&DelayedLinkValue> {
    //     self.table.get(key)
    // }

    // fn remove(&mut self, key: &DelayedLinkKey) -> Option<DelayedLinkValue> {
    //     self.table.remove(key)
    // }
}

pub fn process_copy_in() -> io::Result<()> {
    

    Ok(())
}