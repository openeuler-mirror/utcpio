/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use crate::externs::*;
use crate::initramfs::*;

// static DOC: &str = "GNU `cpio' copies files to and from archives\n\
// \n\
// Examples:\n\
//  # Copy files named in name-list to the archive\n\
//  cpio -o < name-list [> archive]\n\
//  # Extract files from the archive\n\
//  cpio -i [< archive]\n\
//  # Copy files named in name-list to destination-directory\n\
//  cpio -p destination-directory < name-list\n";

use std::fs::File;
use std::io;
use std::sync::MutexGuard;
use std::sync::{Mutex, OnceLock};

pub static ARCHIVE_DES: OnceLock<Mutex<File>> = OnceLock::new();

pub fn get_archive_des() -> io::Result<MutexGuard<'static, File>> {
    let mutex = ARCHIVE_DES
        .get()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "ARCHIVE_DES not initialized"))?;
    mutex.lock().map_err(|e| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("Failed to lock ARCHIVE_DES: {}", e),
        )
    })
}

// 设置 archive_des
pub fn set_archive_des(value: File) -> io::Result<()> {
    // 如果 ARCHIVE_DES 未初始化，先初始化
    if ARCHIVE_DES.get().is_none() {
        ARCHIVE_DES
            .set(Mutex::new(value))
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "ARCHIVE_DES already initialized"))?;
        Ok(())
    } else {
        // 如果已初始化，获取锁并设置
        let mutex = ARCHIVE_DES.get().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "ARCHIVE_DES not found")
        })?;
        let mut guard = mutex.lock().map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Failed to lock ARCHIVE_DES: {}", e),
            )
        })?;
        *guard = value;
        Ok(())
    }
}

pub struct AppArgs {
    // Operation modifiers valid in copy-out and copy-pass modes GRID 600
    name_end: i8,
    reset_time_flag: bool,
    append_flag: bool,
    swap_bytes_flag: bool,
    swap_halfwords_flag: bool,
    io_block_size: i32,
    archive_format: ArchiveFormat,
    create_dir_flag: bool,
    change_directory_option: Option<String>,
    metadata_type: MetadataTypes,
    pattern_file_name: Option<String>,
    archive_name: Option<String>,
    copy_matching_files: bool,
    copy_function: Option<CopyFunctionFn>,
    input_archive_name: Option<String>,
    link_flag: bool,
    //xstat: Option<Xstat>,
    retain_time_flag: bool,
    new_media_message: Option<String>,
    new_media_message_with_number: Option<String>,
    new_media_message_after_number: Option<String>,

    crc_i_flag: bool,
    crc: usize,

    rename_batch_file: Option<String>,

    no_abs_paths_flag: bool,
    rename_flag: bool,

    set_owner_flag: bool,
    set_group_flag: bool,
    set_owner: u32,
    set_group: u32,

    output_archive_name: Option<String>,
    only_verify_crc_flag: bool,
    ignore_devno_option: bool,
    renumber_inodes_option: bool,
    rsh_command_option: Option<String>,
    quiet_flag: bool,

    ignore_dirnlink_option: bool,

    no_chown_flag: bool,
    table_flag: bool,
    unconditional_flag: bool,
    verbose_flag: bool,
    dot_flag: bool,
    warn_option: i32,
    sparse_flag: bool,
    force_local_option: bool,
    to_stdout_option: bool,
    // debug_flag: bool,
    numeric_uid: bool,

    directory_name: Option<String>,
    num_patterns: i32,
    save_patterns: Vec<String>,
}
impl AppArgs {
    pub fn new() -> Self {
        AppArgs {
            name_end: b'\n' as i8,
            reset_time_flag: false,
            append_flag: false,
            swap_bytes_flag: false,
            swap_halfwords_flag: false,
            io_block_size: 512,
            archive_format: ArchiveFormat::Unknown,
            create_dir_flag: false,
            change_directory_option: None,
            metadata_type: MetadataTypes::TypeNone,
            pattern_file_name: None,
            archive_name: None,
            copy_matching_files: true,
            copy_function: None,
            input_archive_name: None,
            link_flag: false,
            //            xstat: None,
            retain_time_flag: false,
            new_media_message: None,
            new_media_message_with_number: None,
            new_media_message_after_number: None,
            crc_i_flag: false,
            crc: 0,
            rename_batch_file: None,
            no_abs_paths_flag: false,
            rename_flag: false,
            set_owner_flag: false,
            set_group_flag: false,
            set_owner: 0,
            set_group: 0,
            output_archive_name: None,
            only_verify_crc_flag: false,
            ignore_devno_option: false,
            renumber_inodes_option: false,
            rsh_command_option: None,
            quiet_flag: false,
            ignore_dirnlink_option: false,
            no_chown_flag: false,
            table_flag: false,
            unconditional_flag: false,
            verbose_flag: false,
            dot_flag: false,
            warn_option: 0,
            sparse_flag: false,
            force_local_option: false,
            to_stdout_option: false,
            //            debug_flag: false,
            numeric_uid: false,
            directory_name: None,
            //            archive_des: 0,
            num_patterns: 0,
            save_patterns: vec![],
        }
    }
}

pub static APPARGS: OnceLock<Mutex<AppArgs>> = OnceLock::new();

pub fn get_save_patterns() -> Vec<String> {
    APPARGS.get().unwrap().lock().unwrap().save_patterns.clone()
}
pub fn set_save_patterns(value: Vec<String>) {
    APPARGS.get().unwrap().lock().unwrap().save_patterns = value;
}

pub fn get_num_patterns() -> i32 {
    APPARGS.get().unwrap().lock().unwrap().num_patterns
}
pub fn set_num_patterns(value: i32) {
    if let Some(appargs) = APPARGS.get() {
        if let Ok(mut guard) = appargs.lock() {
            guard.num_patterns = value;
        } else {
            eprintln!("Failed to lock APPARGS");
            // 处理锁获取失败的情况
        }
    } else {
        eprintln!("APPARGS is not initialized");
        // 处理 APPARGS 未初始化的情况
    }
}

pub fn get_sparse_flag() -> bool {
    APPARGS.get().unwrap().lock().unwrap().sparse_flag
}

pub fn get_new_media_message() -> Option<String> {
    APPARGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .new_media_message
        .clone()
}

pub fn get_args_new_media_message_with_number() -> Option<String> {
    APPARGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .new_media_message_with_number
        .clone()
}

pub fn get_new_media_message_after_number() -> Option<String> {
    APPARGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .new_media_message_after_number
        .clone()
}


pub fn get_archive_name() -> Option<String> {
    APPARGS.get().unwrap().lock().unwrap().archive_name.clone()
}

pub fn get_rsh_command_option() -> Option<String> {
    APPARGS
        .get()
        .unwrap()
        .lock()
        .unwrap()
        .rsh_command_option
        .clone()
}


pub fn get_append_flag() -> bool {
    APPARGS.get().unwrap().lock().unwrap().append_flag
}
