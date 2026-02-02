//! Test owner constraint - #[account(owner = expr)]
//!
//! Tests that the account's owner matches the expected program ID.

use panchor::prelude::*;

/// Accounts for testing #[account(owner = expr)] constraint
#[derive(Accounts)]
pub struct TestOwnerConstraintAccounts<'info> {
    /// Account that should be owned by system program
    #[account(owner = &System::ID)]
    pub target: &'info AccountInfo,
}

/// Handler for `test_owner_constraint` instruction
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_owner_constraint(ctx: Context<TestOwnerConstraintAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
