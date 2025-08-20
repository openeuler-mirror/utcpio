/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::type_complexity, clippy::derivable_impls)]

/*
存放arg 相关的函数
如果是 const char*  对应 Option<&'static str>
如果是 char*  对应 String
如果是 void*  对应 Box<dyn std::any::Any>
如果是 int  对应 i32
如果是 unsigned int  对应 u32
如果是 unsigned long  对应 u64
如果是指针类型， 需要有Option<>


*/
use std::io::Write;
use std::sync::Mutex;
use std::sync::OnceLock;

use libc::c_long; // 引入 Write trait

pub const OPTION_ARG_OPTIONAL: i32 = 0x1;
pub const OPTION_HIDDEN: i32 = 0x2;
pub const OPTION_ALIAS: i32 = 0x4;
pub const OPTION_DOC: i32 = 0x8;
pub const OPTION_NO_USAGE: i32 = 0x10;
pub const OPTION_NO_TRANS: i32 = 0x20;
pub const ARGP_ERR_UNKNOWN: i32 = libc::E2BIG; // Hurd should never need E2BIG. XXX
pub const ARGP_KEY_ARG: i32 = 0;
pub const ARGP_KEY_ARGS: i32 = 0x1000006;
pub const ARGP_KEY_END: i32 = 0x1000001;
pub const ARGP_KEY_NO_ARGS: i32 = 0x1000002;
pub const ARGP_KEY_INIT: i32 = 0x1000003;

pub const PACKAGE_NAME: &str = "GNU cpio";
pub const PACKAGE: &str = "cpio";
pub const PACKAGE_BUGREPORT: &str = "bug-cpio@gnu.org";
pub const VERSION: &str = "2.14";
// const LC_ALL: i32 = 0; // 需要定义 LC_ALL 常量
// const LOCALEDIR: &str = "/usr/share/locale"; // 替换为你的消息目录路径

lazy_static::lazy_static! {
    pub static ref ARGP_PROGRAM_VERSION: Mutex<Option<String>> = Mutex::new(None);
    pub static ref ARGP_PROGRAM_BUG_ADDRESS: Mutex<Option<String>> = Mutex::new(None);
    pub static ref PROGRAM_NAME: Mutex<Option<String>> = Mutex::new(None);
    pub static ref PROGRAM_INVOCATION_NAME: Mutex<Option<String>> = Mutex::new(None);
    pub static ref PROGRAM_INVOCATION_SHORT_NAME: Mutex<Option<String>> = Mutex::new(None);
}

pub fn get_argp_program_version() -> Option<String> {
    ARGP_PROGRAM_VERSION.lock().unwrap().clone()
}
pub fn set_argp_program_version(version: Option<String>) {
    *ARGP_PROGRAM_VERSION.lock().unwrap() = version;
}
pub fn get_argp_program_bug_address() -> Option<String> {
    ARGP_PROGRAM_BUG_ADDRESS.lock().unwrap().clone()
}
pub fn set_argp_program_bug_address(address: Option<String>) {
    *ARGP_PROGRAM_BUG_ADDRESS.lock().unwrap() = address;
}
pub fn get_program_name() -> Option<String> {
    PROGRAM_NAME.lock().unwrap().clone()
}
pub fn bind_program_name(name: Option<String>) {
    *PROGRAM_NAME.lock().unwrap() = name;
}
pub fn get_program_invocation_name() -> Option<String> {
    PROGRAM_INVOCATION_NAME.lock().unwrap().clone()
}
pub fn set_program_invocation_name(name: Option<String>) {
    *PROGRAM_INVOCATION_NAME.lock().unwrap() = name;
}
pub fn get_program_invocation_short_name() -> Option<String> {
    PROGRAM_INVOCATION_SHORT_NAME.lock().unwrap().clone()
}
pub fn set_program_invocation_short_name(name: Option<String>) {
    *PROGRAM_INVOCATION_SHORT_NAME.lock().unwrap() = name;
}

type ArgpParserFn = fn(key: i32, arg: Option<&str>, state: &mut ArgpState) -> i32;
type ArgpProgramVersionHookFn = fn(Option<&mut dyn Write>, &mut ArgpState);

pub static ARGP_PROGRAM_VERSION_HOOK: OnceLock<ArgpProgramVersionHookFn> = OnceLock::new();

