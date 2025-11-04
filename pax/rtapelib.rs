/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(
    clippy::non_minimal_cfg,
    clippy::redundant_pattern_matching,
    clippy::match_result_ok,
    clippy::needless_borrow,
    clippy::unnecessary_cast,
    clippy::partialeq_to_none
)]

use gnu::basename_lgpl::*;
use gnu::error::*;
use gnu::full_write::*;
use gnu::safe_read::*;
use gnu::xmalloc::*;

use crate::sysdep::*;

use nix::unistd::close;

use lazy_static::lazy_static;
use std::io;
use std::io::SeekFrom;
use std::net::{IpAddr, ToSocketAddrs};
use std::os::unix::io::RawFd;
use std::process::Command;
use std::str;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

const MTIOCTOP: u64 = 1;
const MTIOCGET: u64 = 2;

const EXIT_ON_EXEC_ERROR: i32 = 128; // Exit status if exec errors.
const COMMAND_BUFFER_SIZE: usize = 64; // Size of buffers for reading and writing commands to rmt.
const MAXUNIT: usize = 4; // Maximum number of simultaneous remote tape connections.
const PREAD: usize = 0; // read file descriptor from pipe()
const PWRITE: usize = 1; // write file descriptor from pipe()
const REMOTE_SHELL: &str = "/usr/bin/ssh";
const RMT_COMMAND: &str = "rmt";

const STDIN_FILENO: u32 = 0;
const STDOUT_FILENO: u32 = 1;
// const STDERR_FILENO: u32 = 2;

// The pipes for receiving data from remote tape drives.
lazy_static! {
    static ref FROM_REMOTE: Mutex<[[RawFd; 2]; MAXUNIT]> =
        Mutex::new([[-1, -1], [-1, -1], [-1, -1], [-1, -1]]);
    static ref TO_REMOTE: Mutex<[[RawFd; 2]; MAXUNIT]> =
        Mutex::new([[-1, -1], [-1, -1], [-1, -1], [-1, -1]]);
}

// Return the parent's read side of remote tape connection Fd.
fn read_side(fd_index: usize) -> RawFd {
    match FROM_REMOTE.lock() {
        Ok(from_remote) => from_remote[fd_index][PREAD],
        Err(_) => -1,
    }
}

