//! Test program constraint
//!
//! This instruction tests the #[account(program = expr)] constraint, which
//! requires the account to be an executable program with a specific address.

use panchor::prelude::*;
use pinocchio::ProgramResult;

/// Accounts for testing program constraint
#[derive(Accounts)]
pub struct TestProgramAccounts<'info> {
    /// System program that must be executable with correct address
    pub system_program: Program<'info, System>,
}

/// Process the `test_program` instruction
///
/// This instruction validates that the `system_program` account:
/// 1. Is executable
/// 2. Has the correct program ID (system program)
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_program(ctx: Context<TestProgramAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
