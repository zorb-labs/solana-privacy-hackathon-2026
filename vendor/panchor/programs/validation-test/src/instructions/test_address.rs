//! Test address constraint
//!
//! This instruction tests the #[account(address = expr)] constraint, which
//! requires the account to have a specific address.

use panchor::prelude::*;
use pinocchio::ProgramResult;

/// Accounts for testing address constraint
#[derive(Accounts)]
pub struct TestAddressAccounts<'info> {
    /// Account that must have a specific address (system program address)
    #[account(address = pinocchio_system::ID)]
    pub target: &'info AccountInfo,
}

/// Process the `test_address` instruction
///
/// This instruction validates that the target account has the expected address.
/// If not, it returns `InvalidAccountData` error.
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_address(ctx: Context<TestAddressAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