pub struct ArgpOption {
    pub name: Option<&'static str>,
    pub key: i32,
    pub arg: Option<&'static str>,
    pub flags: i32,
    pub doc: Option<&'static str>,
    pub group: i32,
}

pub struct Argp<'a> {
    pub options: Option<&'static [ArgpOption]>,
    pub parser: Option<ArgpParserFn>,
    pub args_doc: Option<&'static str>,
    pub doc: Option<&'static str>,
    pub children: Option<&'a [ArgpChild<'a>]>,
    pub help_filter: Option<fn(i32, &str, *mut std::ffi::c_void) -> Option<String>>,
    pub argp_domain: Option<&'static str>,
}

pub struct ArgpParser<'a> {
    pub argp: Option<&'a Argp<'a>>,
    pub short_opts: String,
    pub long_opts: Vec<OptExtOption>,
    pub opt_data: GetOptData,
    pub groups: Vec<Group<'a>>,
    pub egroup: Vec<Group<'a>>,
    pub child_inputs: Vec<Box<dyn std::any::Any>>,
    pub try_getopt: bool,
    pub state: ArgpState<'a>,
    pub storage: Vec<u8>,
}
pub struct OptExtOption {
    pub name: String,
    pub has_arg: i32,           // Use an enum for clarity and type safety
    pub flag: Option<*mut i32>, // Use Option to handle potentially null flag
    pub val: i32,
}
pub struct GetOptData {
    pub optind: i32,
    pub opterr: i32,
    pub optopt: i32,
    pub optarg: Option<String>,
    pub __initialized: bool,
    pub __nextchar: String,
    pub __ordering: ArgpOrdering,
    pub __first_nonopt: i32,
    pub __last_nonopt: i32,
}

impl Default for GetOptData {
    fn default() -> Self {
        GetOptData {
            optind: 1,
            opterr: 1, // or 0 depending on your use case
            __nextchar: String::new(),
            __first_nonopt: 0,
            __last_nonopt: 0,
            __ordering: ArgpOrdering::Permute,
            __initialized: false,
            optarg: None,
            optopt: 0,
        }
    }
}

#[derive(PartialEq)]
pub enum ArgpOrdering {
    RequireOrder,
    Permute,
    ReturnInOrder,
}

// When an argp has a non-zero CHILDREN field, it should point to a vector of
// argp_child structures, each of which describes a subsidiary argp.
pub struct ArgpChild<'a> {
    pub argp: Option<&'a Argp<'a>>, //使用一个切片就可以了
    pub flags: i32,
    pub header: Option<String>,
    pub group: i32,
}

pub struct Group<'a> {
    pub parser: Option<ArgpParserFn>,
    pub argp: Option<&'a Argp<'a>>,
    pub short_end: String, // 使用 String 来存储 short_end
    pub args_processed: u32,
    pub parent: Option<&'a Group<'a>>,
    pub parent_index: usize,
    pub input: Option<Box<dyn std::any::Any>>, // 使用 Box 和 trait object 来存储 input
    pub child_inputs: Vec<Box<dyn std::any::Any>>, // 使用 Vec 来存储 child_inputs
    pub hook: Option<Box<dyn std::any::Any>>,  // 使用 Box 和 trait object 来存储 hook
}

// Parsing state. This is provided to parsing functions called by argp,
// which may examine and, as noted, modify fields.
pub struct ArgpState<'a> {
    // The top level ARGP being parsed.
    pub root_argp: Option<&'a Argp<'a>>,
    pub argc: i32,
    pub argv: Vec<String>,
    pub next: i32,
    pub flags: u32,
    pub arg_num: u32,
    pub quoted: i32,
    pub input: Option<Box<dyn std::any::Any>>,
    pub child_inputs: Vec<Box<dyn std::any::Any>>,
    pub hook: Option<Box<dyn std::any::Any>>,
    pub name: String,
    pub err_stream: Option<&'a mut dyn Write>, // For errors; initialized to stderr.
    pub out_stream: Option<&'a mut dyn Write>, // For information; initialized to stdout.
    pub pstate: Option<Vec<u8>>,               // Private, for use by argp.
}

