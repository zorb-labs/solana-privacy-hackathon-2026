//! Test mutable constraint
//!
//! This instruction tests the #[account(mut)] constraint, which requires
//! the account to be marked as writable in the transaction.

use panchor::prelude::*;
use pinocchio::ProgramResult;

/// Accounts for testing mutable constraint
#[derive(Accounts)]
pub struct TestMutableAccounts<'info> {
    /// Account that must be writable
    #[account(mut)]
    pub target: &'info AccountInfo,
}

/// Process the `test_mutable` instruction
///
/// This instruction validates that the target account is writable.
/// If not, it returns `InvalidAccountData` error.
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_mutable(ctx: Context<TestMutableAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
