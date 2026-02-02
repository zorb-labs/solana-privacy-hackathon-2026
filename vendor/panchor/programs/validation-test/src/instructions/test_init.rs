//! Test init constraint
//!
//! This instruction tests the #[account(init, seeds = [...], payer = ...)] constraint,
//! which creates a new PDA account with the specified seeds.

use panchor::prelude::*;
use pinocchio::ProgramResult;

use crate::state::TestAccount;

/// Accounts for testing init constraint
#[derive(Accounts)]
pub struct TestInitAccounts<'info> {
    /// Payer for account creation
    #[account(mut)]
    pub payer: Signer<'info>,
    /// New account to create
    #[account(init, seeds = [b"test", payer.key().as_ref()], payer = payer)]
    pub test_account: AccountLoader<'info, TestAccount>,
    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Process the `test_init` instruction
///
/// This instruction creates a new `TestAccount` PDA derived from the payer's key.
/// It validates that:
/// 1. The payer is a signer
/// 2. The `test_account` doesn't already exist (is empty)
/// 3. The `system_program` is the correct program
#[allow(clippy::needless_pass_by_value)]
pub fn process_test_init(ctx: Context<TestInitAccounts>) -> ProgramResult {
    let TestInitAccounts {
        payer,
        test_account,
        system_program: _,
    } = ctx.accounts;

    // Initialize the account data
    test_account.try_map_mut(|data| {
        data.authority = *payer.key();
        data.value = 42;
        Ok(())
    })?;

    Ok(())
}
