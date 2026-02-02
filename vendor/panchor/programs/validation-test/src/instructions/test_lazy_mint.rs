//! Test `LazyAccount`<Mint> constraint
//!
//! This instruction tests the `LazyAccount`<'info, Mint> wrapper, which validates:
//! 1. Account owner matches `TOKEN_PROGRAM_ID`
//! 2. Account has correct size (82 bytes for Mint)

use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_token::state::Mint;

/// Accounts for testing `LazyAccount`<Mint> constraint
#[derive(Accounts)]
pub struct TestLazyMintAccounts<'info> {
    /// Mint account - validates owner is Token Program and data is 82 bytes
    pub mint: LazyAccount<'info, Mint>,
}

/// Process the `test_lazy_mint` instruction
///
/// This instruction validates that the mint account is:
/// 1. Owned by the Token Program
/// 2. Has exactly 82 bytes of data
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_lazy_mint(ctx: Context<TestLazyMintAccounts>) -> ProgramResult {
    let _ = ctx.accounts;
    Ok(())
}
