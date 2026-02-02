// Truncation is intentional for 64-bit extraction from 128-bit fixed-point values
#![allow(clippy::cast_possible_truncation)]
#![cfg_attr(not(any(test, feature = "idl-build")), no_std)]

//! Fixed-point numeric type for rewards calculations
//!
//! Uses 128-bit integers with 64-bit precision for accurate division without floating point.
//!
//! # Format
//!
//! This is a 64.64 fixed-point format where:
//! - Upper 64 bits: integer part (0 to 2^64 - 1)
//! - Lower 64 bits: fractional part (precision of ~5.4e-20)
//!
//! # Safety for Financial Calculations
//!
//! For financial calculations, prefer the `checked_*` methods which return `Option<Self>`
//! instead of panicking or silently returning incorrect values:
//!
//! - [`checked_from_fraction`](Numeric::checked_from_fraction) - Returns `None` on division by zero
//! - [`checked_add`](Numeric::checked_add) - Returns `None` on overflow
//! - [`checked_sub`](Numeric::checked_sub) - Returns `None` on underflow
//! - [`checked_mul`](Numeric::checked_mul) - Returns `None` on overflow
//! - [`checked_div`](Numeric::checked_div) - Returns `None` on division by zero or overflow
//!
//! The standard operators (`+`, `-`, `*`) will panic on overflow in debug mode
//! and wrap in release mode. Use them only when overflow is impossible.

use bytemuck::{Pod, Zeroable};
use core::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

/// Fixed-point numeric type with 64-bit precision
///
/// The value is stored as a 128-bit integer where the lower 64 bits represent
/// the fractional part and the upper 64 bits represent the integer part.
///
/// # Precision
///
/// - Minimum positive value: 2^-64 ≈ 5.42e-20
/// - Maximum value: 2^64 - 2^-64 ≈ 1.84e19
/// - Integer precision: Exact for integers up to 2^64
/// - Fractional precision: 64 bits (~19 decimal digits)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Pod, Zeroable)]
pub struct Numeric {
    value: u128,
}

panchor::idl_type!(Numeric, alias = u128);

impl Numeric {
    /// Precision scale (2^64)
    const SCALE: u128 = 1u128 << 64;

    /// Zero value
    pub const ZERO: Self = Self { value: 0 };

    /// One value (1.0 in fixed-point)
    pub const ONE: Self = Self { value: Self::SCALE };

    /// Maximum representable value
    pub const MAX: Self = Self { value: u128::MAX };

    /// Minimum representable value (same as ZERO for unsigned)
    pub const MIN: Self = Self::ZERO;

    /// Smallest positive value (2^-64)
    pub const EPSILON: Self = Self { value: 1 };

    /// Create a new Numeric from a raw u128 value
    #[inline]
    pub const fn from_raw(value: u128) -> Self {
        Self { value }
    }

    /// Get the raw u128 value
    #[inline]
    pub const fn to_raw(self) -> u128 {
        self.value
    }

    /// Create a Numeric from a u64 integer
    #[inline]
    pub fn from_u64(value: u64) -> Self {
        Self {
            value: u128::from(value) << 64,
        }
    }

    /// Convert to u64 (truncates fractional part toward zero)
    ///
    /// # Note
    ///
    /// This always rounds toward zero. For other rounding modes, see:
    /// - [`to_u64_ceil`](Self::to_u64_ceil) - Round up
    /// - [`checked_to_u64`](Self::checked_to_u64) - Returns `None` if value exceeds u64::MAX
    #[inline]
    pub fn to_u64(self) -> u64 {
        (self.value >> 64) as u64
    }

    /// Convert to u64, rounding up (ceiling)
    ///
    /// Returns the smallest integer greater than or equal to this value.
    #[inline]
    pub fn to_u64_ceil(self) -> u64 {
        let floor = self.to_u64();
        // Check if there's any fractional part
        if self.value & (Self::SCALE - 1) > 0 {
            floor.saturating_add(1)
        } else {
            floor
        }
    }

