//! State accounts for validation testing

mod test_account;

pub use test_account::*;

use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Account discriminator for validation test accounts
#[derive(Clone, Copy, IntoPrimitive, TryFromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub enum ValidationAccount {
    /// Test account for validation
    TestAccount = 0,
}
