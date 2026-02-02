//! Account operations trait for sending SOL and closing accounts

use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use pinocchio_system::instructions::Transfer;

use crate::{AccountAssertions, constants::SYSTEM_PROGRAM_ID};

/// Extension trait for account operations like sending SOL and closing
///
/// This trait provides ergonomic methods for common account operations.
///
/// # Choosing Between `send` and `transfer`
///
/// - **`send`**: Use when transferring FROM a program-owned account. This is a direct
///   lamport manipulation (no CPI) and only works when `self` is owned by the calling program.
///
/// - **`transfer`**: Use when transferring FROM a non-program account (e.g., user wallet).
///   This invokes the system program via CPI and requires `self` to be a signer.
///
/// # Example
/// ```ignore
/// use panchor::prelude::*;
///
/// // Transfer SOL from user wallet into program account (user must sign)
/// user_wallet.transfer(round_info, deposit_amount)?;
///
/// // Send SOL from program-owned account to user
/// miner_info.send(user_wallet, withdrawal_amount)?;
///
/// // Close an account and send rent to recipient
/// old_account.close_to(recipient)?;
/// ```
pub trait AccountOperations {
    /// Send lamports from this account to another account.
    ///
    /// This performs a direct lamport transfer without invoking the system program.
    /// **Only works when `self` is owned by the calling program.**
    ///
    /// For transferring from non-program accounts (e.g., user wallets), use [`transfer`](Self::transfer) instead.
    ///
    /// # Arguments
    /// * `to` - The account to credit lamports to
    /// * `amount` - The number of lamports to send
    ///
    /// # Errors
    /// * `ProgramError::InsufficientFunds` - If this account doesn't have enough lamports
    /// * `ProgramError::InvalidArgument` - If the send would overflow `to`'s balance
    fn send(&self, to: &AccountInfo, amount: u64) -> Result<(), ProgramError>;

    /// Close this account by transferring all lamports to another account.
    ///
    /// This method:
    /// 1. Transfers all lamports from this account to `to`
    /// 2. Assigns the system program as the owner
    /// 3. Reallocates data to zero bytes
    /// 4. Marks the account as closed
    ///
    /// After calling this method, the account will be closed and its
    /// rent-exempt balance returned to the `to` account.
    fn close_to(&self, to: &AccountInfo) -> Result<(), ProgramError>;

    /// Transfer lamports from this non-program account to another account.
    ///
    /// This invokes the system program's transfer instruction via CPI.
    /// **Use this when transferring FROM user wallets or other non-program accounts.**
    /// Requires `self` to be a signer.
    ///
    /// For transferring from program-owned accounts, use [`send`](Self::send) instead
    /// (more efficient, no CPI overhead).
    ///
    /// # Arguments
    /// * `to` - The account to credit lamports to
    /// * `amount` - The number of lamports to transfer
    /// * `system_program` - The system program account (validated internally)
    ///
    /// # Errors
    /// * Returns error if the CPI fails (e.g., `self` is not a signer or has insufficient funds)
    fn transfer(
        &self,
        to: &AccountInfo,
        amount: u64,
        system_program: &AccountInfo,
    ) -> Result<(), ProgramError>;
}

impl AccountOperations for AccountInfo {
    fn send(&self, to: &AccountInfo, amount: u64) -> Result<(), ProgramError> {
        let mut from_lamports = self.try_borrow_mut_lamports()?;
        let mut to_lamports = to.try_borrow_mut_lamports()?;

        *from_lamports = from_lamports
            .checked_sub(amount)
            .ok_or(ProgramError::InsufficientFunds)?;
        *to_lamports = to_lamports
            .checked_add(amount)
            .ok_or(ProgramError::InvalidArgument)?;

        Ok(())
    }

    fn close_to(&self, to: &AccountInfo) -> Result<(), ProgramError> {
        // Transfer all lamports to the destination account
        let lamports = self.lamports();
        *to.try_borrow_mut_lamports()? += lamports;
        *self.try_borrow_mut_lamports()? = 0;

        // Mark account as closed (runtime will zero the data)
        self.close()?;

        Ok(())
    }

    fn transfer(
        &self,
        to: &AccountInfo,
        amount: u64,
        system_program: &AccountInfo,
    ) -> Result<(), ProgramError> {
        system_program.assert_program(&SYSTEM_PROGRAM_ID)?;
        Transfer {
            from: self,
            to,
            lamports: amount,
        }
        .invoke()
    }
}