impl<'a> Default for ArgpState<'a> {
    fn default() -> Self {
        ArgpState {
            root_argp: None,
            argc: 0,
            argv: Vec::new(),
            next: 0,
            flags: 0,
            arg_num: 0,
            quoted: 0,
            input: None,
            child_inputs: Vec::new(),
            hook: None,
            name: String::new(),
            err_stream: None,
            out_stream: None,
            pstate: None,
        }
    }
}

#[repr(C)]
pub struct mtop {
    pub mt_op: i32,
    pub mt_count: i32,
}

#[repr(C)]
pub struct mtget {
    pub mt_type: c_long,
    pub mt_resid: c_long,
    pub mt_dsreg: c_long,
    pub mt_gstat: c_long,
    pub mt_erreg: c_long,
    pub mt_fileno: i32,
    pub mt_blkno: i32,
}

pub const ARGP_KEY_FINI: i32 = 0x1000007;
pub const ARGP_KEY_SUCCESS: i32 = 0x1000004;
pub const ARGP_KEY_ERROR: i32 = 0x1000005;

// Possible KEY arguments to a help filter function.
// const ARGP_KEY_HELP_PRE_DOC: u32 = 0x2000001; // Help text preceding options.
// const ARGP_KEY_HELP_POST_DOC: u32 = 0x2000002; // Help text following options.
// const ARGP_KEY_HELP_HEADER: u32 = 0x2000003; // Option header string.
// const ARGP_KEY_HELP_EXTRA: u32 = 0x2000004; // After all other documentation; TEXT is NULL for this key.

// // Explanatory note emitted when duplicate option arguments have been suppressed.
// const ARGP_KEY_HELP_DUP_ARGS_NOTE: u32 = 0x2000005;
// const ARGP_KEY_HELP_ARGS_DOC: u32 = 0x2000006; // Argument doc string.

pub const ARGP_PARSE_ARGV0: u32 = 0x01;
pub const ARGP_NO_ERRS: u32 = 0x02;
pub const ARGP_NO_ARGS: u32 = 0x04;
pub const ARGP_IN_ORDER: u32 = 0x08;
pub const ARGP_NO_HELP: u32 = 0x10;
pub const ARGP_NO_EXIT: u32 = 0x20;
pub const ARGP_LONG_ONLY: u32 = 0x40;
pub const ARGP_SILENT: u32 = ARGP_NO_EXIT | ARGP_NO_ERRS | ARGP_NO_HELP;

// Flags for argp_help.
pub const ARGP_HELP_USAGE: u32 = 0x01; // a Usage: message.
pub const ARGP_HELP_SHORT_USAGE: u32 = 0x02; // " but don't actually print options.
pub const ARGP_HELP_SEE: u32 = 0x04; // a "Try ... for more help" message.
pub const ARGP_HELP_LONG: u32 = 0x08; // a long help message.
pub const ARGP_HELP_PRE_DOC: u32 = 0x10; // doc string preceding long help.
pub const ARGP_HELP_POST_DOC: u32 = 0x20; // doc string following long help.
pub const ARGP_HELP_DOC: u32 = ARGP_HELP_PRE_DOC | ARGP_HELP_POST_DOC; // Combined doc flags.
pub const ARGP_HELP_BUG_ADDR: u32 = 0x40; // bug report address
pub const ARGP_HELP_LONG_ONLY: u32 = 0x80; // modify output appropriately to reflect ARGP_LONG_ONLY mode.
pub const ARGP_HELP_EXIT_ERR: u32 = 0x100; // Call exit(1) instead of returning.
pub const ARGP_HELP_EXIT_OK: u32 = 0x200; // Call exit(0) instead of returning.
pub const ARGP_HELP_STD_ERR: u32 = ARGP_HELP_SEE | ARGP_HELP_EXIT_ERR;
pub const ARGP_HELP_STD_USAGE: u32 = ARGP_HELP_SHORT_USAGE | ARGP_HELP_SEE | ARGP_HELP_EXIT_ERR;

// The standard thing to do in response to a --help option.
pub const ARGP_HELP_STD_HELP: u32 =
    ARGP_HELP_SHORT_USAGE | ARGP_HELP_LONG | ARGP_HELP_EXIT_OK | ARGP_HELP_DOC | ARGP_HELP_BUG_ADDR;
