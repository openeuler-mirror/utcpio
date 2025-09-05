/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::unnecessary_cast)]

use std::cmp::{max, min};

use crate::xalloc_die::xalloc_die;

// Replacement for xmalloc
pub fn xmalloc(size: usize) -> Vec<u8> {
    vec![0; size]
}

// Replacement for ximalloc (assuming idx_t is isize)
pub fn ximalloc(size: isize) -> Vec<u8> {
    if size < 0 {
        xalloc_die();
    }
    xmalloc(size as usize)
}

// Replacement for xcharalloc
pub fn xcharalloc(n: usize) -> Vec<char> {
    vec!['\0'; n]
}

// Replacement for xrealloc
pub fn xrealloc(mut vec: Vec<u8>, new_size: usize) -> Vec<u8> {
    vec.resize(new_size, 0);
    vec
}

// Replacement for xirealloc (assuming idx_t is isize)
pub fn xirealloc(vec: Vec<u8>, new_size: isize) -> Vec<u8> {
    if new_size < 0 {
        xalloc_die();
    }
    xrealloc(vec, new_size as usize)
}

// Replacement for xreallocarray
pub fn xreallocarray(mut vec: Vec<u8>, n: usize, s: usize) -> Vec<u8> {
    if n == 0 || s == 0 {
        vec.clear();
        return vec;
    }
    vec.resize(n * s, 0);
    vec
}

// Replacement for xireallocarray (assuming idx_t is isize)
pub fn xireallocarray(vec: Vec<u8>, n: isize, s: isize) -> Vec<u8> {
    if n < 0 || s < 0 {
        xalloc_die();
    }
    xreallocarray(vec, n as usize, s as usize)
}

// Replacement for xnmalloc
pub fn xnmalloc(n: usize, s: usize) -> Vec<u8> {
    xreallocarray(Vec::new(), n, s)
}

pub fn x2realloc(vec: Vec<u8>, ps: &mut usize) -> Vec<u8> {
    x2nrealloc(vec, ps, 1)
}
// Replacement for xinmalloc (assuming idx_t is isize)
pub fn xinmalloc(n: isize, s: isize) -> Vec<u8> {
    if n < 0 || s < 0 {
        xalloc_die();
    }
    xnmalloc(n as usize, s as usize)
}

// Replacement for x2realloc and x2nrealloc
pub fn x2nrealloc(mut vec: Vec<u8>, pn: &mut usize, s: usize) -> Vec<u8> {
    let mut n = *pn;

    if vec.is_empty() {
        if n == 0 {
            const DEFAULT_MXFAST: usize = 64 * std::mem::size_of::<usize>() / 4;
            n = max(DEFAULT_MXFAST / s, 1);
        }
    } else {
        n = n.saturating_add((n >> 1) + 1); // Use saturating add to prevent panics
    }

    vec.resize(n * s, 0);
    *pn = n;
    vec
}

// Replacement for xpalloc (assuming idx_t is isize and ptrdiff_t is isize)
pub fn xpalloc(
    mut vec: Vec<u8>,
    pn: &mut isize,
    n_incr_min: isize,
    n_max: isize,
    s: isize,
) -> Vec<u8> {
    let n0 = *pn;
    const DEFAULT_MXFAST: usize = 64 * std::mem::size_of::<usize>() / 4;

    let mut n = n0.saturating_add(n0 >> 1); // Use saturating add
    if n_max >= 0 && n_max < n {
        n = n_max;
    }

    let adjusted_nbytes = if let Some(nbytes) = n.checked_mul(s) {
        if nbytes < DEFAULT_MXFAST as isize {
            DEFAULT_MXFAST
        } else {
            0
        }
    } else {
        min(isize::MAX as usize, usize::MAX)
    };

    if adjusted_nbytes != 0 {
        n = adjusted_nbytes as isize / s as isize;
    }

    if vec.is_empty() {
        *pn = 0;
    }

    if (n as isize).saturating_sub(n0) < n_incr_min
        && ((n0.saturating_add(n_incr_min) > n_max && n_max >= 0)
            || (n0.saturating_add(n_incr_min).checked_mul(s).is_none()))
    {
        xalloc_die();
    }

    vec.resize(n as usize * s as usize, 0);
    *pn = n as isize;
    vec
}

// Replacement for xzalloc and xizalloc
pub fn xzalloc(size: usize) -> Vec<u8> {
    xcalloc(size, 1)
}

pub fn xizalloc(size: isize) -> Vec<u8> {
    if size < 0 {
        xalloc_die();
    }
    xzalloc(size as usize)
}

// Replacement for xcalloc and xicalloc
pub fn xcalloc(n: usize, s: usize) -> Vec<u8> {
    vec![0; n * s]
}

pub fn xicalloc(n: isize, s: isize) -> Vec<u8> {
    if n < 0 || s < 0 {
        xalloc_die();
    }
    xcalloc(n as usize, s as usize)
}

