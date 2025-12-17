// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

use std::io;

mod appargs;
mod copyin;
mod copyout;
mod copypass;
mod cpiohdr;
mod dstring;
mod externs;
mod filemode;
mod filetype;
mod global;
mod idcache;
mod initramfs;
mod tar;
mod userspec;
mod util;

use clap::{Arg, ArgAction, ArgGroup, Command};
use gnu::quotearg::quotearg_colon;
use pax::paxexit::pax_exit;

use std::env::{self};
use std::fs::File;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::process;
use std::sync::Mutex;

use appargs::*;
use copyin::*;
use copyout::*;
use copypass::*;
use dstring::clear_reader_cache;
use externs::*;
use global::*;
use initramfs::*;
use userspec::*;
use util::*;

use pax::paxlib::*;
use pax::sysdep::*;

use gnu::error::*;
use gnu::progname::*;

// const PROGRAM_AUTHORS: [&str; 5] = [
//     "Phil Nelson",
//     "David MacKenzie",
//     "John Oleynick",
//     "Sergey Poznyakoff",
//     "", // Represents NULL in C
// ];
const DOC: &str = "GNU `utcpio' copies files to and from archives\n\
\n\
Examples:\n\
  # Copy files named in name-list to the archive\n\
  utcpio -o < name-list [> archive]\n\
  # Extract files from the archive\n\
  utcpio -i [< archive]\n\
  # Copy files named in name-list to destination-directory\n\
  utcpio -p destination-directory < name-list\n";

const USAGE_STR: &str =
    "utcpio [-ioptBcvVbfnrsSAl0aLdmu?] [-C NUMBER] [-D DIR] [-H FORMAT]\n      \
        [-R [USER][:.][GROUP]] [-W FLAG] [-F [[USER@]HOST:]FILE-NAME]\n      \
        [-M STRING] [-I [[USER@]HOST:]FILE-NAME] [-E FILE] [-e TYPE]\n      \
        [-O [[USER@]HOST:]FILE-NAME] [--extract] [--create]\n      \
        [--pass-through] [--list] [--block-size=BLOCK-SIZE]\n      \
        [--io-size=NUMBER] [--directory=DIR] [--force-local]\n      \
        [--format=FORMAT] [--quiet] [--owner=[USER][:.][GROUP]] [--verbose]\n      \
        [--dot] [--warning=FLAG] [--file=[[USER@]HOST:]FILE-NAME]\n      \
        [--message=STRING] [--rsh-command=COMMAND] [--swap] [--nonmatching]\n      \
        [--numeric-uid-gid] [--rename] [--swap-bytes] [--swap-halfwords]\n      \
        [--to-stdout] [--pattern-file=FILE] [--only-verify-crc] [--append]\n      \
        [--device-independent] [--reproducible] [--file-metadata=TYPE]\n      \
        [--ignore-devno] [--ignore-dirnlink] [--renumber-inodes] [--link]\n      \
        [--absolute-filenames] [--no-absolute-filenames] [--null]\n      \
        [--reset-access-time] [--dereference] [--make-directories]\n      \
        [--preserve-modification-time] [--no-preserve-owner] [--sparse]\n      \
        [--unconditional] [--help] [--usage] [--version]\n      \
        [destination-directory]";

macro_rules! CHECK_USAGE {
    ($cond:expr, $opt:expr, $cmd:expr) => {
        if $cond {
            USAGE_ERROR(
                0,
                format_args!("{:?} option is meaningless with {:?}", $opt, $cmd),
            );
        }
    };
}

#[derive(Clone, Copy, PartialEq)]
struct WarnTab {
    name: &'static str,
    flag: usize,
}

struct WarnControl {
    warn_option: usize,
    warn_tab: &'static [WarnTab],
}

impl WarnControl {
    fn new() -> Self {
        WarnControl {
            warn_option: CPIO_WARN_ALL,
            warn_tab: &[
                WarnTab {
                    name: "none",
                    flag: CPIO_WARN_ALL,
                },
                WarnTab {
                    name: "truncate",
                    flag: CPIO_WARN_TRUNCATE,
                },
                WarnTab {
                    name: "all",
                    flag: CPIO_WARN_ALL,
                },
                WarnTab {
                    name: "interdir",
                    flag: CPIO_WARN_INTERDIR,
                },
            ],
        }
    }

    fn warn_control(&mut self, arg: &str) -> bool {
        if arg == "none" {
            self.warn_option = 0;
            set_warn_option(0);

            return false;
        }

        let mut offset = 0;
        if arg.len() > 2 && arg.starts_with("no-") {
            offset = 3;
        }

        let mut w_o = get_warn_option();

        for wt in self.warn_tab {
            if &arg[offset..] == wt.name {
                if offset > 0 {
                    w_o &= !wt.flag as i32;
                } else {
                    w_o |= wt.flag as i32;
                }
                set_warn_option(w_o);
                return false;
            }
        }

        true
    }
}