fn write_side(fd_index: usize) -> RawFd {
    match TO_REMOTE.lock() {
        Ok(to_remote) => to_remote[fd_index][PWRITE],
        Err(_) => -1,
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Mtop {
    pub mt_op: libc::c_short,
    pub mt_count: libc::c_int,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct Mtget {
    pub mt_type: libc::c_long,
    pub mt_resid: libc::c_long,
    pub mt_dsreg: libc::c_long,
    pub mt_gstat: libc::c_long,
    pub mt_erreg: libc::c_long,
    pub mt_fileno: libc::c_long,
    pub mt_blkno: libc::c_long,
}

fn gethostbyname(hostname: &str) -> Result<Vec<IpAddr>, std::io::Error> {
    let socket_addrs_iter = (hostname, 0).to_socket_addrs()?;
    let ip_addrs: Vec<IpAddr> = socket_addrs_iter
        .filter_map(|addr| {
            let ip = addr.ip();
            if let IpAddr::V4(_) = ip {
                Some(ip)
            } else {
                None
            }
        })
        .collect();
    Ok(ip_addrs)
}

static SIGPIPE_IGNORED: AtomicBool = AtomicBool::new(false);
fn signal_sigpipe_ignore(ignore: bool) -> bool {
    let previous = SIGPIPE_IGNORED.load(Ordering::Relaxed);
    SIGPIPE_IGNORED.store(ignore, Ordering::Relaxed);
    previous
}

fn rmt_shutdown(handle: usize, errno_value: i32) {
    if let Ok(mut from_remote) = FROM_REMOTE.lock() {
        // 关闭文件描述符
        let _ = close(from_remote[handle][PREAD]);
        // 设置为无效 FD（-1）
        from_remote[handle][PREAD] = -1;
    }

    if let Ok(mut to_remote) = TO_REMOTE.lock() {
        // 关闭文件描述符
        let _ = close(to_remote[handle][PWRITE]);
        // 设置为无效 FD（-1）
        to_remote[handle][PWRITE] = -1;
    }

    set_errno(errno_value);
}

fn do_command(handle: usize, buffer: &str) -> i32 {
    let length = buffer.len();
    let pipe_handler = signal_sigpipe_ignore(true);

    let mut fd = write_side(handle);
    let written_result = full_write(&mut fd, buffer.as_bytes(), length);
    signal_sigpipe_ignore(pipe_handler);

    match written_result {
        Ok(written) if written == length => 0,
        _ => {
            rmt_shutdown(handle, EIO);
            -1
        }
    }
}

fn get_status_string(handle: usize, command_buffer: &mut [u8]) -> Result<String, io::Error> {
    let mut cursor = 0;

    let read_fd = read_side(handle);
    for _ in 0..COMMAND_BUFFER_SIZE {
        let mut byte = [0u8; 1];
        if safe_read(read_fd, &mut byte, 1).is_err() {
            rmt_shutdown(handle, EIO);
            return Err(io::Error::from_raw_os_error(EIO));
        }
        command_buffer[cursor] = byte[0];
        if byte[0] == b'\n' {
            command_buffer[cursor] = 0;
            break;
        }
        cursor += 1;
    }

    if cursor == COMMAND_BUFFER_SIZE {
        rmt_shutdown(handle, libc::EIO);
        return Err(io::Error::from_raw_os_error(EIO));
    }

    let command_str = match str::from_utf8(&command_buffer[..cursor]) {
        Ok(s) => s,
        Err(_) => {
            rmt_shutdown(handle, libc::EIO);
            return Err(io::Error::from_raw_os_error(libc::EIO));
        }
    };
    let cursor_str = command_str.trim_start();

    if cursor_str.starts_with('E') || cursor_str.starts_with('F') {
        let mut byte = [0u8; 1];
        while safe_read(read_fd, &mut byte, 1).is_ok() {
            if byte[0] == b'\n' {
                break;
            }
        }

        let errno_value = cursor_str[1..].parse::<i32>().unwrap_or(libc::EIO);
        if cursor_str.starts_with('F') {
            rmt_shutdown(handle, errno_value);
        }
        return Err(io::Error::from_raw_os_error(EIO));
    }

    if !cursor_str.starts_with('A') {
        rmt_shutdown(handle, libc::EIO);
        return Err(io::Error::from_raw_os_error(EIO));
    }

    Ok(cursor_str[1..].to_string())
}

fn get_status(handle: usize) -> i64 {
    let mut command_buffer = [0u8; COMMAND_BUFFER_SIZE];

    match get_status_string(handle, &mut command_buffer) {
        Ok(status_str) => {
            if let Some(result) = status_str.trim_start().parse::<i64>().ok() {
                if result >= 0 {
                    return result;
                } else {
                    set_errno(libc::EIO);
                }
            } else {
                set_errno(libc::EIO);
            }
        }
        Err(_) => {
            set_errno(libc::EIO);
        }
    }
    -1
}

fn get_status_off(handle: usize) -> i64 {
    let mut command_buffer = [0u8; COMMAND_BUFFER_SIZE];

    match get_status_string(handle, &mut command_buffer) {
        Ok(status_str) => {
            let mut status_chars = status_str
                .trim_start()
                .chars()
                .skip_while(|&c| c == ' ' || c == '\t');
            let negative = match status_chars.clone().next() {
                Some('-') => {
                    status_chars.next();
                    true
                }
                Some('+') => {
                    status_chars.next();
                    false
                }
                _ => false,
            };

            let mut count: i64 = 0;
            for c in status_chars {
                if let Some(digit) = c.to_digit(10) {
                    let c10 = 10 * count;
                    let nc = if negative {
                        c10 - digit as i64
                    } else {
                        c10 + digit as i64
                    };
                    if c10 / 10 != count || (negative && c10 < nc) || (!negative && nc < c10) {
                        return -1;
                    }
                    count = nc;
                } else {
                    break;
                }
            }
            count
        }
        Err(_) => -1,
    }
}

fn encode_oflag(buf: &mut Vec<u8>, oflag: i32) {
    let temp_buf = format!("{} ", oflag);

    buf.extend_from_slice(temp_buf.as_bytes());

    match oflag & libc::O_ACCMODE {
        libc::O_RDONLY => buf.extend_from_slice(b"O_RDONLY"),
        libc::O_RDWR => buf.extend_from_slice(b"O_RDWR"),
        libc::O_WRONLY => buf.extend_from_slice(b"O_WRONLY"),
        _ => panic!("Invalid O_ACCMODE value"),
    }

    if oflag & libc::O_APPEND != 0 {
        buf.extend_from_slice(b"|O_APPEND");
    }
    if oflag & libc::O_CREAT != 0 {
        buf.extend_from_slice(b"|O_CREAT");
    }
    if oflag & libc::O_DSYNC != 0 {
        buf.extend_from_slice(b"|O_DSYNC");
    }
    if oflag & libc::O_EXCL != 0 {
        buf.extend_from_slice(b"|O_EXCL");
    }

    #[cfg(any(target_os = "linux"))]
    {
        if libc::O_LARGEFILE != 0 && oflag != 0 {
            buf.extend_from_slice(b"|O_LARGEFILE");
        }
    }

    if oflag & libc::O_NOCTTY != 0 {
        buf.extend_from_slice(b"|O_NOCTTY");
    }
    if oflag & libc::O_NONBLOCK != 0 {
        buf.extend_from_slice(b"|O_NONBLOCK");
    }
    if oflag & libc::O_RSYNC != 0 {
        buf.extend_from_slice(b"|O_RSYNC");
    }
    if oflag & libc::O_SYNC != 0 {
        buf.extend_from_slice(b"|O_SYNC");
    }
    if oflag & libc::O_TRUNC != 0 {
        buf.extend_from_slice(b"|O_TRUNC");
    }
}

fn sys_reset_uid_gid() -> Option<String> {
    let uid = get_uid();
    let gid = get_gid();

    let username = get_pwuid(uid)?;

    match initgroups(&username, gid) {
        Ok(_) => {
            if gid != get_egid() && set_gid(gid).is_err() {
                return Some("setgid".to_string());
            }

            if uid != get_euid() && set_uid(uid).is_err() {
                return Some("setuid".to_string());
            }
            None
        }
        Err(_) => Some("initgroups".to_string()),
    }
}

fn redirect_files(remote_pipe_number: usize) -> io::Result<()> {
    let to_remote = match TO_REMOTE.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to lock TO_REMOTE",
            ))
        }
    };
    let from_remote = match FROM_REMOTE.lock() {
        Ok(guard) => guard,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to lock FROM_REMOTE",
            ))
        }
    };

    let to_read = to_remote[remote_pipe_number][PREAD];
    let to_write = to_remote[remote_pipe_number][PWRITE];
    let from_read = from_remote[remote_pipe_number][PREAD];
    let from_write = from_remote[remote_pipe_number][PWRITE];

    use nix::unistd::{close, dup2};

    dup2(to_read, STDIN_FILENO as RawFd)?;
    if to_read != STDIN_FILENO as RawFd {
        close(to_read)?;
    }
    if to_write != STDIN_FILENO as RawFd {
        close(to_write)?;
    }

    dup2(from_write, STDOUT_FILENO as RawFd)?;
    close(from_read)?;
    close(from_write)?;

    Ok(())
}

