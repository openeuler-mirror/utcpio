/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

 use nix::errno::Errno::*;
 use std::fs::File;
 use std::io::{self, BufRead, Read, Write};
 use std::os::unix::io::{FromRawFd, RawFd};
 use std::path::{Path, PathBuf};
 use std::str::FromStr;
 use std::sync::Mutex;
 use std::{env, process};
 
 use crate::io::SeekFrom;
 use crate::process::exit;
 
 use gnu::progname::*;
 use gnu::util::validate_and_sanitize_path;
 
 use clap::Parser;
 
 use gnu::argp_version_etc::*; //argp_version_setup;
 use gnu::full_write::*;
 
 use nix::errno::*;
 use nix::fcntl::{open, OFlag};
 use nix::libc::{self, c_void};
 use nix::sys::stat::Mode;
 use nix::unistd::lseek;
 use nix::unistd::{close, Whence};
 
 use lazy_static::lazy_static;
 
 
 
 pub enum DebugFile {
     DebugFileOption = 256,
 }
 
 // 全局变量使用static mut，但在实际使用时应考虑使用互斥锁
 lazy_static! {
     static ref DBGLEV: Mutex<i32> = Mutex::new(0);
     static ref DBGOUT: Mutex<Option<File>> = Mutex::new(None);
 }
 
 #[derive(Parser, Debug)]
 #[clap(
     author,
     version,
     about,
     long_about = "Manipulate a tape drive, accepting commands from a remote process"
 )]
 struct Args {
     /// Set debug level
     #[clap(short, long, value_parser, help = "Set debug level")]
     debug: Option<i32>,
 
     /// Set debug output file name
     #[clap(long, value_parser, help = "Set debug output file name")]
     debug_file: Option<String>,
 }
 
 // #[repr(C)]
 // pub struct mtop {
 //     pub mt_op: i32,
 //     pub mt_count: i32,
 // }
 
 // #[repr(C)]
 // pub struct mtget {
 //     pub mt_type: c_long,
 //     pub mt_resid: c_long,
 //     pub mt_dsreg: c_long,
 //     pub mt_gstat: c_long,
 //     pub mt_erreg: c_long,
 //     pub mt_fileno: i32,
 //     pub mt_blkno: i32,
 // }
 
 // 使用 std::env::var 读取环境变量
 lazy_static! {
     static ref ARGP_PROGRAM_VERSION: String = format!(
         "rmt ({} {}) ",
         std::env::var("PACKAGE_NAME").unwrap_or_default(),
         std::env::var("VERSION").unwrap_or_default()
     );
     static ref ARGP_PROGRAM_BUG_ADDRESS: String = format!(
         "<{}>",
         std::env::var("PACKAGE_BUGREPORT").unwrap_or_default()
     );
 }
 
 macro_rules! DEBUG {
     ($lev:expr, $msg:expr) => {
         if let Some(ref mut f) = *DBGOUT.lock().unwrap() {
             if $lev <= *DBGLEV.lock().unwrap() {
                 write!(f, "{}", $msg).unwrap();
             }
         }
     };
 }
 
 macro_rules! DEBUG1 {
     ($lev:expr, $fmt:expr, $x:expr) => {
         if let Some(ref mut f) = *DBGOUT.lock().unwrap() {
             if $lev <= *DBGLEV.lock().unwrap() {
                 write!(f, $fmt, $x).unwrap();
             }
         }
     };
 }
 
 macro_rules! DEBUG2 {
     ($lev:expr, $fmt:expr, $x1:expr, $x2:expr) => {
         if let Some(ref mut f) = *DBGOUT.lock().unwrap() {
             if $lev <= *DBGLEV.lock().unwrap() {
                 write!(f, $fmt, $x1, $x2).unwrap();
             }
         }
     };
 }
 
 macro_rules! VDEBUG {
     ($lev:expr, $pfx:expr, $($arg:tt)*) => {{
             if let Some(ref mut f) = *DBGOUT.lock().unwrap() {
                 if $lev <= *DBGLEV.lock().unwrap() {
                     //writeln!(f, "{}{}", $pfx, format_args!($($arg)*)).unwrap();
                     writeln!(f, "{}{}", $pfx, format!($($arg)*));
                 }
             }
     }};
 }
 
 fn trimnl(s: &mut String) {
     if s.ends_with('\n') {
         s.pop();
     }
 }
 
 fn rmt_read() -> Option<String> {
     let stdin = io::stdin();
     let mut input_buf = String::new();
 
     match stdin.lock().read_line(&mut input_buf) {
         Ok(n) if n > 0 => {
             DEBUG1!(10, "C: {:?}", input_buf);
             trimnl(&mut input_buf);
             Some(input_buf)
         }
         _ => {
             DEBUG!(10, "reached EOF");
             None
         }
     }
 }
 
 use std::fmt::Write as FmtWrite;
 
 fn rmt_write(fmt: &str, args: std::fmt::Arguments) {
     let mut s = String::new();
     s.write_fmt(format_args!("{}", fmt)).unwrap(); // Format the string
 
     print!("{}", s); // Print to stdout
     std::io::stdout().flush().unwrap(); // Flush stdout
 
     VDEBUG!(10, "S: {}", "{}", s); // Assuming VDEBUG is a macro
 }
 
 fn rmt_reply(code: u64) {
     rmt_write("A{}\n", format_args!("{}", code));
 }
 
 fn rmt_error_message(code: Errno, msg: &str) {
     DEBUG1!(10, "S: E{}\n", code);
     DEBUG1!(10, "S: {}\n", msg);
     DEBUG1!(1, "error: {}\n", msg);
     println!("E{}\n{}", code as i32, msg);
     io::stdout().flush().unwrap();
 }
 
 fn rmt_error(code: Errno) {
     rmt_error_message(code, &code.to_string());
 }
 
 // 全局缓冲区
 lazy_static! {
     static ref RECORD_BUFFER_PTR: Mutex<Vec<u8>> = Mutex::new(Vec::new());
 }
 
 fn prepare_record_buffer(size: usize) {
     let mut buffer = RECORD_BUFFER_PTR.lock().unwrap();
 
     let current_capacity = buffer.capacity();
 
     if size > buffer.capacity() {
         buffer.reserve(size - current_capacity);
     }
     buffer.clear(); // 清空旧数据
     buffer.resize(size, 0); // 设置缓冲区长度并用 0 填充
 }
 
 static DEVICE_FD: Mutex<i32> = Mutex::new(-1);
 
 struct RmtKw {
     name: &'static str,
     len: usize,
     value: i32,
 }
 
 macro_rules! RMT_KW {
     ($s:ident, $v:expr) => {
         RmtKw {
             name: stringify!($s),
             len: stringify!($s).len(),
             value: $v,
         }
     };
 }
 
 fn xlat_kw<'a>(
     mut s: &'a str,
     pfx: Option<&str>,
     kw: &[RmtKw],
     valp: &mut i32,
     endp: &mut Option<&'a str>,
 ) -> i32 {
     let mut slen = s.len();
 
     // 处理前缀
     if let Some(prefix) = pfx {
         let pfxlen = prefix.len();
         if slen > pfxlen && s.starts_with(prefix) {
             s = &s[pfxlen..];
             slen -= pfxlen;
         }
     }
 
     // 遍历关键字
     for entry in kw {
         if slen >= entry.len
             && s.starts_with(entry.name)
             && !s
                 .chars()
                 .nth(entry.len)
                 .map_or(false, |c| c.is_alphanumeric())
         {
             *valp = entry.value;
             *endp = Some(&s[entry.len..]);
             return 0;
         }
     }
     1
 }
 
 fn skip_ws(s: &str) -> &str {
     s.trim_start()
 }
 
 static OPEN_FLAG_KW: &[RmtKw] = &[
     RMT_KW!(APPEND, libc::O_APPEND),
     RMT_KW!(CREAT, libc::O_CREAT),
     RMT_KW!(DSYNC, libc::O_DSYNC),
     RMT_KW!(EXCL, libc::O_EXCL),
     RMT_KW!(LARGEFILE, libc::O_LARGEFILE),
     RMT_KW!(NOCTTY, libc::O_NOCTTY),
     RMT_KW!(NONBLOCK, libc::O_NONBLOCK),
     RMT_KW!(RDONLY, libc::O_RDONLY),
     RMT_KW!(RDWR, libc::O_RDWR),
     RMT_KW!(RSYNC, libc::O_RSYNC),
     RMT_KW!(SYNC, libc::O_SYNC),
     RMT_KW!(TRUNC, libc::O_TRUNC),
     RMT_KW!(WRONLY, libc::O_WRONLY),
 ];
 
 fn decode_open_flag(mstr: &str, pmode: &mut i32) -> i32 {
     let mut mstr = skip_ws(mstr);
     let mut numeric_mode = 0;
     let mut mode = 0;
 
     // Parse initial numeric mode if present
     if let Some(first_char) = mstr.chars().next() {
         if first_char.is_ascii_digit() {
             let num_end = mstr
                 .find(|c: char| !c.is_ascii_digit())
                 .unwrap_or(mstr.len());
             match i32::from_str(&mstr[..num_end]) {
                 Ok(num) => {
                     numeric_mode = num;
                     mstr = skip_ws(&mstr[num_end..]);
                 }
                 Err(_) => {
                     rmt_error_message(EINVAL, "invalid numeric mode");
                     return 1;
                 }
             }
         }
     }
 
     if !mstr.is_empty() {
         while !mstr.is_empty() {
             let mut v = 0;
             mstr = skip_ws(mstr);
 
             if mstr.is_empty() {
                 break;
             } else if mstr.chars().next().unwrap().is_ascii_digit() {
                 let num_end = mstr
                     .find(|c: char| !c.is_ascii_digit())
                     .unwrap_or(mstr.len());
                 match i32::from_str(&mstr[..num_end]) {
                     Ok(num) => {
                         v = num;
                         mstr = &mstr[num_end..];
                     }
                     Err(_) => {
                         rmt_error_message(EINVAL, "invalid numeric mode");
                         return 1;
                     }
                 }
             } else {
                 let mut endp = None; // 用于存储剩余字符串
                 let found = xlat_kw(mstr, Some("O_"), &OPEN_FLAG_KW, &mut v, &mut endp);
                 if found != 0 {
                     rmt_error_message(EINVAL, "invalid open mode");
                     return 1;
                 }
                 mstr = endp.unwrap_or(mstr); // 更新 mstr 为剩余部分
             }
 
             mode |= v;
 
             mstr = skip_ws(mstr);
             if mstr.is_empty() {
                 break;
             }
 
             if mstr.starts_with('|') {
                 mstr = &mstr[1..];
             } else {
                 rmt_error_message(EINVAL, "invalid open mode");
                 return 1;
             }
         }
     } else {
         mode = numeric_mode;
     }
 
     *pmode = mode;
     0
 }
 
 fn open_device(device_path: &str) {
     // 验证和清理路径
     let safe_device_path = match validate_and_sanitize_path(device_path) {
         Ok(path) => path,
         Err(_) => {
             rmt_error_message(EINVAL, "Invalid or unsafe device path");
             return;
         }
     };
 
     // 读取标志字符串
     let flag_str = match rmt_read() {
         Some(s) => s,
         None => {
             DEBUG!(1, "unexpected EOF");
             exit(-1); // 使用 exit 更简洁
         }
     };
 
     let mut flag = 0;
     if decode_open_flag(&flag_str, &mut flag) == 0 {
         // 使用作用域限制锁的生命周期，确保及时释放
         {
             let mut fd_guard = DEVICE_FD.lock().unwrap(); // 获取可变锁
             let fd = *fd_guard;
             if fd >= 0 {
                 if let Err(err) = close(fd as RawFd) {
                     rmt_error(err);
                     return; // 提前返回，避免后续操作
                 }
                 *fd_guard = -1; // 重置文件描述符
             }
         } // 锁在这里自动释放
 
         // 打开新设备
         match open(
             &safe_device_path,
             OFlag::from_bits_truncate(flag),
             Mode::from_bits_truncate(0o666),
         ) {
             Ok(fd) => {
                 let mut fd_guard = DEVICE_FD.lock().unwrap(); // 获取锁
                 *fd_guard = fd; // 更新文件描述符
                 rmt_reply(0);
             }
             Err(err) => {
                 rmt_error(err);
             }
         }
     }
 }
 
 fn close_device() {
     let mut fd_guard = DEVICE_FD.lock().unwrap(); // 获取锁
     let fd = *fd_guard;
     if fd >= 0 {
         let raw_fd = fd as RawFd;
         match close(raw_fd) {
             Ok(_) => {
                 *fd_guard = -1; // 更新文件描述符
                 rmt_reply(0);
             }
             Err(err) => {
                 rmt_error(err);
             }
         }
     }
 }
 
 const SEEK_WHENCE_KW: &[RmtKw] = &[
     RMT_KW! { SET, libc::SEEK_SET },
     RMT_KW! { CUR, libc::SEEK_CUR },
     RMT_KW! { END, libc::SEEK_END },
 ];
 fn lseek_device(seek_str: &str) {
     let whence = if seek_str.len() == 1 {
         match seek_str.chars().next().unwrap() {
             '0' => SeekFrom::Start(0),
             '1' => SeekFrom::Current(0),
             '2' => SeekFrom::End(0),
             _ => {
                 rmt_error_message(EINVAL, "Seek direction out of range");
                 return;
             }
         }
     } else {
         let mut whence_val = 0;
         let mut endp = None; // 用于存储剩余字符串
         let found = xlat_kw(
             seek_str,
             Some("SEEK_"),
             SEEK_WHENCE_KW,
             &mut whence_val,
             &mut endp,
         );
         if found != 0 {
             rmt_error_message(EINVAL, "Invalid seek direction");
             return;
         }
         match whence_val {
             libc::SEEK_SET => SeekFrom::Start(0),
             libc::SEEK_CUR => SeekFrom::Current(0),
             libc::SEEK_END => SeekFrom::End(0),
             _ => {
                 rmt_error_message(EINVAL, "Invalid seek direction");
                 return;
             }
         }
     };
 
     let offset_str = match rmt_read() {
         Some(s) => s,
         None => return,
     };
 
     let offset = match offset_str.parse::<i64>() {
         Ok(n) => n,
         Err(_) => {
             rmt_error_message(EINVAL, "Invalid seek offset");
             return;
         }
     };
 
     let nix_whence = match whence {
         std::io::SeekFrom::Start(offset) => {
             offset as i64; // 确保 offset 类型正确
             Whence::SeekSet
         }
         std::io::SeekFrom::End(offset) => {
             offset as i64;
             Whence::SeekEnd
         }
         std::io::SeekFrom::Current(offset) => {
             offset as i64;
             Whence::SeekCur
         }
     };
 
     // No need to check offset range if using i64 and SeekFrom
 
     let fd_guard = DEVICE_FD.lock().unwrap(); // Acquire the lock
     let fd = *fd_guard;
 
     if fd < 0 {
         rmt_error_message(EBADF, "Bad file descriptor");
         return;
     }
 
     let raw_fd: i32 = fd as RawFd;
 
     match lseek(raw_fd, offset, nix_whence) {
         Ok(new_offset) => rmt_reply(new_offset as u64),
         Err(err) => rmt_error(err),
     }
     // Lock is automatically released when fd_guard goes out of scope
 }
 
 fn read_device(str: &str) {
     // 解析字节数
     let size = match str.parse::<usize>() {
         Ok(n) => n,
         Err(_) => {
             rmt_error_message(EINVAL, "Invalid byte count");
             return;
         }
     };
 
     // 检查大小是否在有效范围内
     if size as u64 != size as u64 {
         rmt_error_message(EINVAL, "Byte count out of range");
         return;
     }
 
     // 准备缓冲区
     prepare_record_buffer(size);
 
     let fd_guard = DEVICE_FD.lock().unwrap(); // Acquire file descriptor lock
     let fd = *fd_guard;
 
     let mut buffer = RECORD_BUFFER_PTR.lock().unwrap();
     buffer.resize(size, 0);
 
     let raw_fd = fd as RawFd; // Cast to RawFd
 
     // 读取数据
     match nix::unistd::read(raw_fd, &mut buffer) {
         Ok(status) => {
             // 发送成功响应
             rmt_reply(status as u64);
             // 将数据写入标准输出
             let stdout = io::stdout();
             let mut handle = stdout.lock();
             if let Err(_) = handle.write_all(&buffer[..status]) {
                 rmt_error(Errno::EIO);
                 return;
             }
             if let Err(_) = handle.flush() {
                 rmt_error(Errno::EIO);
                 return;
             }
         }
         Err(err) => {
             rmt_error(err);
         }
     }
 }
 
 fn write_device(str: &str) {
     // 解析字节数
     let size = match str.parse::<usize>() {
         Ok(n) => n,
         Err(_) => {
             rmt_error_message(EINVAL, "Invalid byte count");
             return;
         }
     };
 
     // 检查大小是否在有效范围内
     if size as u64 != size as u64 {
         rmt_error_message(EINVAL, "Byte count out of range");
         return;
     }
 
     // 准备缓冲区
     prepare_record_buffer(size);
 
     // 从标准输入读取数据
     let mut stdin = std::io::stdin();
     let mut buffer = vec![0u8; size];
     if let Err(e) = stdin.read_exact(&mut buffer) {
         if e.kind() == std::io::ErrorKind::UnexpectedEof {
             rmt_error_message(EIO, "Premature EOF");
         } else {
             rmt_error(Errno::from_i32(e.raw_os_error().unwrap_or(EIO as i32)));
         }
         return;
     }
 
     // 使用已经定义的 full_write 函数来写入设备
     let fd_guard = DEVICE_FD.lock().unwrap(); // Acquire file descriptor lock
     let mut fd = *fd_guard;
 
     if fd < 0 {
         rmt_error_message(Errno::EBADF, "Bad file descriptor");
         return;
     }
 
     // let mut file = unsafe { File::from_raw_fd(fd as RawFd) };
 
     match full_write(&mut fd, &buffer, size) {
         Ok(bytes_written) => {
             rmt_reply(bytes_written as u64);
         }
         Err(err) => {
             rmt_error(Errno::from_i32(err.raw_os_error().unwrap_or(EIO as i32)));
         }
     }
 }
 
 fn iocop_device(str: &str) {
     // 解析操作码 (opcode)
     let opcode = match str.parse::<i32>() {
         Ok(opcode) if opcode >= 0 => opcode,
         _ => {
             rmt_error_message(EINVAL, "Invalid operation code");
             return;
         }
     };
 
     // 读取字节数 (count)
     let count_str = match rmt_read() {
         Some(s) => s,
         None => return,
     };
 
     let count = match count_str.parse::<i32>() {
         Ok(count) if count >= 0 => count, // Combined parse and range check
         Ok(_) => {
             // Handle the case where parsing succeeds but count is negative
             rmt_error_message(EINVAL, "Byte count out of range");
             return;
         }
         Err(err) => {
             rmt_error_message(EINVAL, &format!("Invalid byte count: {}", err));
             return;
         }
     };
 
     #[cfg(MTIOCTOP)]
     {
         let mtop = mtop {
             mt_op: opcode,
             mt_count: count,
         };
 
         if mtop.mt_count != count {
             rmt_error_message(EINVAL as i32, "Byte count out of range");
             return;
         }
 
         // 执行 ioctl 操作
         if unsafe { ioctl(DEVICE_FD, MTIOCTOP, &mtop) } < 0 {
             rmt_error(errno());
         } else {
             rmt_reply(0);
         }
     }
 
     #[cfg(not(MTIOCTOP))]
     {
         rmt_error_message(ENOSYS, "Operation not supported");
     }
 }
 fn status_device(str: &str) {
     // Check for unexpected arguments
 
     if !str.is_empty() {
         rmt_error_message(EINVAL, "Unexpected arguments");
         return;
     }
 
     // let mut mtget = mtget {
     //     mt_type: 0,
     //     mt_resid: 0,
     //     mt_dsreg: 0,
     //     mt_gstat: 0,
     //     mt_erreg: 0,
     //     mt_fileno: 0,
     //     mt_blkno: 0,
     // };
 
     #[cfg(feature = "mtio")]
     {
         let mut mtget = unsafe { std::mem::zeroed::<mtget>() };
 
         let device_fd = unsafe {
             /* 你需要获取设备文件描述符 */
             0
         }; // 需要替换
 
         if unsafe {
             ioctl(
                 device_fd,
                 MTIOCGET,
                 &mut mtget as *mut _ as *mut std::ffi::c_void,
             )
         } < 0
         {
             rmt_error(io::Error::last_os_error());
         } else {
             rmt_reply(std::mem::size_of::<mtget>());
             unsafe {
                 full_write(
                     STDOUT_FILENO,
                     &mtget as *const _ as *const u8,
                     std::mem::size_of::<mtget>(),
                 );
             }
         }
     }
 
     #[cfg(not(feature = "mtio"))]
     {
         rmt_error_message(ENOSYS, "Operation not supported");
     }
 }
 
 // Authors information
 pub const RMT_AUTHORS: &'static [&'static str] = &["Zhang Haidong"];
 
 // Replace xalloc_die with a Rust version
 pub fn xalloc_die() -> ! {
     rmt_error(ENOMEM);
     process::exit(1);
 }
 
 fn cleanup_buffers() {}
 fn main() {
     let args = Args::parse();
 
     let mut stop = false;
 
     set_program_name(&env::args().collect::<Vec<String>>()[0].clone());
     argp_version_setup(Some("rmt"), Some(RMT_AUTHORS));
 
     // if unsafe { isatty(STDOUT_FILENO) } != 0 {
     //     setlocale(LocaleCategory::LcAll, "");
     //     textdomain(PACKAGE).unwrap();
     // }
 
     if let Some(level) = args.debug {
         *DBGLEV.lock().unwrap() = level;
     }
 
     if let Some(file_path) = args.debug_file {
         match File::create(&file_path) {
             Ok(file) => {
                 *DBGOUT.lock().unwrap() = Some(file);
                 *DBGLEV.lock().unwrap() = 1;
             }
             Err(err) => {
                 eprintln!("cannot open {}: {}", file_path, err);
                 process::exit(1);
             }
         }
     }
 
     // Main command processing loop
     while !stop {
         match rmt_read() {
             Some(buf) => {
                 if let Some(first_char) = buf.chars().next() {
                     let command_arg = &buf[1..];
                     match first_char {
                         'C' => {
                             close_device();
                             stop = true;
                         }
                         'I' => {
                             iocop_device(command_arg);
                         }
                         'L' => {
                             lseek_device(command_arg);
                         }
                         'O' => {
                             open_device(command_arg);
                         }
                         'R' => {
                             read_device(command_arg);
                         }
                         'S' => {
                             status_device(command_arg);
                         }
                         'W' => {
                             write_device(command_arg);
                         }
                         _ => {
                             DEBUG1!(1, "garbage input {}", buf);
                             rmt_error_message(EINVAL, "Garbage command");
                             process::exit(1);
                         }
                     }
                 }
             }
             None => break,
         }
     }
 
     // Cleanup
     let fd_guard = DEVICE_FD.lock().unwrap();
     let fd = *fd_guard;
 
     if fd >= 0 {
         close_device();
     }
 
     cleanup_buffers();
 }
