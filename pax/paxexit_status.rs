/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::clone_on_copy)]

use std::sync::Mutex;

use crate::paxlib::*;

lazy_static::lazy_static! {
    pub static ref exit_status: Mutex<i32> = Mutex::new(PAXEXIT_SUCCESS);
}

pub fn get_exit_status() -> i32 {
    exit_status.lock().unwrap().clone()
}
pub fn set_exit_status(value: i32) {
    *exit_status.lock().unwrap() = value;
}