pub fn rmt_open__(file_name: &str, open_mode: i32, bias: i32, remote_shell: Option<&str>) -> i32 {
    let mut remote_pipe_number = MAXUNIT;

    for i in 0..MAXUNIT {
        if read_side(i) == -1 && write_side(i) == -1 {
            remote_pipe_number = i;
            break;
        }
    }

    if remote_pipe_number == MAXUNIT {
        set_errno(libc::EMFILE);
        return -1;
    }

    let mut file_name_copy = xstrdup(file_name);
    let remote_host = file_name_copy.clone();
    let mut remote_user = None;
    let mut remote_file = None;

    let mut cursor = 0;
    while cursor < file_name_copy.len() {
        match file_name_copy.as_bytes()[cursor] {
            b'\n' => {
                set_errno(libc::ENOENT);
                return -1;
            }
            b'\0' => {
                // 检查空字节（null byte）
                set_errno(libc::ENOENT);
                return -1;
            }
            b'@' if remote_user.is_none() => {
                remote_user = Some(file_name_copy[0..cursor].to_string());
                file_name_copy.replace_range(cursor..cursor + 1, "\0");
                file_name_copy = file_name_copy[cursor + 1..].to_string();
                cursor = 0;
            }
            b':' if remote_file.is_none() => {
                remote_file = Some(file_name_copy[cursor + 1..].to_string());
                file_name_copy.replace_range(cursor..cursor + 1, "\0");
                file_name_copy = file_name_copy[0..cursor].to_string();
                cursor = 0;
            }
            _ => cursor += 1,
        }
    }

    let remote_file = match remote_file {
        Some(file) => file,
        None => {
            set_errno(libc::ENOENT);
            return -1;
        }
    };

    // 验证远程文件路径的安全性
    if remote_file.is_empty() {
        set_errno(libc::ENOENT);
        return -1;
    }

    // 检查远程文件路径是否包含危险的遍历序列
    if remote_file.contains("..") {
        set_errno(libc::ENOENT);
        return -1;
    }

    // 检查远程文件路径是否以绝对路径开始
    if remote_file.starts_with('/') {
        set_errno(libc::ENOENT);
        return -1;
    }

    // 检查远程文件路径长度
    if remote_file.len() > 4096 {
        set_errno(libc::ENAMETOOLONG);
        return -1;
    }

    if gethostbyname(&remote_host).is_err() {
        eprintln!("Cannot connect to {}: resolve failed", remote_host);
        return -1;
    }

    let remote_user = remote_user.filter(|user| !user.is_empty());

    let remote_shell = remote_shell.unwrap_or(REMOTE_SHELL);
    let remote_shell_basename = last_component(remote_shell);

    let (to_read, to_write) = create_pipe();
    let (from_read, from_write) = create_pipe();

    if to_read == -1 || from_read == -1 {
        set_errno(EIO);
        return -1;
    }
    {
        if let Ok(mut to_remote) = TO_REMOTE.lock() {
            to_remote[remote_pipe_number][PWRITE] = to_write;
            to_remote[remote_pipe_number][PREAD] = to_read;
        }
    }

    {
        if let Ok(mut from_remote) = FROM_REMOTE.lock() {
            from_remote[remote_pipe_number][PWRITE] = from_write;
            from_remote[remote_pipe_number][PREAD] = from_read;
        }
    }

    match std::process::Command::new(REMOTE_SHELL).spawn() {
        Ok(child) => {
            if child.id() == 0 {
                // Child process
                if let Err(e) = redirect_files(remote_pipe_number) {
                    error(
                        EXIT_ON_EXEC_ERROR,
                        e.raw_os_error().unwrap_or(0),
                        format_args!("Cannot redirect files"),
                    );
                    return -1;
                }

                if sys_reset_uid_gid() == None {
                    error(
                        EXIT_ON_EXEC_ERROR,
                        -1,
                        format_args!("Cannot reset uid and gid"),
                    );
                    return -1;
                }

                let mut command = Command::new(remote_shell_basename);
                command.arg(remote_host);

                if let Some(user) = remote_user {
                    command.arg("-l").arg(user);
                }

                command.arg(RMT_COMMAND);

                if let Err(e) = command.status() {
                    error(
                        EXIT_ON_EXEC_ERROR,
                        e.raw_os_error().unwrap_or(0),
                        format_args!("Cannot execute remote shell"),
                    );

                    return -1;
                }
                return 0;
            } else {
                child.id() as i32
            }
        }
        Err(_e) => {
            set_errno(EIO);

            return -1;
        }
    };
    {
        let _remote_file_len = remote_file.len();

        let mut command_buffer = format!("O{}\n", remote_file);

        let oflag_buffer = String::new();
        encode_oflag(&mut oflag_buffer.as_bytes().to_vec(), open_mode);
        command_buffer.push_str(&String::from_utf8_lossy(&oflag_buffer.as_bytes()));
        command_buffer.push('\n');

        if do_command(remote_pipe_number, &command_buffer) == -1
            || get_status(remote_pipe_number) == -1
        {
            rmt_shutdown(remote_pipe_number, 0 as i32);
            return -1;
        }
    }
    remote_pipe_number as i32 + bias
}

