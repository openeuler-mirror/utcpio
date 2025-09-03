/*
 * SPDX-FileCopyrightText: 2025 UnionTech Software Technology Co., Ltd.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

pub fn umaxtostr<T>(value: T) -> String
where
    T: Into<u128> + Copy,
{
    let mut result = String::with_capacity(40); // Max u128 digits + 1 for null
    let mut n = value.into();

    if n == 0 {
        return "0".to_string();
    }

    // Convert digits in reverse order
    let mut digits = Vec::with_capacity(40);
    while n > 0 {
        digits.push((b'0' + (n % 10) as u8) as char);
        n /= 10;
    }

    // Reverse the digits to get the correct order
    for digit in digits.into_iter().rev() {
        result.push(digit);
    }

    result
}

/// A more efficient version that writes directly to a string buffer
///
/// This version avoids allocating a temporary vector for digits
pub fn umaxtostr_efficient<T>(value: T) -> String
where
    T: Into<u128> + Copy,
{
    if value.into() == 0 {
        return "0".to_string();
    }

    // Calculate required capacity based on number size
    let mut n = value.into();
    let mut len = 0;
    let mut temp = n;
    while temp > 0 {
        len += 1;
        temp /= 10;
    }

    let mut result = String::with_capacity(len);

    // Convert number to string in reverse
    while n > 0 {
        let digit = (b'0' + (n % 10) as u8) as char;
        result.insert(0, digit);
        n /= 10;
    }

    result
}

/// A version that writes to a provided string buffer
///
/// This version allows reusing an existing String buffer
pub fn umaxtostr_buf<T>(value: T, buf: &mut String) -> &str
where
    T: Into<u128> + Copy,
{
    buf.clear();

    let mut n = value.into();
    if n == 0 {
        buf.push('0');
        return buf;
    }

    // Convert number to string
    while n > 0 {
        let digit = (b'0' + (n % 10) as u8) as char;
        buf.insert(0, digit);
        n /= 10;
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_umaxtostr() {
        assert_eq!(umaxtostr(0u64), "0");
        assert_eq!(umaxtostr(123u64), "123");
        assert_eq!(umaxtostr(u64::MAX), "18446744073709551615");
        assert_eq!(umaxtostr(42u32), "42");
    }

    #[test]
    fn test_umaxtostr_efficient() {
        assert_eq!(umaxtostr_efficient(0u64), "0");
        assert_eq!(umaxtostr_efficient(123u64), "123");
        assert_eq!(umaxtostr_efficient(u64::MAX), "18446744073709551615");
        assert_eq!(umaxtostr_efficient(42u32), "42");
    }

    #[test]
    fn test_umaxtostr_buf() {
        let mut buf = String::with_capacity(20);

        assert_eq!(umaxtostr_buf(0u64, &mut buf), "0");
        assert_eq!(umaxtostr_buf(123u64, &mut buf), "123");
        assert_eq!(umaxtostr_buf(u64::MAX, &mut buf), "18446744073709551615");
        assert_eq!(umaxtostr_buf(42u32, &mut buf), "42");
    }

    #[test]
    fn test_large_numbers() {
        let large = u128::MAX;
        assert_eq!(umaxtostr(large).parse::<u128>().unwrap(), large);
        assert_eq!(umaxtostr_efficient(large).parse::<u128>().unwrap(), large);

        let mut buf = String::new();
        assert_eq!(
            umaxtostr_buf(large, &mut buf).parse::<u128>().unwrap(),
            large
        );
    }
}
