//! Test signer constraint
//!
//! This instruction tests the #[account(signer)] constraint, which requires
//! the account to have signed the transaction.

use panchor::prelude::*;
use pinocchio::ProgramResult;

/// Accounts for testing signer constraint
#[derive(Accounts)]
pub struct TestSignerAccounts<'info> {
    /// Account that must be a signer
    #[account(signer)]
    pub authority: &'info AccountInfo,
}

/// Process the `test_signer` instruction
///
/// This instruction simply validates that the authority account is a signer.
/// If not, it returns `MissingRequiredSignature` error.
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_signer(ctx: Context<TestSignerAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
