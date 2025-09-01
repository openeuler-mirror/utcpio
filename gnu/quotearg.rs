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

fn quoting_options_from_style(style: QuotingStyle) -> QuotingOptions {
    if style == QuotingStyle::Custom {
        panic!("Custom style requires left and right quotes");
    }
    QuotingOptions {
        style,
        flags: QuotingFlags::empty(),
        quote_these_too: [0; (u8::MAX as usize / 32) + 1],
        left_quote: String::new(),
        right_quote: String::new(),
    }
}

fn quotearg_buffer_restyled(
    buffer: &mut Vec<u8>,
    buffersize: usize,
    arg: &str,
    argsize: usize,
    style: QuotingStyle,
    flags: QuotingFlags,
    quote_these_too: &[u32; (u8::MAX as usize / 32) + 1],
    left_quote: &[u8],
    right_quote: &[u8],
) -> usize {
    let mut len = 0;
    let elide_outer_quotes = flags.contains(QuotingFlags::ELIDE_OUTER_QUOTES);
    let mut backslash_escapes = false;
    let mut quote_string = None;
    let mut quote_string_len = 0;

    macro_rules! store {
        ($c:expr) => {{
            if len < buffersize {
                if len < buffer.len() {
                    buffer[len] = $c as u8;
                } else {
                    buffer.push($c as u8);
                }
            }
            len += 1;
        }};
    }
    match style {
        QuotingStyle::C => {
            if !elide_outer_quotes {
                store!('"');
            }
            backslash_escapes = true;
            quote_string = Some("\"");
            quote_string_len = 1;
        }
        QuotingStyle::CMaybe => {
            return quotearg_buffer_restyled(
                buffer,
                buffersize,
                arg,
                argsize,
                QuotingStyle::C,
                flags | QuotingFlags::ELIDE_OUTER_QUOTES,
                quote_these_too,
                left_quote,
                right_quote,
            );
        }
        QuotingStyle::Escape => {
            backslash_escapes = true;
        }
        QuotingStyle::ShellAlways => {
            if !elide_outer_quotes {
                store!('\'');
            }
            quote_string = Some("'");
            quote_string_len = 1;
        }
        QuotingStyle::Shell => {
            return quotearg_buffer_restyled(
                buffer,
                buffersize,
                arg,
                argsize,
                QuotingStyle::ShellAlways,
                flags | QuotingFlags::ELIDE_OUTER_QUOTES,
                quote_these_too,
                left_quote,
                right_quote,
            );
        }
        QuotingStyle::ShellEscape => {
            // backslash_escapes = true;
            return quotearg_buffer_restyled(
                buffer,
                buffersize,
                arg,
                argsize,
                QuotingStyle::Shell,
                flags,
                quote_these_too,
                left_quote,
                right_quote,
            );
        }
        QuotingStyle::ShellEscapeAlways => {
            // backslash_escapes = true;
            return quotearg_buffer_restyled(
                buffer,
                buffersize,
                arg,
                argsize,
                QuotingStyle::ShellAlways,
                flags,
                quote_these_too,
                left_quote,
                right_quote,
            );
        }
        QuotingStyle::Literal => {}
        QuotingStyle::Locale | QuotingStyle::CLocale => {
            let lq = if left_quote.is_empty() {
                "`"
            } else {
                std::str::from_utf8(left_quote).unwrap_or("`")
            };
            let rq = if right_quote.is_empty() {
                "'"
            } else {
                std::str::from_utf8(right_quote).unwrap_or("`")
            };
            if !elide_outer_quotes {
                for c in lq.bytes() {
                    store!(c);
                }
            }
            backslash_escapes = true;
            quote_string_len = rq.len();
            quote_string = Some(rq);
        }
        QuotingStyle::Custom => {
            let lq = std::str::from_utf8(left_quote).unwrap_or("`");
            let rq = std::str::from_utf8(right_quote).unwrap_or("`");
            if !elide_outer_quotes {
                for c in lq.bytes() {
                    store!(c);
                }
            }
            backslash_escapes = true;
            quote_string = Some(rq);
            quote_string_len = rq.len();
        }
    }

    let arg_bytes = arg.as_bytes();
    let argsize = if argsize == usize::MAX {
        arg.len()
    } else {
        argsize
    };
    for i in 0..argsize {
        let c = arg_bytes[i];
        if backslash_escapes
            && quote_string_len > 0
            && i + quote_string_len <= arg.len()
            && &arg[i..i + quote_string_len] == quote_string.unwrap()
        {
            if elide_outer_quotes {
                panic!("Force outer quoting not implemented");
            }
            store!('\\');
        }

        match c as char {
            '\0' => {
                if backslash_escapes {
                    store!('\\');
                    store!('0');
                } else if flags.contains(QuotingFlags::ELIDE_NULL_BYTES) {
                    continue;
                }
            }
            '\n' => {
                if backslash_escapes {
                    store!('\\');
                    store!('n');
                } else {
                    store!(c);
                }
            }
            '"' if style == QuotingStyle::C => {
                store!('\\');
                store!(c);
            }
            '\'' if style == QuotingStyle::ShellAlways => {
                store!('\'');
                store!('\\');
                store!('\'');
            }
            c if backslash_escapes
                && quote_these_too[c as usize / 32] & (1 << ((c as usize) % 32)) != 0 =>
            {
                store!('\\');
                store!(c);
            }
            _ => store!(c),
        }
    }

    if let Some(qs) = quote_string {
        if !elide_outer_quotes {
            for c in qs.bytes() {
                store!(c);
            }
        }
    }

    if len < buffersize {
        buffer.resize(len + 1, 0);
        buffer[len] = 0;
    }
    len
}
