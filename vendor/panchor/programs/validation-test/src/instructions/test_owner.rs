//! Test owner constraint
//!
//! This instruction tests the `AccountLoader`<T> wrapper, which validates:
//! 1. Account owner matches `T::PROGRAM_ID`
//! 2. Account has correct discriminator
//! 3. Account has valid size

use panchor::prelude::*;
use pinocchio::ProgramResult;

use crate::state::TestAccount;

/// Accounts for testing owner constraint
#[derive(Accounts)]
pub struct TestOwnerAccounts<'info> {
    /// Account that must be owned by our program with correct discriminator
    pub test_account: AccountLoader<'info, TestAccount>,
}

/// Process the `test_owner` instruction
///
/// This instruction validates that the `test_account` is:
/// 1. Owned by the validation test program
/// 2. Has the correct discriminator for `TestAccount`
/// 3. Has valid size for `TestAccount`
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_owner(ctx: Context<TestOwnerAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
