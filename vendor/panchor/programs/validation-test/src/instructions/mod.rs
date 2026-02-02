//! Instruction handlers for validation testing
//!
//! Each instruction tests specific account constraint types:
//! - `test_signer`: #[account(signer)] constraint on raw `AccountInfo`
//! - `test_signer_wrapper`: Signer<'info> wrapper type
//! - `test_mutable`: mut constraint
//! - `test_owner`: account = Type constraint (owner check via `AccountLoader`)
//! - `test_owner_constraint`: owner = expr constraint (explicit owner check)
//! - `test_program`: Program<'info, T> wrapper type
//! - `test_address`: address = expr constraint
//! - `test_init`: init constraint with seeds and payer

use panchor::prelude::*;

mod test_address;
mod test_init;
mod test_lazy_mint;
mod test_mutable;
mod test_owner;
mod test_owner_constraint;
mod test_program;
mod test_signer;
mod test_signer_wrapper;

pub use test_address::*;
pub use test_init::*;
pub use test_lazy_mint::*;
pub use test_mutable::*;
pub use test_owner::*;
pub use test_owner_constraint::*;
pub use test_program::*;
pub use test_signer::*;
pub use test_signer_wrapper::*;

/// Instruction discriminators for the validation test program
#[instructions]
pub enum ValidationInstruction {
    /// Test signer constraint - #[account(signer)] on raw `AccountInfo`
    #[handler]
    TestSigner = 0,
    /// Test mutable constraint - account must be marked writable
    #[handler]
    TestMutable = 1,
    /// Test owner constraint - `AccountLoader` validates owner, discriminator, size
    #[handler]
    TestOwner = 2,
    /// Test program constraint - Program<'info, T> wrapper validates executable + ID
    #[handler]
    TestProgram = 3,
    /// Test address constraint - account must have exact address
    #[handler]
    TestAddress = 4,
    /// Test init constraint - creates PDA with seeds
    #[handler]
    TestInit = 5,
    /// Test owner = expr constraint - explicit owner validation
    #[handler]
    TestOwnerConstraint = 6,
    /// Test Signer<'info> wrapper - validates `is_signer` via `TryFrom`
    #[handler]
    TestSignerWrapper = 7,
    /// Test `LazyAccount`<'info, Mint> - validates Token Program owner and 82-byte size
    #[handler]
    TestLazyMint = 8,
}
