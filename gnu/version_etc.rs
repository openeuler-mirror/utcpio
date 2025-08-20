/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::format_in_format_args, clippy::write_literal)]

use crate::argp::*;
const COPYRIGHT_YEAR: u32 = 2023;
// version_etc.rs

/// The copyright notice for version information.
pub const VERSION_ETC_COPYRIGHT: &str = "Copyright {0} {1} Free Software Foundation, Inc.";

/// Display the --version information the standard way.
fn version_etc_arn(
    stream: &mut dyn std::io::Write,
    command_name: Option<&str>,
    package: &str,
    version: &str,
    authors: &[&str],
) {
    if let Some(name) = command_name {
        writeln!(stream, "{} ({}) {}", name, package, version).unwrap();
    } else {
        writeln!(stream, "{} {}", package, version).unwrap();
    }

    writeln!(
        stream,
        "{}",
        format!(
            "Copyright {} {} Free Software Foundation, Inc",
            "(C)", COPYRIGHT_YEAR
        )
    )
    .unwrap();
    writeln!(stream, "GPL-3.0-or-later: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.\nThis is free software: you are free to change and redistribute it.\nThere is NO WARRANTY, to the extent permitted by law.").unwrap();

    match authors.len() {
        0 => {}
        1 => writeln!(stream, "Written by {}.", authors[0]).unwrap(),
        2 => writeln!(stream, "Written by {} and {}.", authors[0], authors[1]).unwrap(),
        3 => writeln!(
            stream,
            "Written by {}, {}, and {}.",
            authors[0], authors[1], authors[2]
        )
        .unwrap(),
        4 => writeln!(
            stream,
            "Written by {}, {}, {}, and {}.",
            authors[0], authors[1], authors[2], authors[3]
        )
        .unwrap(),
        5 => writeln!(
            stream,
            "Written by {}, {}, {}, {}, and {}.",
            authors[0], authors[1], authors[2], authors[3], authors[4]
        )
        .unwrap(),
        6 => writeln!(
            stream,
            "Written by {}, {}, {}, {}, {}, and {}.",
            authors[0], authors[1], authors[2], authors[3], authors[4], authors[5]
        )
        .unwrap(),
        7 => writeln!(
            stream,
            "Written by {}, {}, {}, {}, {}, {}, and {}.",
            authors[0], authors[1], authors[2], authors[3], authors[4], authors[5], authors[6]
        )
        .unwrap(),
        8 => writeln!(
            stream,
            "Written by {}, {}, {}, {}, {}, {}, {}, and {}.",
            authors[0],
            authors[1],
            authors[2],
            authors[3],
            authors[4],
            authors[5],
            authors[6],
            authors[7]
        )
        .unwrap(),
        _ => writeln!(
            stream,
            "Written by {}, {}, {}, {}, {}, {}, {}, {}, and others.",
            authors[0],
            authors[1],
            authors[2],
            authors[3],
            authors[4],
            authors[5],
            authors[6],
            authors[7]
        )
        .unwrap(),
    }
}
/// Display the --version information the standard way.
pub fn version_etc_ar(
    stream: &mut dyn std::io::Write,
    command_name: Option<&str>,
    package: &str,
    version: &str,
    authors: &[&str],
) {
    version_etc_arn(stream, command_name, package, version, authors)
}

// fn version_etc_va(
//     stream: &mut dyn std::io::Write,
//     command_name: Option<&str>,
//     package: &str,
//     version: &str,
//     authors: &[Option<&str>],
// ) {
//     let authtab: Vec<&str> = authors
//         .iter()
//         .filter_map(|&author| author)
//         .take(10)
//         .collect();

//     version_etc_arn(stream, command_name, package, version, &authtab)
// }

/// Display the --version information the standard way with variable arguments.
pub fn version_etc(
    stream: &mut dyn std::io::Write,
    command_name: Option<&str>,
    package: &str,
    version: &str,
    authors: &[&str],
) {
    version_etc_ar(stream, command_name, package, version, authors)
}

/// Emit bug reporting address.
pub fn emit_bug_reporting_address(stream: &mut dyn std::io::Write) -> std::io::Result<()> {
    writeln!(stream)?;
    writeln!(stream, "Report bugs to: {}", PACKAGE_BUGREPORT)?;

    writeln!(
        stream,
        "{} home page: <{}>",
        PACKAGE_NAME, "http://www.gnu.org/software/cpio"
    )?;

    writeln!(
        stream,
        "{} home page: <https://www.gnu.org/software/{}>",
        PACKAGE_NAME, PACKAGE
    )?;
    writeln!(
        stream,
        "General help using GNU software: <https://www.gnu.org/gethelp/>"
    )?;
    Ok(())
}
