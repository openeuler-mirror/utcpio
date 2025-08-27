/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

use libc::{clock_gettime, timespec};

pub fn gettime(ts: &mut timespec) {
    unsafe { clock_gettime(0 as libc::c_int, ts) };
}
pub fn current_timespec() -> timespec {
    let mut ts: timespec = timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    gettime(&mut ts);
    ts
}

#[cfg(test)]
mod tests {
    use super::*;
    use libc::timespec;

    #[test]
    fn test_gettime_updates_timespec() {
        let mut ts = timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        gettime(&mut ts);

        // 验证时间已被更新(至少不是0)
        assert!(ts.tv_sec > 0 || ts.tv_nsec > 0);
    }

    #[test]
    fn test_current_timespec_returns_valid_time() {
        let ts = current_timespec();

        // 验证返回的时间结构体包含有效值
        assert!(ts.tv_sec > 0 || ts.tv_nsec > 0);
    }

    #[test]
    fn test_current_timespec_initialization() {
        let ts = current_timespec();

        // 验证tv_sec和tv_nsec至少有一个大于0
        assert!(ts.tv_sec > 0 || ts.tv_nsec > 0);
    }
}
