// SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
//
// # SPDX-License-Identifier: GPL-3.0-or-later

pub fn dir_len(file: &str) -> usize {
    let mut prefix_length: usize = 0;
    let mut length: usize;

    prefix_length += if prefix_length != 0 {
        if file.chars().nth(prefix_length).map_or(false, |c| c == '/') {
            1
        } else {
            0
        }
    } else if file.chars().next().map_or(false, |c| c == '/') {
        if file.chars().nth(1).map_or(false, |c| c == '/')
            && file.chars().nth(2).map_or(true, |c| c != '/')
        {
            2
        } else {
            1
        }
    } else {
        0
    };

    length = file.rfind('/').map_or(file.len(), |i| i);

    while prefix_length < length {
        if file.chars().nth(length - 1).map_or(false, |c| c == '/') {
            length -= 1;
        } else {
            break;
        }
    }
    length
}

pub fn mdir_name(file: &str) -> String {
    let length = dir_len(file);
    let append_dot = length == 0;
    // || (length == 0
    //     && file.chars().nth(2).map_or(false, |c| c != '\0')
    //     && file.chars().nth(2).map_or(true, |c| c != '/'));

    let mut dir = String::with_capacity(length + if append_dot { 1 } else { 0 } + 1);
    dir.push_str(&file[..length]);

    if append_dot {
        dir.push('.');
    }
    // dir.push('\0');  rust 不需要处理的空. push 会自动处理
    dir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dir_len() {
        // Edge cases
        assert_eq!(dir_len(""), 0);
        assert_eq!(dir_len("."), 0);
        assert_eq!(dir_len(".."), 0);

        // Root directory cases
        assert_eq!(dir_len("/"), 1);
        assert_eq!(dir_len("//"), 2);
        assert_eq!(dir_len("///"), 2);

        // Simple paths
        assert_eq!(dir_len("/a"), 1);
        assert_eq!(dir_len("a/"), 1);
        assert_eq!(dir_len("a/b"), 2);
        assert_eq!(dir_len("a//b"), 3);

        // Complex paths
        assert_eq!(dir_len("/usr/bin"), 4);
        assert_eq!(dir_len("/usr/bin/"), 4);
        assert_eq!(dir_len("/usr//bin/"), 5);
        assert_eq!(dir_len("/usr/local/bin"), 9);
        assert_eq!(dir_len("./usr/bin"), 5);
        assert_eq!(dir_len("../usr/bin"), 6);
        assert_eq!(dir_len("home/user/docs/"), 9);
    }

    #[test]
    fn test_mdir_name() {
        // Edge cases
        assert_eq!(mdir_name(""), ".\0");
        assert_eq!(mdir_name("."), ".\0");
        assert_eq!(mdir_name(".."), ".\0");

        // Root directory cases
        assert_eq!(mdir_name("/"), "/\0");
        assert_eq!(mdir_name("//"), "//\0");
        assert_eq!(mdir_name("///"), "//\0");

        // Simple paths
        assert_eq!(mdir_name("/a"), "/\0");
        assert_eq!(mdir_name("a/"), ".\0");
        assert_eq!(mdir_name("a/b"), "a\0");
        assert_eq!(mdir_name("a//b"), "a/\0");

        // Complex paths
        assert_eq!(mdir_name("/usr/bin"), "/usr\0");
        assert_eq!(mdir_name("/usr/bin/"), "/usr\0");
        assert_eq!(mdir_name("/usr//bin/"), "/usr/\0");
        assert_eq!(mdir_name("/usr/local/bin"), "/usr/local\0");
        assert_eq!(mdir_name("./usr/bin"), "./usr\0");
        assert_eq!(mdir_name("../usr/bin"), "../usr\0");
        assert_eq!(mdir_name("home/user/docs/"), "home/user\0");
    }
}