pub fn rmt_close__(handle: usize) -> i32 {
    if do_command(handle, "C\n") == -1 {
        return -1;
    }

    let status = get_status(handle);

    rmt_shutdown(handle, 0);

    status as i32
}

pub fn rmt_read__(handle: usize, buffer: &mut [u8], length: usize) -> usize {
    let command_buffer = format!("R{}\n", length);
    let status: usize;
    let mut counter: usize;

    if do_command(handle, &command_buffer) == -1 || {
        status = get_status(handle) as usize;
        status == SAFE_READ_ERROR || status > length
    } {
        return SAFE_READ_ERROR;
    }

    counter = 0;
    while counter < status {
        match safe_read(handle as RawFd, &mut buffer[counter..], status - counter) {
            Ok(0) => {
                rmt_shutdown(handle, 1); // 模拟 EIO
                return SAFE_READ_ERROR;
            }
            Ok(bytes_read) => {
                counter += bytes_read;
            }
            Err(_e) => {
                rmt_shutdown(handle, 1); // 模拟 EIO
                return SAFE_READ_ERROR;
            }
        }
    }

    status
}

pub fn rmt_write__(handle: usize, buffer: &[u8], length: usize) -> usize {
    let command_buffer = format!("W{}\n", length);

    if do_command(handle, &command_buffer) == -1 {
        return 0;
    }

    let pipe_handler = signal_sigpipe_ignore(true);
    let write_result = full_write(&mut (handle as RawFd), buffer, length);
    signal_sigpipe_ignore(pipe_handler);

    let written = match write_result {
        Ok(w) if w == length => {
            let r = get_status(handle);
            if r < 0 {
                return 0;
            }
            if r as usize == length {
                return length;
            }
            r as usize
        }
        Ok(w) => w,
        Err(_) => {
            rmt_shutdown(handle, 1);
            return 0;
        }
    };

    rmt_shutdown(handle, 1); // 模拟 EIO
    written
}