// Replacement for xmemdup and ximemdup
pub fn xmemdup(p: &[u8]) -> Vec<u8> {
    p.to_vec()
}

pub fn ximemdup(p: &[u8]) -> Vec<u8> {
    xmemdup(p)
}

// Replacement for ximemdup0
pub fn ximemdup0(p: &[u8], s: i32) -> String {
    let mut vec: Vec<u8> = p.to_vec();
    vec[s as usize] = 0;
    String::from_utf8(vec).expect("String::from_utf8 failed")
}

pub fn xstrdup(s: &str) -> String {
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xmalloc() {
        let vec = xmalloc(5);
        assert_eq!(vec.len(), 5);
        assert_eq!(vec, vec![0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_ximalloc() {
        let vec = ximalloc(3);
        assert_eq!(vec.len(), 3);

        #[should_panic]
        fn test_ximalloc_negative() {
            ximalloc(-1);
        }
    }

    #[test]
    fn test_xcharalloc() {
        let vec = xcharalloc(2);
        assert_eq!(vec.len(), 2);
        assert_eq!(vec, vec!['\0', '\0']);
    }

    #[test]
    fn test_xrealloc() {
        let mut vec = vec![1, 2, 3];
        vec = xrealloc(vec, 5);
        assert_eq!(vec.len(), 5);
        assert_eq!(vec, vec![1, 2, 3, 0, 0]);
    }

    #[test]
    fn test_xirealloc() {
        let vec = vec![1, 2];
        let vec = xirealloc(vec, 4);
        assert_eq!(vec.len(), 4);

        #[should_panic]
        fn test_xirealloc_negative() {
            xirealloc(vec![], -1);
        }
    }

    #[test]
    fn test_xreallocarray() {
        let vec = vec![1, 2];
        let vec = xreallocarray(vec, 2, 3);
        assert_eq!(vec.len(), 6);

        let vec = xreallocarray(vec, 0, 5);
        assert!(vec.is_empty());
    }

    #[test]
    fn test_xireallocarray() {
        let vec = vec![1];
        let vec = xireallocarray(vec, 2, 2);
        assert_eq!(vec.len(), 4);

        #[should_panic]
        fn test_xireallocarray_negative() {
            xireallocarray(vec![], -1, 2);
        }
    }

    #[test]
    fn test_xnmalloc() {
        let vec = xnmalloc(2, 3);
        assert_eq!(vec.len(), 6);
    }

    #[test]
    fn test_xinmalloc() {
        let vec = xinmalloc(2, 3);
        assert_eq!(vec.len(), 6);

        #[should_panic]
        fn test_xinmalloc_negative() {
            xinmalloc(-1, 2);
        }
    }

    #[test]
    fn test_x2nrealloc() {
        let mut n = 0;
        let vec = x2nrealloc(Vec::new(), &mut n, 2);
        assert!(n > 0);
        assert_eq!(vec.len(), n * 2);

        let mut n = 5;
        let vec = x2nrealloc(vec![1, 2, 3], &mut n, 1);
        assert!(n > 5);
        assert_eq!(vec.len(), n);
    }

    #[test]
    fn test_xpalloc() {
        let mut n = 5;
        let vec = xpalloc(Vec::new(), &mut n, 1, 10, 2);
        assert_eq!(n, 5);
        assert_eq!(vec.len(), 10);

        #[should_panic]
        fn test_xpalloc_overflow() {
            let mut n = isize::MAX;
            xpalloc(Vec::new(), &mut n, 1, isize::MAX, 2);
        }
    }

    #[test]
    fn test_xzalloc() {
        let vec = xzalloc(4);
        assert_eq!(vec.len(), 4);
    }

    #[test]
    fn test_xizalloc() {
        let vec = xizalloc(3);
        assert_eq!(vec.len(), 3);

        #[should_panic]
        fn test_xizalloc_negative() {
            xizalloc(-1);
        }
    }

    #[test]
    fn test_xcalloc() {
        let vec = xcalloc(2, 3);
        assert_eq!(vec.len(), 6);
    }

    #[test]
    fn test_xicalloc() {
        let vec = xicalloc(2, 3);
        assert_eq!(vec.len(), 6);

        #[should_panic]
        fn test_xicalloc_negative() {
            xicalloc(-1, 2);
        }
    }

    #[test]
    fn test_xmemdup() {
        let src = [1, 2, 3];
        let vec = xmemdup(&src);
        assert_eq!(vec, src);
    }

    #[test]
    fn test_ximemdup() {
        let src = [4, 5];
        let vec = ximemdup(&src);
        assert_eq!(vec, src);
    }

    #[test]
    fn test_ximemdup0() {
        let src = b"hello";
        let s = ximemdup0(src, 3);
        assert_eq!(s, "hel\0lo");
    }

    #[test]
    fn test_xstrdup() {
        let s = "test";
        assert_eq!(xstrdup(s), "test");
    }
}
