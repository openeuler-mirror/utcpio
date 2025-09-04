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