#[allow(dead_code)]
fn test_read() {
    let mut stdin = io::stdin();
    let mut buffer = [0u8; 8192]; // 8KB 缓冲区

    loop {
        match stdin.read(&mut buffer[..512]) {
            Ok(0) => {
                // EOF，读取完成
                break;
            }
            Ok(read_bytes) => {
                // 处理读取的数据
                println!("Read {} bytes", read_bytes);
                // 在这里添加你的数据处理逻辑
                // 例如，你可以将数据写入另一个文件，或者进行其他操作
                //println!("Read data: {:?}", &buffer[..read_bytes]);
            }
            Err(e) => {
                // 读取错误
                eprintln!("Error reading from stdin: {}", e);
                break;
            }
        }
    }
}

fn parse_metadata_type(arg: &str) -> MetadataTypes {
    match arg {
        "none" => MetadataTypes::TypeNone,
        "xattr" => MetadataTypes::TypeXattr,
        _ => MetadataTypes::TypeNone, // Default to TypeNone
    }
}

fn process_args() {
    // let args: Vec<String> = env::args().collect();
    // print!("args: {:?}", args);

    let cmd = Command::new("utcpio")
        .author("zhanghaidong@uniontech.com")
        .about(DOC)
        .version("1.0.0")
        .disable_version_flag(true)
        .disable_help_flag(true)
        .group(ArgGroup::new("main_operation")
            .required(false) // 组中的参数不是必需的
            .multiple(true)) //表示该组中的参数可以同时出现
        .arg(Arg::new("create")
            .short('o')
            .long("create")
            .action(clap::ArgAction::SetTrue)
            .help("Create the archive (run in copy-out mode)")
            .group("main_operation"))
        .arg(Arg::new("extract")
            .short('i')
            .long("extract")
            .action(clap::ArgAction::SetTrue)
            .help("Extract files from an archive (run in copy-in mode)")
            .group("main_operation"))
        .arg(Arg::new("pass_through")
            .short('p')
            .long("pass-through")
            .action(clap::ArgAction::SetTrue)
            .help("Run in copy-pass mode")
            .group("main_operation"))
        .arg(Arg::new("list")
            .short('t')
            .action(clap::ArgAction::SetTrue)
            .long("list")
            .help("Print a table of contents of the input")
            .group("main_operation"))        
        .group(ArgGroup::new("operation_modifiers_any")
            .required(false)
            .multiple(true))
        .arg(Arg::new("directory")
            .short('D')
            .long("directory")
            .help("Change to directory DIR")
            .value_name("DIR")
            .group("operation_modifiers_any"))
        .arg(Arg::new("force_local")
            .long("force-local")
            .action(clap::ArgAction::SetTrue)
            .help("Archive file is local, even if its name contains colons")
            .group("operation_modifiers_any"))
        .arg(Arg::new("format")
            .short('H')
            .long("format")
            .help("Use given archive FORMAT")
            .value_name("FORMAT")
            .group("operation_modifiers_any"))
        .arg(Arg::new("block_size_5120")
            .short('B')
            .action(clap::ArgAction::SetTrue)
            .help("Set the I/O block size to 5120 bytes")
            .group("operation_modifiers_any"))
        .arg(Arg::new("block_size")
            .long("block-size")
            .help("Set the I/O block size to BLOCK-SIZE * 512 bytes")
            .value_name("BLOCK-SIZE")
            .group("operation_modifiers_any"))
        .arg(Arg::new("use_svr4")
            .short('c')
            .action(clap::ArgAction::SetTrue)
            .help("Identical to \"-H newc\", use the new (SVR4) portable format. If you wish the old portable (ASCII) archive format, use \"-H odc\" instead.")
            .group("operation_modifiers_any"))
        .arg(Arg::new("dot")
            .short('V')
            .long("dot")
            .action(clap::ArgAction::SetTrue)
            .help("Print a \".\" for each file processed")
            .group("operation_modifiers_any"))
        .arg(Arg::new("io_size")
            .short('C')
            .long("io-size")
            .help("Set the I/O block size to the given NUMBER of bytes")
            .value_name("NUMBER")
            .group("operation_modifiers_any"))
        .arg(Arg::new("quiet")
            .long("quiet")
            .action(clap::ArgAction::SetTrue)
            .help("Do not print the number of blocks copied")
            .group("operation_modifiers_any"))
        .arg(Arg::new("verbose")
            .short('v')
            .long("verbose")
            .action(clap::ArgAction::SetTrue)
            .help("Verbosely list the files processed")
            .group("operation_modifiers_any"))
        .arg(Arg::new("warning")
            .short('W')
            .long("warning")
            .help("Control warning display. Currently FLAG is one of 'none', 'truncate', 'all'. Multiple options accumulate.")
            .value_name("FLAG")
            .group("operation_modifiers_any"))
        .arg(Arg::new("owner")
            .short('R')
            .long("owner")
            .help("Set the ownership of all files created to the specified USER and/or GROUP")
            .value_name("[USER][:.][GROUP]")
            .group("operation_modifiers_any"))
        .group(ArgGroup::new("operation_modifiers_in_out")
            .required(false)
            .multiple(true))
        .arg(Arg::new("file")
            .short('F')
            .long("file")
            .help("Use this FILE-NAME instead of standard input or output. Optional USER and HOST specify the user and host names in case of a remote archive")
            .value_name("[[USER@]HOST:]FILE-NAME")
            .group("operation_modifiers_in_out"))
        .arg(Arg::new("message")
            .short('M')
            .long("message")
            .help("Print STRING when the end of a volume of the backup media is reached")
            .value_name("STRING")
            .group("operation_modifiers_in_out"))
        .arg(Arg::new("rsh_command")
            .long("rsh-command")
            .help("Use COMMAND instead of rsh")
            .value_name("COMMAND")
            .group("operation_modifiers_in_out"))        
        .group(ArgGroup::new("operation_modifiers_in_only")
            .required(false)
            .multiple(true))
        .arg(Arg::new("nonmatching")
            .short('f')
            .long("nonmatching")
            .action(clap::ArgAction::SetTrue)
            .help("Only copy files that do not match any of the given patterns")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("numeric_uid_gid")
            .short('n')
            .long("numeric-uid-gid")
            .action(clap::ArgAction::SetTrue)
            .help("In the verbose table of contents listing, show numeric UID and GID")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("pattern_file")
            .short('E')
            .long("pattern-file")
            .help("Read additional patterns specifying filenames to extract or list from FILE")
            .value_name("FILE")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("only_verify_crc")
            .long("only-verify-crc")
            .action(clap::ArgAction::SetTrue)
            .help("When reading a CRC format archive, only verify the checksum of each file in the archive, don't actually extract the files")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("rename")
            .short('r')
            .long("rename")
            .action(clap::ArgAction::SetTrue)
            .help("Interactively rename files")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("rename_batch_file")
            .long("rename-batch-file")
            .help("")
            .value_name("FILE")
            .hide(true)
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("swap")
            .short('b')
            .long("swap")
            .action(clap::ArgAction::SetTrue)
            .help("Swap both halfwords of words and bytes of halfwords in the data. Equivalent to -sS")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("swap_bytes")
            .short('s')
            .long("swap-bytes")
            .action(clap::ArgAction::SetTrue)
            .help("Swap the bytes of each halfword in the files")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("swap_halfwords")
            .short('S')
            .long("swap-halfwords")
            .action(clap::ArgAction::SetTrue)
            .help("Swap the halfwords of each word (4 bytes) in the files")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("to_stdout")
            .long("to-stdout")
            .action(clap::ArgAction::SetTrue)
            .help("Extract files to standard output")
            .group("operation_modifiers_in_only"))
        .arg(Arg::new("input_archive")
            .short('I')
            .long("input-archive")
            .help("Archive filename to use instead of standard input. Optional USER and HOST specify the user and host names in case of a remote archive")
            .value_name("[[USER@]HOST:]FILE-NAME"))        
        .group(ArgGroup::new("operation_modifiers_out_only")
            .required(false)
            .multiple(true))
        .arg(Arg::new("append")
            .short('A')
            .long("append")
            .action(clap::ArgAction::SetTrue)
            .help("Append to an existing archive.")
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("output_archive")
            .short('O')
            .long("output-archive")
            .help("Archive filename to use instead of standard output. Optional USER and HOST specify the user and host names in case of a remote archive")
            .value_name("[[USER@]HOST:]FILE-NAME")
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("renumber_inodes")
            .long("renumber-inodes")
            .help("Renumber inodes")
            .action(clap::ArgAction::SetTrue)
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("ignore_devno")
            .long("ignore-devno")
            .action(clap::ArgAction::SetTrue)
            .help("Don't store device numbers")
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("ignore_dirnlink")
            .long("ignore-dirnlink")
            .action(clap::ArgAction::SetTrue)
            .help("ignore number of links of a directory; always assume 2")
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("device_independent")
            .long("device-independent")
            .action(clap::ArgAction::SetTrue)
            .help("Create device-independent (reproducible) archives")
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("reproducible")
            .long("reproducible")
            .help("Alias for --device-independent")
            .hide(true)
            .group("operation_modifiers_out_only"))
        .arg(Arg::new("file_metadata_out")
            .short('e')
            .long("file-metadata")
            .help("Include file metadata")
            .value_name("TYPE")
            .group("operation_modifiers_out_only"))
        .group(ArgGroup::new("operation_modifiers_pass_only")
            .required(false)
            .multiple(true))
        .arg(Arg::new("link")
            .short('l')
            .long("link")
            .action(clap::ArgAction::SetTrue)
            .help("Link files instead of copying them, when possible")
            .group("operation_modifiers_pass_only"))        
        .group(ArgGroup::new("operation_modifiers_in_out_2")
            .required(false)
            .multiple(true))
        .arg(Arg::new("absolute_filenames")
            .long("absolute-filenames")
            .action(clap::ArgAction::SetTrue)
            .help("Do not strip file system prefix components from the file names")
            .group("operation_modifiers_in_out_2"))
        .arg(Arg::new("no_absolute_filenames")
            .long("no-absolute-filenames")
            .action(clap::ArgAction::SetTrue)
            .help("Create all files relative to the current directory")
            .group("operation_modifiers_in_out_2"))
        .group(ArgGroup::new("operation_modifiers_out_pass")
            .required(false)
            .multiple(true))
        .arg(Arg::new("null")
            .short('0')
            .long("null")
            .action(clap::ArgAction::SetTrue)
            .help("Filenames in the list are delimited by null characters instead of newlines")
            .group("operation_modifiers_out_pass"))
        .arg(Arg::new("dereference")
            .short('L')
            .long("dereference")
            .action(clap::ArgAction::SetTrue)
            .help("Dereference symbolic links (copy the files that they point to instead of copying the links).")
            .group("operation_modifiers_out_pass"))
        .arg(Arg::new("reset_access_time")
            .short('a')
            .long("reset-access-time")
            .action(clap::ArgAction::SetTrue)
            .help("Reset the access times of files after reading them")
            .group("operation_modifiers_out_pass"))
        .group(ArgGroup::new("operation_modifiers_in_pass")
            .required(false)
            .multiple(true))
        .arg(Arg::new("preserve_modification_time")
            .short('m')
            .long("preserve-modification-time")
            .action(clap::ArgAction::SetTrue)
            .help("Retain previous file modification times when creating files")
            .group("operation_modifiers_in_pass"))
        .arg(Arg::new("make_directories")
            .short('d')
            .action(clap::ArgAction::SetTrue)
            .long("make-directories")
            .help("Create leading directories where needed")
            .group("operation_modifiers_in_pass"))
        .arg(Arg::new("no_preserve_owner")
            .long("no-preserve-owner")
            .help("Do not change the ownership of the files")
            .action(clap::ArgAction::SetTrue)
            .group("operation_modifiers_in_pass"))
        .arg(Arg::new("unconditional")
            .short('u')
            .long("unconditional")
            .action(clap::ArgAction::SetTrue)
            .help("Replace all files unconditionally")
            .group("operation_modifiers_in_pass"))
        .arg(Arg::new("sparse")
            .long("sparse")
            .action(clap::ArgAction::SetTrue)
            .help("Write files with large blocks of zeros as sparse files")
            .group("operation_modifiers_in_pass"))
        .arg(Arg::new("help")
            .short('?')
            .long("help")
            .action(ArgAction::SetTrue)
            .help("give a help list"))
        .arg(Arg::new("usage")
            .long("usage")
            .action(ArgAction::SetTrue)
            .help("Display usage information"))
        .override_usage(USAGE_STR)
        .arg(Arg::new("patterns")
            .num_args(0..)
            .trailing_var_arg(true));

    let mut cmd_clone = cmd.clone();

    let matches: clap::ArgMatches = match cmd.try_get_matches() {
        Ok(matches) => {
            //            println!("Matches: {:?}", matches);
            matches
        }
        Err(err) => {
            eprintln!("{}", err); // 打印错误信息到标准错误输出

            usage(0);

            process::exit(err.exit_code());
        }
    };

    if matches.get_flag("usage") {
        let usage = cmd_clone.render_usage();
        println!("{}", usage);
        process::exit(0);
    }

    if matches.get_flag("help") {
        let usage = cmd_clone.render_help();
        println!("{}", usage);
        process::exit(0);
    }

    if matches.get_flag("null") {
        // 0
        set_name_end('\0' as i8);
    }
    if matches.get_flag("reset_access_time") {
        // -a
        set_reset_time_flag(true);
    }
    if matches.get_flag("append") {
        // -A
        set_append_flag(true);
    }
    if matches.get_flag("swap") {
        //  -b
        set_swap_bytes_flag(true);
        set_swap_halfwords_flag(true);
    }
    if matches.get_flag("block_size_5120") {
        // -B
        set_io_block_size(5120);
    }
    if matches.get_flag("nonmatching") {
        // -f
        set_copy_matching_files(false);
    }

    if matches.contains_id("block_size") {
        // -B

        let block_size_str = matches
            .get_one::<String>("block_size")
            .expect("block_size should be present");
        let block_size: i32 = block_size_str
            .parse()
            .expect("block_size should be a valid number");
        if !(1..=512).contains(&(block_size)) {
            USAGE_ERROR(0, format_args!("Invalid block size"));
        }
        set_io_block_size(block_size * 512);
    }
    if matches.get_flag("use_svr4") {
        // -c
        if get_archive_format() != ArchiveFormat::Unknown {
            USAGE_ERROR(1, format_args!("Archive format multiply defined"));
        }
        set_archive_format(ArchiveFormat::Newascii);
    }
    if matches.contains_id("io_size") {
        // -C
        let io_size_str = matches
            .get_one::<String>("io_size")
            .expect("block_size should be present");
        let io_size = io_size_str
            .parse()
            .expect("block_size should be a valid number");
        if !(1..=512).contains(&(io_size)) {
            USAGE_ERROR(1, format_args!("valid block size"));
        }
        set_io_block_size(io_size);
    }
    if matches.get_flag("make_directories") {
        // -d
        set_create_dir_flag(true);
    }
    if matches.contains_id("directory") {
        // -D
        let directory_str = matches
            .get_one::<String>("directory")
            .expect("directory should be present");

        set_change_directory_option(Some(directory_str.clone()));
    }
    if matches.contains_id("file_metadata_out") {
        // -e
        let file_metadata_out_str = matches
            .get_one::<String>("file_metadata_out")
            .expect("file_metadata_out should be present");
        let mt = parse_metadata_type(file_metadata_out_str);
        set_metadata_type(mt);
    }
    if matches.contains_id("pattern_file") {
        // -E
        let pattern_file_str = matches
            .get_one::<String>("pattern_file")
            .expect("pattern_file should be present");
        set_pattern_file_name(Some(pattern_file_str.clone()));
    }
    if matches.contains_id("file") {
        // -F
        let file_str = matches
            .get_one::<String>("file")
            .expect("file should be present");
        set_archive_name(Some(file_str.clone()));
    }

    if matches.contains_id("format") {
        // -H
        let format_str = matches
            .get_one::<String>("format")
            .expect("format should be present");
        if get_archive_format() != ArchiveFormat::Unknown {
            return USAGE_ERROR(0, format_args!("Archive format multiply defined"));
        }

        let _ = match format_str.to_lowercase().as_str() {
            "crc" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Crcascii),
            "newc" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Newascii),
            "odc" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Oldascii),
            "bin" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Binary),
            "ustar" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Ustar),
            "tar" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Tar),
            "hpodc" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Hpoldascii),
            "hpbin" => Ok::<ArchiveFormat, ()>(ArchiveFormat::Hpbinary),
            _ => {
                USAGE_ERROR(0, format_args!("invalid archive format `{}'; valid formats are: crc newc odc bin ustar tar (all-caps also recognized)", format_str));
                return;
            }
        };
    }
    // -i
    if matches.get_flag("extract") {
        // -i
        if get_copy_function().is_some() {
            USAGE_ERROR(0, format_args!("Mode already defined"));
        }
        set_copy_function(process_copy_in);
    }
    // -I
    if matches.contains_id("input_archive") {
        // -I
        let input_archive_str = matches
            .get_one::<String>("input_archive")
            .expect("input_archive should be present");
        set_input_archive_name(Some(input_archive_str.clone()));
    }
    // -l
    if matches.get_flag("link") {
        // -l
        set_link_flag(true);
    }
    // -L
    if matches.contains_id("dereference") { // -L
         //set_xstat(stat);  // 统一用系统默认的方式处理了。
    }
    // -m
    if matches.get_flag("preserve_modification_time") {
        // -m
        set_retain_time_flag(true);
    }
    // -M
    if matches.contains_id("message") {
        // -M
        let message_str = matches
            .get_one::<String>("message")
            .expect("message should be present");
        set_new_media_message(message_str);
    }
    // -n
    if matches.get_flag("numeric_uid_gid") {
        // -n
        set_numeric_uid(true);
    }
    // --no-absolute-filenames
    if matches.get_flag("no_absolute_filenames") {
        // -n
        set_no_abs_paths_flag(true);
    }
    // --absolute-filenames
    if matches.get_flag("absolute_filenames") {
        // -n
        set_no_abs_paths_flag(false);
    }

    // --no-preserve-owner
    if matches.get_flag("no_preserve_owner") {
        // -n
        if get_set_group_flag() || get_set_group_flag() {
            USAGE_ERROR(
                1,
                format_args!("Cannot use --no-preserve-owner with --owner"),
            );
        }
        set_no_chown_flag(true);
    }

    //-o
    if matches.get_flag("create") {
        // -o
        if get_copy_function().is_some() {
            USAGE_ERROR(0, format_args!("Mode already defined"));
        }
        set_copy_function(process_copy_out);
    }

    // -O
    if matches.contains_id("output_archive") {
        // -O
        let output_archive_str = matches
            .get_one::<String>("output_archive")
            .expect("output_archive should be present");
        set_output_archive_name(Some(output_archive_str.clone()));
    }

    // -only-verify-crc
    if matches.get_flag("only_verify_crc") {
        set_only_verify_crc_flag(true);
    }
    // -p
    if matches.get_flag("pass_through") {
        if get_copy_function().is_some() {
            USAGE_ERROR(0, format_args!("Mode already defined"));
        }
        set_copy_function(process_copy_pass);
    }
    // --ignore-devno
    if matches.get_flag("ignore_devno") {
        set_ignore_devno_option(true);
    }
    // --renumber-inodes
    if matches.get_flag("renumber_inodes") {
        set_renumber_inodes_option(true);
    }
    // --ignore-dirnlink
    if matches.get_flag("ignore_dirnlink") {
        set_ignore_dirnlink_option(true);
    }
    // --device-independent
    if matches.get_flag("device_independent") {
        set_ignore_devno_option(true);
        set_renumber_inodes_option(true);
        set_ignore_dirnlink_option(true);
    }

    // rsh-command
    if matches.contains_id("rsh_command") {
        let rsh_command_str = matches
            .get_one::<String>("rsh_command")
            .expect("rsh_command should be present");
        set_rsh_command_option(Some(rsh_command_str.clone()));
    }

    // -r
    if matches.get_flag("rename") {
        // -r
        set_rename_flag(true);
    }
    //RENAME_BATCH_FILE_OPTION
    if matches.contains_id("rename_batch_file") {
        // -r
        let rename_batch_file_str = matches
            .get_one::<String>("rename_batch_file")
            .expect("rename_batch_file should be present");
        set_rename_batch_file(Some(rename_batch_file_str.clone()));
    }

    //QUIET_OPTION
    if matches.get_flag("quiet") {
        // -q
        set_quiet_flag(true);
    }

    // -R
    if matches.contains_id("owner") {
        // -R
        let owner_str = matches
            .get_one::<String>("owner")
            .expect("owner should be present");
        // let setowner;
        // let setgroup;

        match parse_user_spec(owner_str) {
            Ok((uid, gid, username, groupname)) => {
                set_set_owner(uid);
                set_set_group(gid);
                if username.is_some() {
                    set_set_owner_flag(true);
                }
                if groupname.is_some() {
                    set_set_group_flag(true);
                }
            }
            Err(_err) => {
                USAGE_ERROR(
                    0,
                    format_args!("Invalid value for --owner option: {}", owner_str),
                );
            }
        };
    }
    // -s
    if matches.get_flag("swap_bytes") {
        // -s
        set_swap_bytes_flag(true);
    }
    // -S
    if matches.get_flag("swap_halfwords") {
        // -S
        set_swap_halfwords_flag(true);
    }
    // -t
    if matches.get_flag("list") {
        // -t
        set_table_flag(true);
    }
    // -u
    if matches.get_flag("unconditional") {
        set_unconditional_flag(true);
    }
    // -v
    if matches.get_flag("verbose") {
        set_verbose_flag(true);
    }
    // -V
    if matches.get_flag("dot") {
        set_dot_flag(true);
    }
    // -w
    if matches.contains_id("warning") {
        let warning_str = matches
            .get_one::<String>("warning")
            .expect("warning should be present");
        let mut warn = WarnControl::new();

        if warn.warn_control(warning_str) {
            USAGE_ERROR(
                0,
                format_args!("Invalid value for --warning option: {}", warning_str),
            );
        }
    }
    // SPARSE_OPTION
    if matches.get_flag("sparse") {
        set_sparse_flag(true);
    }
    //FORCE_LOCAL_OPTION
    if matches.get_flag("force_local") {
        set_force_local_option(true);
        FORCE_LOCAL_OPTION.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    //TO_STDOUT_OPTION
    if matches.get_flag("to_stdout") {
        set_to_stdout_option(true);
    }

    if get_copy_function().is_none() {
        if get_table_flag() {
            set_copy_function(process_copy_in);
        } else {
            USAGE_ERROR(0, format_args!("You must specify one of -oipt options."));
        }
    }

    if get_copy_function() == Some(process_copy_in) {
        let file = unsafe { File::from_raw_fd(libc::STDIN_FILENO) };
        let _ = set_archive_des(file);

        let reset_time_glag = get_reset_time_flag();
        let link_flag = get_link_flag();

        CHECK_USAGE!(reset_time_glag, "--reset", "--extract");

        CHECK_USAGE!(link_flag, "--link", "--extract");
        //CHECK_USAGE!(unsafe { get_xstat() != lstat }, "--dereference", "--extract");
        // CHECK_USAGE!(false, "--dereference", "--extract");
        CHECK_USAGE!(get_append_flag(), "--append", "--extract");
        CHECK_USAGE!(get_input_archive_name().is_some(), "-O", "--extract"); // 修改
        CHECK_USAGE!(
            get_renumber_inodes_option(),
            "--renumber-inodes",
            "--extract"
        );
        CHECK_USAGE!(get_ignore_devno_option(), "--ignore-devno", "--extract");
        if get_to_stdout_option() {
            CHECK_USAGE!(get_create_dir_flag(), "--make-directories", "--to-stdout");
            CHECK_USAGE!(get_rename_flag(), "--rename", "--to-stdout");
            CHECK_USAGE!(get_no_chown_flag(), "--no-preserve-owner", "--to-stdout");
            CHECK_USAGE!(
                get_set_owner_flag() || get_set_group_flag(),
                "--owner",
                "--to-stdout"
            );
            CHECK_USAGE!(
                get_retain_time_flag(),
                "--preserve-modification-time",
                "--to-stdout"
            );
        }

        if get_archive_name().is_some() && get_input_archive_name().is_some() {
            USAGE_ERROR(0, format_args!("Both -I and -F are used in copy-in mode"));
        }

        if get_archive_format() == ArchiveFormat::Crcascii {
            set_crc_i_flag(true);
        }
        if let Some(patterns) = matches.get_many::<String>("patterns") {
            let pattern_vec: Vec<String> = patterns.map(|s| s.to_string()).collect();
            set_num_patterns(pattern_vec.len() as i32);
            set_save_patterns(pattern_vec);
        }

        if get_input_archive_name().is_some() {
            set_archive_name(get_input_archive_name().clone());
        }
    } else if get_copy_function() == Some(process_copy_out) {
        // if index != args.len() as i32 { // 修改
        //     USAGE_ERROR!((1, 0, _("Too many arguments")));
        // }

        let file = unsafe { File::from_raw_fd(libc::STDOUT_FILENO) };
        let _ = set_archive_des(file);

        CHECK_USAGE!(get_create_dir_flag(), "--make-directories", "--create");
        CHECK_USAGE!(get_rename_flag(), "--rename", "--create");
        CHECK_USAGE!(get_table_flag(), "--list", "--create");
        CHECK_USAGE!(get_unconditional_flag(), "--unconditional", "--create");
        CHECK_USAGE!(get_link_flag(), "--link", "--create");
        CHECK_USAGE!(get_sparse_flag(), "--sparse", "--create");
        CHECK_USAGE!(
            get_retain_time_flag(),
            "--preserve-modification-time",
            "--create"
        );
        CHECK_USAGE!(get_no_chown_flag(), "--no-preserve-owner", "--create");
        CHECK_USAGE!(get_swap_bytes_flag(), "--swap-bytes (--swap)", "--create");
        CHECK_USAGE!(
            get_swap_halfwords_flag(),
            "--swap-halfwords (--swap)",
            "--create"
        );
        CHECK_USAGE!(get_to_stdout_option(), "--to-stdout", "--create");

        if get_append_flag() && get_archive_name().is_none() && get_output_archive_name().is_none()
        {
            // 修改
            USAGE_ERROR(
                1,
                format_args!(
                    "--append is used but no archive file name is given (use -F or -O options)"
                ),
            );
        }

        CHECK_USAGE!(
            get_rename_batch_file().is_some(),
            "--rename-batch-file",
            "--create"
        );
        CHECK_USAGE!(get_input_archive_name().is_some(), "-I", "--create");

        if get_archive_name().is_some() && get_output_archive_name().is_some() {
            USAGE_ERROR(1, format_args!("Both -O and -F are used in copy-out mode"));
        }

        if get_archive_format() == ArchiveFormat::Unknown {
            set_archive_format(ArchiveFormat::Binary);
        }

        if get_output_archive_name().is_some() {
            set_archive_name(get_output_archive_name().clone());
        }

        if !arf_stores_inode_p(get_archive_format()) {
            set_renumber_inodes_option(false);
            set_ignore_devno_option(false);
        }
    } else {
        // Copy pass
        // if index < args.len() as i32 - 1 { // 修改
        //     USAGE_ERROR!((1, 0, _("Too many arguments")));
        // } else if index > args.len() as i32 - 1 { // 修改
        //     USAGE_ERROR!((1, 0, _("Not enough arguments")));
        // }

        if get_archive_format() != ArchiveFormat::Unknown {
            USAGE_ERROR(
                1,
                format_args!(
                    "Archive format is not specified in copy-pass mode (use --format option)"
                ),
            );
        }

        CHECK_USAGE!(
            get_swap_bytes_flag(),
            "--swap-bytes (--swap)",
            "--pass-through"
        );
        CHECK_USAGE!(
            get_swap_halfwords_flag(),
            "--swap-halfwords (--swap)",
            "--pass-through"
        );
        CHECK_USAGE!(get_table_flag(), "--list", "--pass-through");
        CHECK_USAGE!(get_rename_flag(), "--rename", "--pass-through");
        CHECK_USAGE!(get_append_flag(), "--append", "--pass-through");
        CHECK_USAGE!(
            get_rename_batch_file().is_some(),
            "--rename-batch-file",
            "--pass-through"
        ); // 修改
        CHECK_USAGE!(
            get_no_abs_paths_flag(),
            "--no-absolute-pathnames",
            "--pass-through"
        );
        CHECK_USAGE!(
            get_no_abs_paths_flag(),
            "--absolute-pathnames",
            "--pass-through"
        );
        CHECK_USAGE!(get_to_stdout_option(), "--to-stdout", "--pass-through");
        CHECK_USAGE!(
            get_renumber_inodes_option(),
            "--renumber-inodes",
            "--pass-through"
        );
        CHECK_USAGE!(
            get_ignore_devno_option(),
            "--ignore-devno",
            "--pass-through"
        );

        if let Some(patterns) = matches.get_many::<String>("patterns") {
            let pattern_vec: Vec<String> = patterns.map(|s| s.to_string()).collect();
            if !pattern_vec.is_empty() {
                set_directory_name(Some(pattern_vec[0].clone()));
            }
        }
        // unsafe { directory_name = args[index as usize].as_ptr() as *mut c_char }; // 需要定义 directory_name，注意这里使用了 unsafe 代码
    };

    if get_archive_name().is_some() {
        if get_copy_function() != Some(process_copy_in)
            && get_copy_function() != Some(process_copy_out)
        {
            error(
                PAXEXIT_FAILURE,
                0,
                format_args!("-F can be used only with --create or --extract"),
            );
        }
        let arch = open_archive(get_archive_name().clone().unwrap().as_str());

        match arch {
            Ok(fd) => {
                let _ = set_archive_des(fd);
            }
            Err(_e) => {
                error(
                    PAXEXIT_FAILURE,
                    errno(),
                    format_args!(
                        "Cannot open {:?}",
                        quotearg_colon(get_archive_name().clone().unwrap().as_str())
                    ),
                );
            }
        }
    }

    // Prevent SysV non-root users from giving away files inadvertently.
    if !get_set_owner_flag() && !get_set_group_flag() && get_euid() != 0.into() {
        // 修改
        set_no_chown_flag(true);
    }
}