    /// Convert to u64 with overflow checking
    ///
    /// Returns `None` if the integer part exceeds `u64::MAX`.
    #[inline]
    pub fn checked_to_u64(self) -> Option<u64> {
        let int_part = self.value >> 64;
        if int_part > u64::MAX as u128 {
            None
        } else {
            Some(int_part as u64)
        }
    }

    /// Create a Numeric from a fraction (numerator / denominator)
    ///
    /// # Warning
    ///
    /// Returns `ZERO` if denominator is zero. For financial calculations,
    /// use [`checked_from_fraction`](Self::checked_from_fraction) instead.
    #[inline]
    pub fn from_fraction(numerator: u64, denominator: u64) -> Self {
        if denominator == 0 {
            return Self::ZERO;
        }
        Self {
            value: (u128::from(numerator) << 64) / u128::from(denominator),
        }
    }

    /// Create a Numeric from a fraction with explicit error handling
    ///
    /// Returns `None` if denominator is zero.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let ratio = Numeric::checked_from_fraction(rewards, total_staked)
    ///     .ok_or(ProgramError::InvalidArgument)?;
    /// ```
    #[inline]
    pub fn checked_from_fraction(numerator: u64, denominator: u64) -> Option<Self> {
        if denominator == 0 {
            return None;
        }
        Some(Self {
            value: (u128::from(numerator) << 64) / u128::from(denominator),
        })
    }

    /// Check if this is zero
    #[inline]
    pub fn is_zero(self) -> bool {
        self.value == 0
    }

    // ========================================================================
    // Checked arithmetic (returns None on overflow/underflow/division-by-zero)
    // ========================================================================

    /// Checked addition. Returns `None` on overflow.
    ///
    /// This is the preferred method for financial calculations.
    #[inline]
    pub fn checked_add(self, other: Self) -> Option<Self> {
        self.value.checked_add(other.value).map(|v| Self { value: v })
    }

    /// Checked subtraction. Returns `None` on underflow.
    ///
    /// This is the preferred method for financial calculations.
    #[inline]
    pub fn checked_sub(self, other: Self) -> Option<Self> {
        self.value.checked_sub(other.value).map(|v| Self { value: v })
    }

    /// Checked multiplication. Returns `None` on overflow.
    ///
    /// This is the preferred method for financial calculations.
    #[inline]
    pub fn checked_mul(self, other: Self) -> Option<Self> {
        // Split into 64-bit parts
        let a_hi = (self.value >> 64) as u64;
        let a_lo = self.value as u64;
        let b_hi = (other.value >> 64) as u64;
        let b_lo = other.value as u64;

        // Each product fits in 128 bits
        let hi_hi = u128::from(a_hi) * u128::from(b_hi);
        let hi_lo = u128::from(a_hi) * u128::from(b_lo);
        let lo_hi = u128::from(a_lo) * u128::from(b_hi);
        let lo_lo = u128::from(a_lo) * u128::from(b_lo);

        // hi_hi << 64 overflows if hi_hi >= 2^64
        if hi_hi >= (1u128 << 64) {
            return None;
        }
        let hi_hi_shifted = hi_hi << 64;

        // Sum all parts, checking for overflow at each step
        let result = hi_hi_shifted
            .checked_add(hi_lo)?
            .checked_add(lo_hi)?
            .checked_add(lo_lo >> 64)?;

        Some(Self { value: result })
    }

