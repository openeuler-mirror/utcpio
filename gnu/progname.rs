/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::unnecessary_cast, clippy::ptr_arg)]

use crate::argp::*;

pub fn set_program_name(argv0: &String) {
    // Sanity check. POSIX requires the invoking process to pass a non-NULL argv[0].
    if argv0.is_empty() {
        eprintln!("A NULL argv[0] was passed through an exec system call.");
        std::process::abort();
    }

    //    let c_str: &CStr = unsafe { CStr::from_ptr(argv0) };
    let program_name_str: String = argv0.to_lowercase();

    // Remove the "<dirname>/.libs/" or "<dirname>/.libs/lt-" prefix here.
    let base: &str = program_name_str
        .rsplit('/')
        .next()
        .unwrap_or(&program_name_str);
    let mut new_program_name = base.to_string();

    if new_program_name.len() >= 7 && new_program_name[..7] == *".libs/" {
        new_program_name = new_program_name[7..].to_string();
        if new_program_name.starts_with("lt-") {
            new_program_name = new_program_name[3..].to_string();
        }
    }

    bind_program_name(Some(new_program_name.clone()));
    set_program_invocation_name(Some(new_program_name));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // 模拟全局状态的简单mock
    static PROGRAM_NAME: Mutex<Option<String>> = Mutex::new(None);
    static INVOCATION_NAME: Mutex<Option<String>> = Mutex::new(None);

    fn bind_program_name(name: Option<String>) {
        *PROGRAM_NAME.lock().unwrap() = name;
    }

    fn set_program_invocation_name(name: Option<String>) {
        *INVOCATION_NAME.lock().unwrap() = name;
    }

    #[test]
    fn test_empty_argv0() {
        let argv0 = String::new();
        set_program_name(&argv0);
        // 测试是否调用了abort()
        // 由于abort()会终止进程，这个测试实际上无法验证
        // 在实际测试中可能需要使用mock或修改函数设计
    }

    #[test]
    fn test_basic_program_name() {
        let argv0 = String::from("/usr/bin/myprogram");
        set_program_name(&argv0);

        let program_name = PROGRAM_NAME.lock().unwrap().clone();
        let invocation_name = INVOCATION_NAME.lock().unwrap().clone();

        assert_eq!(program_name, Some("myprogram".to_string()));
        assert_eq!(invocation_name, Some("myprogram".to_string()));
    }

    #[test]
    fn test_libs_prefix() {
        let argv0 = String::from("/path/.libs/lt-myprogram");
        set_program_name(&argv0);

        let program_name = PROGRAM_NAME.lock().unwrap().clone();
        let invocation_name = INVOCATION_NAME.lock().unwrap().clone();

        assert_eq!(program_name, Some("myprogram".to_string()));
        assert_eq!(invocation_name, Some("myprogram".to_string()));
    }

    #[test]
    fn test_libs_without_lt_prefix() {
        let argv0 = String::from("/path/.libs/myprogram");
        set_program_name(&argv0);

        let program_name = PROGRAM_NAME.lock().unwrap().clone();
        let invocation_name = INVOCATION_NAME.lock().unwrap().clone();

        assert_eq!(program_name, Some("myprogram".to_string()));
        assert_eq!(invocation_name, Some("myprogram".to_string()));
    }

    #[test]
    fn test_lowercase_conversion() {
        let argv0 = String::from("/usr/bin/MYPROGRAM");
        set_program_name(&argv0);

        let program_name = PROGRAM_NAME.lock().unwrap().clone();
        let invocation_name = INVOCATION_NAME.lock().unwrap().clone();

        assert_eq!(program_name, Some("myprogram".to_string()));
        assert_eq!(invocation_name, Some("myprogram".to_string()));
    }
}
