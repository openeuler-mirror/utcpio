// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

#![allow(
    clippy::suspicious_open_options,
    clippy::unnecessary_mut_passed,
    unknown_lints,
    unused_assignments
)]

use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::FromRawFd;
use std::os::linux::fs::MetadataExt as LinuxMetadataExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use gnu::error::*;
use nix::libc::umask;
use pax::paxerror::stat_error;

use crate::appargs::*;
use crate::cpiohdr::*;
use crate::dstring::*;
use crate::externs::*;
use crate::filetype::*;
use crate::global::*;
use crate::util::*;

const AT_SYMLINK_NOFOLLOW: i32 = 4096;

fn set_copypass_perms(file: Option<&File>, name: &str, st: &mut fs::Metadata) {
    let mut header = CpioFileStat::new();
    header.set_c_name(name);
    //    header.c_name = String::from(name);
    stat_to_cpio(st, &mut header);
    set_perms(file, &mut header)
}

pub fn process_copy_pass() -> io::Result<()> {
    let mut input_name = DYNAMIC_STRING_INITIALIZER;
    let mut output_name = DYNAMIC_STRING_INITIALIZER;

    let mut input_tape: std::sync::MutexGuard<'_, TapeInput> = TAPE_INPUT.lock().map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to lock TAPE_INPUT: {}", e),
        )
    })?;
    let mut output_tape: std::sync::MutexGuard<'_, TapeOutput> =
        TAPE_OUTPUT.lock().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to lock TAPE_OUTPUT: {}", e),
            )
        })?;

    let mut existing_dir: bool;
    set_newdir_umask(unsafe { umask(0) });

    // 初始化输出路径
    let directory_name = get_directory_name().unwrap_or_default();
    let mut dirname_len = directory_name.len();

    if get_change_directory_option().is_some() && !directory_name.starts_with('/') {
        let pwd = std::env::current_dir()?;
        ds_concat(
            &mut output_name,
            pwd.to_path_buf().to_str().unwrap_or_default(),
        );
        ds_append(&mut output_name, b'/');
    }

    ds_concat(&mut output_name, directory_name.as_str());
    ds_append(&mut output_name, b'/');

    dirname_len = ds_len(&mut output_name);
    output_tape.output_is_seekable = true;

    // 改变工作目录
    change_dir();

    let mut stdin_file = unsafe { File::from_raw_fd(libc::STDIN_FILENO) };

    while ds_fgetstr(&mut stdin_file, &mut input_name, get_name_end() as u8).is_some() {
        let mut link_res = -1;

        if input_name.ds_string[0] == 0 {
            error(0, 0, format_args!("blank line ignored"));
            continue;
        }
        if input_name.ds_string[0] == b'.'
            && (input_name.ds_string.get(1) == Some(&b'\0')
                || (input_name.ds_string.get(1) == Some(&b'/')
                    && input_name.ds_string.get(2) == Some(&b'\0')))
        {
            continue;
        }

        let mut path_bytes = &input_name.ds_string[..input_name.ds_idx];
        let input_path = String::from_utf8_lossy(path_bytes).to_string();

        let mut in_file_stat = match fs::metadata(input_path.clone()) {
            Err(_) => {
                stat_error(input_path.as_str());
                continue;
            }
            Ok(stat) => stat,
        };

        let slash = &input_name.ds_string[..input_name.ds_idx];
        let mut slash_str = match std::str::from_utf8(slash) {
            Ok(s) => s,
            Err(_) => {
                error(0, 0, format_args!("invalid UTF-8 sequence in filename"));
                continue;
            }
        };

        while slash_str.starts_with("/") {
            slash_str = &slash_str[1..];
        }
        ds_reset(&mut output_name, dirname_len);
        ds_concat(&mut output_name, slash_str);

        existing_dir = false;

        path_bytes = &output_name.ds_string[..output_name.ds_idx];
        let output_path = String::from_utf8_lossy(path_bytes).to_string();

        if let Ok(out_file_stat) = fs::metadata(output_path.clone()) {
            if out_file_stat.is_dir() && in_file_stat.is_dir() {
                existing_dir = true;
            } else if !get_unconditional_flag()
                && in_file_stat.modified()? <= out_file_stat.modified()?
            {
                error(
                    0,
                    0,
                    format_args!(
                        "{} not created: newer or same age version exists",
                        output_path.clone()
                    ),
                );
                continue;
            } else if let Err(e) = if out_file_stat.is_dir() {
                fs::remove_dir(&output_path)
            } else {
                fs::remove_file(&output_path)
            } {
                error(
                    0,
                    e.raw_os_error().unwrap_or(0),
                    format_args!("cannot remove current {}", output_path),
                );
                continue;
            }
        }

        if s_isreg(in_file_stat.mode()) {
            if get_link_flag() {
                link_res = link_to_name(output_path.as_str(), input_path.as_str());
            }

            if link_res < 0 && in_file_stat.st_nlink() > 1 {
                link_res = link_to_maj_min_ino(
                    output_path.as_str(),
                    major(in_file_stat.st_dev() as u32) as u32,
                    minor(in_file_stat.st_dev() as u32) as u32,
                    in_file_stat.st_ino(),
                );
            }

            if link_res < 0 {
                // 打开输入文件
                let mut in_file_des = match File::open(&input_path) {
                    Ok(file) => file,
                    Err(e) => {
                        error(
                            0,
                            e.raw_os_error().unwrap_or(0),
                            format_args!("cannot open {}", input_path),
                        );
                        continue;
                    }
                };

                // 创建输出文件
                let mut out_file_des = match OpenOptions::new()
                    .create(true)
                    .write(true)
                    .mode(0o600)
                    .open(&output_path)
                {
                    Ok(file) => file,
                    Err(_e) if get_create_dir_flag() => {
                        create_all_directories(&output_path);
                        OpenOptions::new()
                            .create(true)
                            .write(true)
                            .mode(0o600)
                            .open(&output_path)?
                    }
                    Err(e) => {
                        error(
                            0,
                            e.raw_os_error().unwrap_or(0),
                            format_args!("cannot create {}", output_path),
                        );
                        continue;
                    }
                };

                // 复制文件内容
                copy_files_disk_to_disk(
                    &mut output_tape,
                    &mut input_tape,
                    &mut in_file_des,
                    &mut out_file_des,
                    in_file_stat.len() as i32,
                    &input_path,
                );

                // 清空输出缓冲区
                disk_empty_output_buffer(&mut output_tape, &mut out_file_des, true);

                // 设置文件权限
                set_copypass_perms(Some(&out_file_des), &output_path, &mut in_file_stat);

                // 重置文件时间
                if get_reset_time_flag() {
                    set_file_times(
                        Some(&in_file_des),
                        &input_path,
                        in_file_stat.mtime(),
                        in_file_stat.mtime(),
                        0,
                    );
                    set_file_times(
                        Some(&out_file_des),
                        &output_path,
                        in_file_stat.mtime(),
                        in_file_stat.mtime(),
                        0,
                    );
                }

                // 检查文件是否改变
                warn_if_file_changed(
                    &input_path,
                    in_file_stat.size(),
                    in_file_stat.mtime() as u64,
                );
            }
        } else if s_isdir(in_file_stat.mode()) {
            let mut file_stat = CpioFileStat::new();
            stat_to_cpio(&mut in_file_stat, &mut file_stat);
            file_stat.set_c_name(output_path.as_str());
            cpio_create_dir(&mut file_stat, existing_dir);
        } else if s_ischr(in_file_stat.mode())
            || s_isblk(in_file_stat.mode())
            || s_isfifo(in_file_stat.mode())
            || s_issock(in_file_stat.mode())
        {
            if get_link_flag() {
                link_res = link_to_name(&output_path, &input_path);
            }
            if link_res < 0 && in_file_stat.st_nlink() > 1 {
                link_res = link_to_maj_min_ino(
                    &output_path,
                    major(in_file_stat.st_dev() as u32) as u32,
                    minor(in_file_stat.st_dev() as u32) as u32,
                    in_file_stat.st_ino(),
                );
            }

            if link_res < 0 {
                let mut res: i32 = unsafe {
                    libc::mknod(
                        output_path.as_str().as_ptr() as *const libc::c_char,
                        in_file_stat.st_mode(),
                        in_file_stat.st_rdev(),
                    )
                };

                if res < 0 && get_create_dir_flag() {
                    create_all_directories(&output_path);
                    res = unsafe {
                        libc::mknod(
                            output_path.as_str().as_ptr() as *const libc::c_char,
                            in_file_stat.st_mode(),
                            in_file_stat.st_rdev(),
                        )
                    };
                }

                if res < 0 {
                    error(0, res, format_args!("cannot create {}", output_path));
                    continue;
                }
                set_copypass_perms(None, output_path.as_str(), &mut in_file_stat);
            } else if s_islnk(in_file_stat.mode()) {
                let link_name = match fs::read_link(&input_path) {
                    Ok(name) => name,
                    Err(e) => {
                        error(
                            0,
                            e.raw_os_error().unwrap_or(0),
                            format_args!("cannot read link {}", input_path),
                        );
                        continue;
                    }
                };
                let res = std::os::unix::fs::symlink(&link_name, &output_path);

                // 如果失败且设置了创建目录标志，尝试创建目录后重试
                let res = match res {
                    Ok(_) => Ok(()),
                    Err(_e) if get_create_dir_flag() => {
                        create_all_directories(&output_path);
                        std::os::unix::fs::symlink(&link_name, &output_path)
                    }
                    Err(e) => Err(e),
                };
                if let Err(e) = res {
                    error(
                        0,
                        e.raw_os_error().unwrap_or(0),
                        format_args!(
                            "cannot create symlink {} -> {}",
                            output_path,
                            link_name.display()
                        ),
                    );
                    continue;
                }
                if !get_no_chown_flag() {
                    let uid = if get_set_owner_flag() {
                        get_set_owner()
                    } else {
                        in_file_stat.uid()
                    };
                    let gid = if get_set_group_flag() {
                        get_set_group()
                    } else {
                        in_file_stat.gid()
                    };

                    if let Err(e) = std::os::unix::fs::lchown(&output_path, Some(uid), Some(gid)) {
                        // 对于符号链接，更宽容地处理权限设置错误
                        match e.raw_os_error() {
                            Some(libc::EPERM) | Some(libc::ENOENT) | Some(libc::EROFS)
                            | Some(libc::EINVAL) | Some(libc::EACCES) | Some(libc::ENOTSUP) => {
                                // 这些错误对于符号链接来说是可以忽略的
                            }
                            _ => {
                                error(
                                    0,
                                    e.raw_os_error().unwrap_or(0),
                                    format_args!("cannot change owner of {}", output_path),
                                );
                            }
                        }
                    }
                }
                if get_retain_time_flag() {
                    set_file_times(
                        None,
                        &output_path,
                        in_file_stat.mtime(),
                        in_file_stat.mtime(),
                        AT_SYMLINK_NOFOLLOW,
                    );
                }
            } else {
                error(0, 0, format_args!("{}: unknown file type", input_path));
            }
        }
        if get_verbose_flag() {
            eprintln!("{}", output_path);
        }
        if get_dot_flag() {
            eprint!(".");
        }
    }

    if get_dot_flag() {
        eprintln!();
    }

    apply_delayed_set_stat();

    if !get_quiet_flag() {
        let blocks = (output_tape.output_bytes + get_io_block_size() as usize - 1)
            / get_io_block_size() as usize;
        eprintln!("{} block{}", blocks, if blocks == 1 { "" } else { "s" });
    }

    Ok(())
}