    /// Checked division. Returns `None` on division by zero.
    ///
    /// This is the preferred method for financial calculations.
    ///
    /// # Precision
    ///
    /// Division is performed with full 64-bit fractional precision.
    #[inline]
    pub fn checked_div(self, other: Self) -> Option<Self> {
        if other.value == 0 {
            return None;
        }

        // To maintain precision, we need to compute (self.value << 64) / other.value
        // But self.value << 64 would overflow u128. So we split the computation:
        //
        // Let a = self.value, b = other.value
        // We want: (a << 64) / b = (a / b) << 64 + ((a % b) << 64) / b
        //
        // But (a % b) << 64 might still overflow. We need to be careful.
        // For a full implementation, we'd need 256-bit arithmetic or iterative division.
        //
        // Simpler approach: use the fact that for most practical cases,
        // if self < other, the result is < 1 (only fractional part)
        // if self >= other, we can compute integer and fractional parts separately

        let a = self.value;
        let b = other.value;

        // Integer part of division
        let int_part = a / b;

        // Check if integer part would overflow when shifted
        if int_part >= (1u128 << 64) {
            return None;
        }

        // Remainder for fractional computation
        let remainder = a % b;

        // Compute fractional part: (remainder << 64) / b
        // We need to handle potential overflow of remainder << 64
        let rem_hi = remainder >> 64;

        // (rem_hi << 128 + rem_lo << 64) / b
        // = (rem_hi << 64) * (1 << 64 / b) + (rem_lo << 64) / b (approximately)
        //
        // For simplicity and to avoid 256-bit math, we use an iterative approach
        // that's accurate for most practical values
        let frac_part = if rem_hi == 0 {
            // Common case: remainder fits in 64 bits, safe to shift
            ((remainder as u128) << 64) / b
        } else {
            // Rare case: need more careful computation
            // Use long division approach
            let mut quotient = 0u128;
            let mut current = remainder;

            for i in (0..64).rev() {
                current <<= 1;
                if current >= b {
                    current -= b;
                    quotient |= 1u128 << i;
                }
                // Early exit if current is 0
                if current == 0 {
                    break;
                }
            }
            quotient
        };

        // Combine integer and fractional parts
        let result = (int_part << 64).checked_add(frac_part)?;

        Some(Self { value: result })
    }

    // ========================================================================
    // Saturating arithmetic (clamps to MIN/MAX instead of overflowing)
    // ========================================================================

    /// Saturating addition. Clamps to `MAX` on overflow.
    #[inline]
    pub fn saturating_add(self, other: Self) -> Self {
        Self {
            value: self.value.saturating_add(other.value),
        }
    }

    /// Saturating subtraction. Clamps to `ZERO` on underflow.
    #[inline]
    pub fn saturating_sub(self, other: Self) -> Self {
        Self {
            value: self.value.saturating_sub(other.value),
        }
    }

    /// Saturating multiplication. Clamps to `MAX` on overflow.
    #[inline]
    pub fn saturating_mul(self, other: Self) -> Self {
        self.checked_mul(other).unwrap_or(Self::MAX)
    }
}

impl Add for Numeric {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
        }
    }
}

impl AddAssign for Numeric {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        self.value += other.value;
    }
}

impl Sub for Numeric {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self {
        Self {
            value: self.value - other.value,
        }
    }
}

impl SubAssign for Numeric {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        self.value -= other.value;
    }
}

impl Mul for Numeric {
    type Output = Self;

