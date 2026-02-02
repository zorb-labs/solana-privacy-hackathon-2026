//! Account closing utilities with proper security practices
//!
//! This module provides safe account closing that:
//! 1. Zeros the data buffer to prevent data leakage
//! 2. Transfers remaining lamports to a destination account
//!
//! # Example
//!
//! ```ignore
//! use panchor::accounts::close_account;
//!
//! // Close account and transfer lamports to destination
//! close_account(account_to_close, destination)?;
//! ```

use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;

/// Close an account by zeroing its data and transferring lamports.
///
/// This function:
/// 1. Zeros the entire data buffer to prevent sensitive data from being readable
///    after the account is closed (important for security)
/// 2. Transfers all lamports to the destination account
///
/// After this function completes, the account will have:
/// - Zero lamports
/// - Zeroed data buffer
/// - Original owner (the runtime will garbage collect it)
///
/// # Arguments
///
/// * `account` - The account to close (must be writable)
/// * `destination` - The account to receive the lamports (must be writable)
///
/// # Errors
///
/// Returns `ProgramError::InvalidAccountData` if either account is not writable.
///
/// # Security
///
/// The data buffer is explicitly zeroed to prevent information leakage.
/// Even though Solana will eventually reclaim the account, there's a window
/// where the data would still be readable if not explicitly zeroed.
#[inline]
pub fn close_account(account: &AccountInfo, destination: &AccountInfo) -> Result<(), ProgramError> {
    // Both accounts must be writable
    if !account.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }
    if !destination.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Zero the data buffer to prevent data leakage
    // SAFETY: We've verified the account is writable
    let mut data = account.try_borrow_mut_data()?;
    data.fill(0);
    drop(data); // Release borrow before modifying lamports

    // Transfer all lamports to destination
    let account_lamports = account.lamports();
    let dest_lamports = destination.lamports();

    // SAFETY: The Solana runtime is single-threaded, so we can safely modify lamports
    // without synchronization. Both accounts have been verified as writable above.
    // This is the standard pattern used in Solana programs for transferring lamports.
    unsafe {
        *account.borrow_mut_lamports_unchecked() = 0;
        *destination.borrow_mut_lamports_unchecked() = dest_lamports
            .checked_add(account_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: Testing requires pinocchio-test-utils which sets up proper AccountInfo structures
    // Integration tests should verify:
    // 1. Data is zeroed after close
    // 2. Lamports are transferred correctly
    // 3. Errors on non-writable accounts
}
