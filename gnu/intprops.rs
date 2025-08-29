/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#![allow(clippy::unnecessary_cast)]

pub trait IsInteger {
    fn is_integer() -> bool {
        true
    }
}

// Implement IsInteger for each integer type individually
impl IsInteger for i8 {}
impl IsInteger for i16 {}
impl IsInteger for i32 {}
impl IsInteger for i64 {}
impl IsInteger for i128 {}
impl IsInteger for isize {}
impl IsInteger for u8 {}
impl IsInteger for u16 {}
impl IsInteger for u32 {}
impl IsInteger for u64 {}
impl IsInteger for u128 {}
impl IsInteger for usize {}

/// Trait for checking if a type is signed
pub trait IsSigned {
    fn is_signed() -> bool;
}

// Individual implementations for IsSigned
impl IsSigned for i8 {
    fn is_signed() -> bool {
        true
    }
}
impl IsSigned for i16 {
    fn is_signed() -> bool {
        true
    }
}
impl IsSigned for i32 {
    fn is_signed() -> bool {
        true
    }
}
impl IsSigned for i64 {
    fn is_signed() -> bool {
        true
    }
}
impl IsSigned for i128 {
    fn is_signed() -> bool {
        true
    }
}
impl IsSigned for isize {
    fn is_signed() -> bool {
        true
    }
}
impl IsSigned for u8 {
    fn is_signed() -> bool {
        false
    }
}
impl IsSigned for u16 {
    fn is_signed() -> bool {
        false
    }
}
impl IsSigned for u32 {
    fn is_signed() -> bool {
        false
    }
}
impl IsSigned for u64 {
    fn is_signed() -> bool {
        false
    }
}
impl IsSigned for u128 {
    fn is_signed() -> bool {
        false
    }
}
impl IsSigned for usize {
    fn is_signed() -> bool {
        false
    }
}

/// Trait for getting the bit width of an integer type
pub trait BitWidth {
    fn bit_width() -> u32;
}

// Individual implementations for BitWidth
impl BitWidth for i8 {
    fn bit_width() -> u32 {
        8
    }
}
impl BitWidth for i16 {
    fn bit_width() -> u32 {
        16
    }
}
impl BitWidth for i32 {
    fn bit_width() -> u32 {
        32
    }
}
impl BitWidth for i64 {
    fn bit_width() -> u32 {
        64
    }
}
impl BitWidth for i128 {
    fn bit_width() -> u32 {
        128
    }
}
impl BitWidth for u8 {
    fn bit_width() -> u32 {
        8
    }
}
impl BitWidth for u16 {
    fn bit_width() -> u32 {
        16
    }
}
impl BitWidth for u32 {
    fn bit_width() -> u32 {
        32
    }
}
impl BitWidth for u64 {
    fn bit_width() -> u32 {
        64
    }
}
impl BitWidth for u128 {
    fn bit_width() -> u32 {
        128
    }
}

/// Extension trait for safe arithmetic operations with overflow checking
pub trait SafeArith: Sized {
    /// Performs addition with overflow checking
    fn checked_add_ext(self, rhs: Self) -> Option<Self>;

    /// Performs subtraction with overflow checking
    fn checked_sub_ext(self, rhs: Self) -> Option<Self>;

    /// Performs multiplication with overflow checking
    fn checked_mul_ext(self, rhs: Self) -> Option<Self>;

    /// Performs division with overflow checking
    fn checked_div_ext(self, rhs: Self) -> Option<Self>;

    /// Performs remainder with overflow checking
    fn checked_rem_ext(self, rhs: Self) -> Option<Self>;

    /// Performs left shift with overflow checking
    fn checked_shl_ext(self, rhs: u32) -> Option<Self>;

    /// Performs negation with overflow checking
    fn checked_neg_ext(self) -> Option<Self>;
}

// Individual implementations for SafeArith
impl SafeArith for i8 {
    fn checked_add_ext(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    fn checked_sub_ext(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
    fn checked_mul_ext(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs)
    }
    fn checked_div_ext(self, rhs: Self) -> Option<Self> {
        self.checked_div(rhs)
    }
    fn checked_rem_ext(self, rhs: Self) -> Option<Self> {
        self.checked_rem(rhs)
    }
    fn checked_shl_ext(self, rhs: u32) -> Option<Self> {
        self.checked_shl(rhs)
    }
    fn checked_neg_ext(self) -> Option<Self> {
        self.checked_neg()
    }
}

impl SafeArith for i16 {
    fn checked_add_ext(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    fn checked_sub_ext(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
    fn checked_mul_ext(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs)
    }
    fn checked_div_ext(self, rhs: Self) -> Option<Self> {
        self.checked_div(rhs)
    }
    fn checked_rem_ext(self, rhs: Self) -> Option<Self> {
        self.checked_rem(rhs)
    }
    fn checked_shl_ext(self, rhs: u32) -> Option<Self> {
        self.checked_shl(rhs)
    }
    fn checked_neg_ext(self) -> Option<Self> {
        self.checked_neg()
    }
}

impl SafeArith for i32 {
    fn checked_add_ext(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    fn checked_sub_ext(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
    fn checked_mul_ext(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs)
    }
    fn checked_div_ext(self, rhs: Self) -> Option<Self> {
        self.checked_div(rhs)
    }
    fn checked_rem_ext(self, rhs: Self) -> Option<Self> {
        self.checked_rem(rhs)
    }
    fn checked_shl_ext(self, rhs: u32) -> Option<Self> {
        self.checked_shl(rhs)
    }
    fn checked_neg_ext(self) -> Option<Self> {
        self.checked_neg()
    }
}

impl SafeArith for i64 {
    fn checked_add_ext(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    fn checked_sub_ext(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
    fn checked_mul_ext(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs)
    }
    fn checked_div_ext(self, rhs: Self) -> Option<Self> {
        self.checked_div(rhs)
    }
    fn checked_rem_ext(self, rhs: Self) -> Option<Self> {
        self.checked_rem(rhs)
    }
    fn checked_shl_ext(self, rhs: u32) -> Option<Self> {
        self.checked_shl(rhs)
    }
    fn checked_neg_ext(self) -> Option<Self> {
        self.checked_neg()
    }
}

impl SafeArith for i128 {
    fn checked_add_ext(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    fn checked_sub_ext(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
    fn checked_mul_ext(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs)
    }
    fn checked_div_ext(self, rhs: Self) -> Option<Self> {
        self.checked_div(rhs)
    }
    fn checked_rem_ext(self, rhs: Self) -> Option<Self> {
        self.checked_rem(rhs)
    }
    fn checked_shl_ext(self, rhs: u32) -> Option<Self> {
        self.checked_shl(rhs)
    }
    fn checked_neg_ext(self) -> Option<Self> {
        self.checked_neg()
    }
}

impl SafeArith for isize {
    fn checked_add_ext(self, rhs: Self) -> Option<Self> {
        self.checked_add(rhs)
    }
    fn checked_sub_ext(self, rhs: Self) -> Option<Self> {
        self.checked_sub(rhs)
    }
    fn checked_mul_ext(self, rhs: Self) -> Option<Self> {
        self.checked_mul(rhs)
    }
    fn checked_div_ext(self, rhs: Self) -> Option<Self> {
        self.checked_div(rhs)
    }
    fn checked_rem_ext(self, rhs: Self) -> Option<Self> {
        self.checked_rem(rhs)
    }
    fn checked_shl_ext(self, rhs: u32) -> Option<Self> {
        self.checked_shl(rhs)
    }
    fn checked_neg_ext(self) -> Option<Self> {
        self.checked_neg()
    }
}