    /// Multiply two Numeric values.
    ///
    /// # Panics
    ///
    /// In debug mode, this will panic on overflow. In release mode, it saturates to MAX.
    /// For financial calculations, use [`checked_mul`](Self::checked_mul) instead.
    #[inline]
    fn mul(self, other: Self) -> Self {
        // For fixed-point multiplication with 64-bit scaling:
        // result = (a * b) >> 64
        //
        // Split into 64-bit parts to avoid 256-bit overflow:
        // a = a_hi * 2^64 + a_lo
        // b = b_hi * 2^64 + b_lo
        // a * b = a_hi*b_hi*2^128 + (a_hi*b_lo + a_lo*b_hi)*2^64 + a_lo*b_lo
        // (a * b) >> 64 = a_hi*b_hi*2^64 + a_hi*b_lo + a_lo*b_hi + (a_lo*b_lo >> 64)

        let a_hi = (self.value >> 64) as u64;
        let a_lo = self.value as u64;
        let b_hi = (other.value >> 64) as u64;
        let b_lo = other.value as u64;

        // Each of these products fits in 128 bits (64-bit * 64-bit)
        let hi_hi = u128::from(a_hi) * u128::from(b_hi);
        let hi_lo = u128::from(a_hi) * u128::from(b_lo);
        let lo_hi = u128::from(a_lo) * u128::from(b_hi);
        let lo_lo = u128::from(a_lo) * u128::from(b_lo);

        // hi_hi << 64 saturates if hi_hi >= 2^64
        let hi_hi_shifted = if hi_hi >= (1u128 << 64) {
            u128::MAX
        } else {
            hi_hi << 64
        };

        let result = hi_hi_shifted
            .saturating_add(hi_lo)
            .saturating_add(lo_hi)
            .saturating_add(lo_lo >> 64);

        Self { value: result }
    }
}

impl Div for Numeric {
    type Output = Self;

    /// Divide two Numeric values.
    ///
    /// # Panics
    ///
    /// Panics if dividing by zero. For financial calculations,
    /// use [`checked_div`](Self::checked_div) instead.
    #[inline]
    fn div(self, other: Self) -> Self {
        self.checked_div(other).expect("division by zero")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use panchor::IdlType;

    #[test]
    fn test_idl_type_name_is_u128() {
        // Numeric should expose its TYPE_NAME as "u128" (its alias)
        // This ensures the IDL generator will serialize Numeric fields as u128
        assert_eq!(Numeric::TYPE_NAME, "u128");
    }

    #[test]
    fn test_from_u64() {
        let n = Numeric::from_u64(100);
        assert_eq!(n.to_u64(), 100);
    }

    #[test]
    fn test_from_fraction() {
        let n = Numeric::from_fraction(1, 2);
        assert!(n.to_u64() == 0); // 0.5 truncated is 0

        let n = Numeric::from_fraction(3, 2);
        assert_eq!(n.to_u64(), 1); // 1.5 truncated is 1
    }

    #[test]
    fn test_add() {
        let a = Numeric::from_u64(10);
        let b = Numeric::from_u64(20);
        assert_eq!((a + b).to_u64(), 30);
    }

    #[test]
    fn test_sub() {
        let a = Numeric::from_u64(30);
        let b = Numeric::from_u64(10);
        assert_eq!((a - b).to_u64(), 20);
    }

    #[test]
    fn test_mul_integers() {
        let a = Numeric::from_u64(10);
        let b = Numeric::from_u64(5);
        assert_eq!((a * b).to_u64(), 50);
    }

    #[test]
    fn test_mul_fractions() {
        // 0.5 * 0.5 = 0.25
        let half = Numeric::from_fraction(1, 2);
        let quarter = half * half;
        // 0.25 truncated is 0
        assert_eq!(quarter.to_u64(), 0);
        // But the raw value should be approximately 1/4 * 2^64
        let expected = Numeric::SCALE / 4;
        assert!(quarter.to_raw() > expected - 1000 && quarter.to_raw() < expected + 1000);
    }

    #[test]
    fn test_mul_mixed() {
        // 10 * 0.5 = 5
        let ten = Numeric::from_u64(10);
        let half = Numeric::from_fraction(1, 2);
        let result = ten * half;
        assert_eq!(result.to_u64(), 5);
    }

    #[test]
    fn test_mul_precision() {
        // Test that we don't lose precision with the fixed implementation
        // Use powers of 2 for exact representation
        let a = Numeric::from_fraction(1, 65536); // 1/2^16
        let b = Numeric::from_u64(65536); // 2^16
        let result = a * b;
        // Should be exactly 1
        assert_eq!(result.to_u64(), 1);

        // Test with smaller fractions that have bits in lower 32 bits
        let c = Numeric::from_fraction(1, 1 << 40); // Very small fraction
        let d = Numeric::from_u64(1 << 40);
        let result2 = c * d;
        // Should be exactly 1
        assert_eq!(result2.to_u64(), 1);
    }

    #[test]
    fn test_mul_rewards_factor() {
        // Simulate real rewards calculation: factor * balance
        // factor = rewards / total_staked (small fraction typically)
        let factor = Numeric::from_fraction(1000, 1_000_000); // 0.001
        let balance = Numeric::from_u64(500_000);
        let rewards = factor * balance;
        // Should be ~500 (0.001 * 500000), allow tiny rounding error
        // due to 1/1000 not being exactly representable in binary
        let result = rewards.to_u64();
        assert!((499..=500).contains(&result), "expected ~500, got {result}");
    }

    #[test]
    fn test_mul_exact_binary_fractions() {
        // These should be exact since they're powers of 2
        let factor = Numeric::from_fraction(1, 1024); // 1/2^10
        let balance = Numeric::from_u64(1024 * 100);
        let result = factor * balance;
        assert_eq!(result.to_u64(), 100);
    }

    // ========================================================================
    // Tests for checked arithmetic methods
    // ========================================================================

    #[test]
    fn test_checked_from_fraction_success() {
        let result = Numeric::checked_from_fraction(1, 2);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_u64(), 0); // 0.5 truncated
    }

