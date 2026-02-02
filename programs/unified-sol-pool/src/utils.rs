//! Utility functions for unified SOL pool operations.

use crate::UnifiedSolPoolError;
use pinocchio::account_info::AccountInfo;
use pinocchio_token::state::TokenAccount;

/// Read the balance from a token account using pinocchio_token typed access.
pub fn read_token_account_balance(account: &AccountInfo) -> Result<u64, UnifiedSolPoolError> {
    let token_account = TokenAccount::from_account_info(account)
        .map_err(|_| UnifiedSolPoolError::InvalidInstructionData)?;
    Ok(token_account.amount())
}