fn initialize_buffers() {
    let copy_function = get_copy_function();
    let io_block_size = get_io_block_size() as usize;

    let (in_buf_size, out_buf_size) = {
        if copy_function == Some(process_copy_in) {
            let in_buf_size = if io_block_size >= 512 {
                2 * io_block_size as usize
            } else {
                1024
            };
            (in_buf_size, DISK_IO_BLOCK_SIZE)
        } else if copy_function == Some(process_copy_out) {
            (DISK_IO_BLOCK_SIZE, io_block_size)
        } else {
            (DISK_IO_BLOCK_SIZE, DISK_IO_BLOCK_SIZE)
        }
    };

    resize_output_buffer(out_buf_size);
    resize_input_buffer(in_buf_size);
}

fn main() {
    // setlocale(LocaleCategory::LcAll, "");
    // textdomain(PACKAGE).unwrap();

    set_program_name(&env::args().collect::<Vec<String>>()[0]);

    let app_args = AppArgs::new();
    let _ = APPARGS.set(Mutex::new(app_args));

    process_args();

    initialize_buffers();

    match get_copy_function() {
        Some(copy_func) => {
            let _ = copy_func();
        }
        None => usage(PAXEXIT_FAILURE),
    }

    clear_reader_cache();

    // if get_achive_des() >= 0 && rmtclose(get_achive_des()) == -1 {
    //         error(PAXEXIT_FAILURE, errno(), format_args!("error closing archive"));
    // }

    pax_exit();
}
