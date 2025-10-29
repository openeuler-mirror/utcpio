/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use std::process;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use crate::fatal::*;
use crate::paxexit_status::*;

use gnu::argp::*;
use gnu::error::*;

pub const PAXEXIT_SUCCESS: i32 = 0;
pub const PAXEXIT_DIFFERS: i32 = 1;
pub const PAXEXIT_FAILURE: i32 = 2;

type ErrorHook = Option<fn()>;
pub static mut ERROR_HOOK: ErrorHook = None;

use lazy_static::lazy_static; // Add this line at the top of your file

lazy_static! {
    pub static ref RMT_DEV_NAME: Mutex<Option<String>> = Mutex::new(None);
    pub static ref FORCE_LOCAL_OPTION: AtomicBool = AtomicBool::new(false);
}

pub fn set_rmt_dev_name(name: Option<String>) {
    *RMT_DEV_NAME.lock().unwrap() = name;
}

pub fn get_rmt_dev_name() -> Option<String> {
    RMT_DEV_NAME.lock().unwrap().clone()
}

pub fn set_error_hook(hook: Option<fn()>) {
    unsafe {
        ERROR_HOOK = hook;
    }
}
#[allow(non_snake_case)]
pub fn WARN(errno: i32, args: std::fmt::Arguments) {
    unsafe {
        if let Some(hook) = ERROR_HOOK {
            hook();
        }
    }
    error(0, errno, args);
}
#[allow(non_snake_case)]
pub fn ERROR(errno: i32, args: std::fmt::Arguments) {
    unsafe {
        if let Some(hook) = ERROR_HOOK {
            hook();
        }
        set_exit_status(PAXEXIT_FAILURE);
    }
    error(0, errno, args);
    set_exit_status(PAXEXIT_FAILURE);
}
#[allow(non_snake_case)]
pub fn FATAL_ERROR(errno: i32, args: std::fmt::Arguments) {
    unsafe {
        if let Some(hook) = ERROR_HOOK {
            hook();
        }
    }
    error(0, errno, args);
    fatal_exit();
}

pub fn usage(err: i32) {
    let program_name: String = get_program_name().unwrap_or_default();

    eprintln!(
        "Try '{} --help' or '{} --usage' for more information.",
        program_name.as_str(),
        program_name.as_str()
    );

    process::exit(err);
}

#[allow(non_snake_case)]
pub fn USAGE_ERROR(errno: i32, args: std::fmt::Arguments) {
    unsafe {
        if let Some(hook) = ERROR_HOOK {
            hook();
        }
    }

    error(0, errno, args);
    usage(PAXEXIT_FAILURE);
}
