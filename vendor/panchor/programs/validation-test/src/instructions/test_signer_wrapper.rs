//! Test Signer<'info> wrapper
//!
//! This instruction tests the Signer<'info> type which validates
//! that the account is a signer via `TryFrom`.

use panchor::prelude::*;

/// Accounts for testing Signer<'info> wrapper
#[derive(Accounts)]
pub struct TestSignerWrapperAccounts<'info> {
    /// Account that must be a signer (validated by Signer wrapper)
    pub authority: Signer<'info>,
}

/// Process the `test_signer_wrapper` instruction
///
/// This instruction validates that the authority account is a signer
/// using the Signer<'info> wrapper type.
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_signer_wrapper(ctx: Context<TestSignerWrapperAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
