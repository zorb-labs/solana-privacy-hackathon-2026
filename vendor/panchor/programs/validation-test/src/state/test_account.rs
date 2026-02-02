//! Test account for validation testing

use panchor::prelude::*;

use super::ValidationAccount;

/// Test account for validating account constraints
#[account(ValidationAccount::TestAccount)]
#[repr(C)]
pub struct TestAccount {
    /// Authority that can modify this account
    pub authority: Pubkey,
    /// Some value to store
    pub value: u64,
    /// Bump seed for PDA derivation
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 7],
}