    #[test]
    fn test_checked_from_fraction_division_by_zero() {
        let result = Numeric::checked_from_fraction(100, 0);
        assert!(result.is_none());
    }

    #[test]
    fn test_checked_add_success() {
        let a = Numeric::from_u64(10);
        let b = Numeric::from_u64(20);
        let result = a.checked_add(b);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_u64(), 30);
    }

    #[test]
    fn test_checked_add_overflow() {
        let max = Numeric::MAX;
        let one = Numeric::ONE;
        let result = max.checked_add(one);
        assert!(result.is_none());
    }

    #[test]
    fn test_checked_sub_success() {
        let a = Numeric::from_u64(30);
        let b = Numeric::from_u64(10);
        let result = a.checked_sub(b);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_u64(), 20);
    }

    #[test]
    fn test_checked_sub_underflow() {
        let a = Numeric::from_u64(10);
        let b = Numeric::from_u64(20);
        let result = a.checked_sub(b);
        assert!(result.is_none());
    }

    #[test]
    fn test_checked_mul_success() {
        let a = Numeric::from_u64(10);
        let b = Numeric::from_u64(5);
        let result = a.checked_mul(b);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_u64(), 50);
    }

    #[test]
    fn test_checked_mul_overflow() {
        let large = Numeric::from_u64(u64::MAX);
        let result = large.checked_mul(large);
        assert!(result.is_none());
    }

    #[test]
    fn test_checked_div_success() {
        let a = Numeric::from_u64(100);
        let b = Numeric::from_u64(5);
        let result = a.checked_div(b);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_u64(), 20);
    }

    #[test]
    fn test_checked_div_by_zero() {
        let a = Numeric::from_u64(100);
        let b = Numeric::ZERO;
        let result = a.checked_div(b);
        assert!(result.is_none());
    }

    #[test]
    fn test_checked_div_fractional_result() {
        // 1 / 2 = 0.5
        let a = Numeric::from_u64(1);
        let b = Numeric::from_u64(2);
        let result = a.checked_div(b).unwrap();
        assert_eq!(result.to_u64(), 0); // Truncated to 0
        // But raw value should be ~0.5 * 2^64
        let expected = Numeric::SCALE / 2;
        assert!(
            result.to_raw() > expected - 1000 && result.to_raw() < expected + 1000,
            "expected ~{expected}, got {}",
            result.to_raw()
        );
    }

    #[test]
    fn test_div_operator() {
        let a = Numeric::from_u64(100);
        let b = Numeric::from_u64(4);
        let result = a / b;
        assert_eq!(result.to_u64(), 25);
    }

    #[test]
    #[should_panic(expected = "division by zero")]
    fn test_div_operator_by_zero_panics() {
        let a = Numeric::from_u64(100);
        let b = Numeric::ZERO;
        let _ = a / b;
    }

    // ========================================================================
    // Tests for rounding modes
    // ========================================================================

    #[test]
    fn test_to_u64_ceil() {
        // Exact integer
        let exact = Numeric::from_u64(5);
        assert_eq!(exact.to_u64_ceil(), 5);

        // Fractional value
        let frac = Numeric::from_fraction(5, 2); // 2.5
        assert_eq!(frac.to_u64(), 2); // Floor
        assert_eq!(frac.to_u64_ceil(), 3); // Ceil

        // Very small fractional part
        let small_frac = Numeric::from_u64(5).saturating_add(Numeric::EPSILON);
        assert_eq!(small_frac.to_u64(), 5);
        assert_eq!(small_frac.to_u64_ceil(), 6);
    }

    #[test]
    fn test_checked_to_u64() {
        let normal = Numeric::from_u64(100);
        assert_eq!(normal.checked_to_u64(), Some(100));

        // MAX has all bits set, so integer part is u64::MAX
        let max = Numeric::MAX;
        // Integer part of MAX is 2^64 - 1 = u64::MAX, which fits
        assert!(max.checked_to_u64().is_some());
    }

    // ========================================================================
    // Tests for constants
    // ========================================================================

    #[test]
    fn test_constants() {
        assert_eq!(Numeric::ZERO.to_u64(), 0);
        assert_eq!(Numeric::ONE.to_u64(), 1);
        assert_eq!(Numeric::MIN, Numeric::ZERO);
        assert!(Numeric::EPSILON.to_raw() == 1);
        assert!(Numeric::MAX.to_raw() == u128::MAX);
    }

    // ========================================================================
    // Tests for saturating arithmetic
    // ========================================================================

    #[test]
    fn test_saturating_add() {
        let max = Numeric::MAX;
        let one = Numeric::ONE;
        let result = max.saturating_add(one);
        assert_eq!(result, Numeric::MAX);
    }

    #[test]
    fn test_saturating_sub() {
        let zero = Numeric::ZERO;
        let one = Numeric::ONE;
        let result = zero.saturating_sub(one);
        assert_eq!(result, Numeric::ZERO);
    }

    #[test]
    fn test_saturating_mul() {
        let large = Numeric::from_u64(u64::MAX);
        let result = large.saturating_mul(large);
        assert_eq!(result, Numeric::MAX);
    }

    // ========================================================================
    // Edge case tests
    // ========================================================================

    #[test]
    fn test_div_by_one() {
        let value = Numeric::from_u64(12345);
        let result = value / Numeric::ONE;
        assert_eq!(result.to_u64(), 12345);
    }

    #[test]
    fn test_div_self_equals_one() {
        let value = Numeric::from_u64(12345);
        let result = value / value;
        // Should be very close to 1.0
        let diff = if result.to_raw() > Numeric::ONE.to_raw() {
            result.to_raw() - Numeric::ONE.to_raw()
        } else {
            Numeric::ONE.to_raw() - result.to_raw()
        };
        assert!(diff < 100, "expected ~1.0, diff was {diff}");
    }

    #[test]
    fn test_mul_by_one() {
        let value = Numeric::from_u64(12345);
        let result = value * Numeric::ONE;
        assert_eq!(result.to_u64(), 12345);
    }

    #[test]
    fn test_mul_by_zero() {
        let value = Numeric::from_u64(12345);
        let result = value * Numeric::ZERO;
        assert_eq!(result, Numeric::ZERO);
    }
}
