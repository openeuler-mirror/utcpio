/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::too_many_arguments, clippy::char_lit_as_u8)]

use std::sync::Mutex;

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub enum QuotingStyle {
    Literal = 0,
    Shell = 1,
    ShellAlways = 2,
    ShellEscape = 3,
    ShellEscapeAlways = 4,
    C = 5,
    CMaybe = 6,
    Escape = 7,
    Locale = 8,
    CLocale = 9,
    Custom = 10,
}

bitflags::bitflags! {
    #[repr(C)]
    pub struct QuotingFlags: i32 {
        const ELIDE_OUTER_QUOTES = 1 << 0;
        const ELIDE_NULL_BYTES = 1 << 1;
        const SPLIT_TRIGRAPHS = 1 << 2;
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct QuotingOptions {
    style: QuotingStyle,
    flags: QuotingFlags,
    quote_these_too: [u32; (u8::MAX as usize / 32) + 1],
    left_quote: String,
    right_quote: String,
}

lazy_static::lazy_static! {
    pub static ref DEFAULT_QUOTING_OPTIONS: Mutex<QuotingOptions> = Mutex::new(QuotingOptions {
        style: QuotingStyle::Literal,
        flags: QuotingFlags::empty(),
        quote_these_too: [0; (u8::MAX as usize / 32) + 1],
        left_quote: String::new(),
        right_quote: String::new(),
    });
}

pub fn clone_quoting_options(o: Option<&QuotingOptions>) -> Box<QuotingOptions> {
    let default_options = DEFAULT_QUOTING_OPTIONS.lock().unwrap();
    let src = o.unwrap_or(&default_options);
    Box::new(src.clone())
}

pub fn get_quoting_style(o: Option<&QuotingOptions>) -> QuotingStyle {
    o.unwrap_or(&DEFAULT_QUOTING_OPTIONS.lock().unwrap()).style
}

pub fn set_quoting_style(o: Option<&mut QuotingOptions>, s: QuotingStyle) {
    if let Some(opt) = o {
        opt.style = s;
    } else {
        let mut default = DEFAULT_QUOTING_OPTIONS.lock().unwrap();
        default.style = s;
    }
}

pub fn set_char_quoting(o: Option<&mut QuotingOptions>, c: u8, i: i32) -> i32 {
    let mut default_options = DEFAULT_QUOTING_OPTIONS.lock().unwrap();
    let opt = o.unwrap_or(&mut default_options);
    let idx = c as usize / 32;
    let shift = c % 32;
    let r = (opt.quote_these_too[idx] >> shift) & 1;
    opt.quote_these_too[idx] ^= (((i & 1) as u32) ^ r) << shift;
    r as i32
}

pub fn set_quoting_flags(o: Option<&mut QuotingOptions>, i: i32) -> i32 {
    let mut default_options = DEFAULT_QUOTING_OPTIONS.lock().unwrap();
    let opt = o.unwrap_or(&mut default_options);
    let r = opt.flags.bits();
    opt.flags = QuotingFlags::from_bits_truncate(i);
    r
}

pub fn set_custom_quoting(o: Option<&mut QuotingOptions>, left: &str, right: &str) {
    let mut default_options = DEFAULT_QUOTING_OPTIONS.lock().unwrap();
    let opt = o.unwrap_or(&mut default_options);
    opt.style = QuotingStyle::Custom;
    opt.left_quote = left.to_string();
    opt.right_quote = right.to_string();
}
