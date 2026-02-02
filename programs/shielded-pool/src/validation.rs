//! Validation helper functions for instruction processing.
//!
//! Provides consistent, reusable validation functions to reduce boilerplate
//! and ensure uniform error handling across all instructions.
//!
//! Note: For basic account assertions (signer, owner, key matching), use
//! `pinocchio_contrib::AccountAssertions` trait instead. This module provides
//! specialized validators for token-specific logic and initialization checks.

use crate::token::{SPL_TOKEN_2022_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};
use pinocchio_token::state::TokenAccount;

/// Require account to be a valid token program (SPL Token or Token-2022).
///
/// Returns `IncorrectProgramId` if the account is not a token program.
#[inline]
pub fn require_token_program(account: &AccountInfo) -> Result<(), ProgramError> {
    let key = account.key();
    if *key != SPL_TOKEN_PROGRAM_ID && *key != SPL_TOKEN_2022_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Require account to be owned by a token program (SPL Token or Token-2022).
///
/// Use this to validate that a token account is a valid SPL token account.
/// Returns `IncorrectProgramId` if the account is not owned by a token program.
#[inline]
pub fn require_token_program_owner(account: &AccountInfo) -> Result<(), ProgramError> {
    let owner = account.owner();
    if *owner != SPL_TOKEN_PROGRAM_ID && *owner != SPL_TOKEN_2022_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Require account to be uninitialized (PDA does not exist).
///
/// An account is considered uninitialized/non-existent if ALL of:
/// 1. Owner is the system program (no program has claimed it)
/// 2. Lamports is 0 (account has no balance)
/// 3. Data length is 0 (no data has been allocated)
///
/// This matches Anchor's behavior for `init` constraints.
/// Reference: https://github.com/coral-xyz/anchor/blob/master/lang/syn/src/codegen/accounts/constraints.rs
///
/// # Security Note
/// This is critical for preventing:
/// - Re-initialization attacks
/// - Account confusion attacks where an attacker pre-creates an account
///
/// Returns `AccountAlreadyInitialized` if the account already exists.
#[inline]
pub fn require_uninitialized(account: &AccountInfo) -> Result<(), ProgramError> {
    // Check owner is system program (not yet owned by any program)
    if *account.owner() != pinocchio_system::ID {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // Check lamports is 0 (account doesn't exist)
    // An account with 0 lamports is garbage collected by the runtime
    if account.lamports() != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    // Check data length is 0 (no data allocated)
    let data = account.try_borrow_data()?;
    if !data.is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    Ok(())
}

/// Require account to be either uninitialized OR owned by our program but empty.
///
/// This is a more lenient check that allows:
/// 1. Completely uninitialized accounts (owner = system program, lamports = 0, data empty)
/// 2. Accounts owned by our program but with zero data (failed previous init)
///
/// Use `require_uninitialized` for stricter checks.
#[inline]
pub fn require_uninitialized_or_owned_empty(
    account: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(), ProgramError> {
    let owner = account.owner();

    // Case 1: Completely uninitialized
    if *owner == pinocchio_system::ID && account.lamports() == 0 {
        let data = account.try_borrow_data()?;
        if data.is_empty() {
            return Ok(());
        }
    }

    // Case 2: Owned by our program but data is all zeros (failed init)
    if owner == program_id {
        let data = account.try_borrow_data()?;
        if data.iter().all(|&b| b == 0) {
            return Ok(());
        }
    }

    Err(ProgramError::AccountAlreadyInitialized)
}

/// Require token account to have the expected SPL token owner.
///
/// Uses pinocchio_token typed access for safe field reading.
/// Returns `IllegalOwner` if the owner doesn't match.
#[inline]
pub fn require_token_account_owner(
    token_account: &AccountInfo,
    expected_owner: &Pubkey,
) -> Result<(), ProgramError> {
    let account = TokenAccount::from_account_info(token_account)?;
    if account.owner() != expected_owner {
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

/// Require token account to have the expected mint.
///
/// Uses pinocchio_token typed access for safe field reading.
/// Returns `InvalidAccountData` if the mint doesn't match.
#[inline]
pub fn require_token_account_mint(
    token_account: &AccountInfo,
    expected_mint: &Pubkey,
) -> Result<(), ProgramError> {
    let account = TokenAccount::from_account_info(token_account)?;
    if account.mint() != expected_mint {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}

/// Require mint account to be owned by the token program.
///
/// Returns `IncorrectProgramId` if the mint is not owned by a token program.
#[inline]
pub fn require_valid_mint(
    mint: &AccountInfo,
    token_program: &AccountInfo,
) -> Result<(), ProgramError> {
    if mint.owner() != token_program.key() {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

/// Require account to be a valid SPL token account.
///
/// Uses pinocchio_token typed access which validates:
/// 1. Account is owned by SPL Token or Token-2022 program
/// 2. Account data is the correct size for a token account (165 bytes)
///
/// Returns `IncorrectProgramId` if not owned by token program,
/// `InvalidAccountData` if data is invalid.
#[inline]
pub fn require_valid_token_account(account: &AccountInfo) -> Result<(), ProgramError> {
    // TokenAccount::from_account_info validates owner and data length
    let _ = TokenAccount::from_account_info(account)?;
    Ok(())
}
