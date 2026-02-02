//! Fee calculation helpers for execute_transact.
//!
//! This module provides fee calculation utilities used during transaction
//! execution. These are security-critical for ensuring correct fee deduction.
//!
//! # Security Considerations
//! - All arithmetic uses checked operations to prevent overflow
//! - Fee rate is in basis points (1/10000), max 10000 (100%)
//! - Results are validated to fit in u64

use crate::errors::ShieldedPoolError;
use pinocchio::program_error::ProgramError;

// ============================================================================
// Fee Calculation Helpers
// ============================================================================

/// Calculate fee amount: (amount * rate) / 10_000
///
/// # Security
/// - Uses u128 intermediate to prevent overflow on multiplication
/// - Checked division prevents panic
/// - Result validated to fit in u64
///
/// # Arguments
/// * `amount` - The base amount to calculate fee on
/// * `rate` - Fee rate in basis points (e.g., 100 = 1%)
///
/// # Returns
/// The fee amount, or ArithmeticOverflow if calculation fails.
#[inline]
pub fn calculate_fee(amount: u64, rate: u16) -> Result<u64, ProgramError> {
    use crate::utils::BASIS_POINTS_DENOMINATOR;
    (amount as u128)
        .checked_mul(rate as u128)
        .and_then(|v| v.checked_div(BASIS_POINTS_DENOMINATOR))
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| ShieldedPoolError::ArithmeticOverflow.into())
}