pub fn rmt_lseek__(handle: usize, offset: i64, whence: SeekFrom) -> i64 {
    let whence_num = match whence {
        SeekFrom::Start(_) => 0,
        SeekFrom::Current(_) => 1,
        SeekFrom::End(_) => 2,
    };

    let command_buffer = format!("L{}\n{}\n", offset, whence_num);

    if do_command(handle, &command_buffer) == -1 {
        return -1;
    }

    get_status_off(handle)
}

pub fn rmt_ioctl__(handle: usize, operation: u64, argument: &mut [u8]) -> i32 {
    match operation {
        MTIOCTOP => {
            let mtop = unsafe { &*(argument.as_ptr() as *const Mtop) };
            let command_buffer = format!("I{}\n{}\n", mtop.mt_op, mtop.mt_count);
            if do_command(handle, &command_buffer) == -1 {
                return -1;
            }
            get_status(handle) as i32
        }
        MTIOCGET => {
            let mut status = 0;
            if do_command(handle, "S") == -1 || {
                status = get_status(handle);
                status == -1
            } {
                return -1;
            }
            if status as usize > argument.len() {
                set_errno(EOVERFLOW);
                return -1;
            }
            let mut p = 0;
            while (status as usize) > p {
                match safe_read(handle as RawFd, &mut argument[p..], status as usize - p) {
                    Ok(counter) if counter > 0 => {
                        p += counter;
                        status -= counter as i64;
                    }
                    _ => {
                        rmt_shutdown(handle, EIO);
                        return -1;
                    }
                }
            }
            let mtget = unsafe { &*(argument.as_ptr() as *const Mtget) };
            if mtget.mt_type < 256 {
                return 0;
            }
            for i in 0..(status as usize / 2) {
                argument.swap(i * 2, i * 2 + 1);
            }
            0
        }
        _ => {
            set_errno(EOPNOTSUPP);
            -1
        }
    }
}
