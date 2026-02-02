//! Validation Test Program
//!
//! A test program for exercising panchor account validation constraints.
//! Each instruction tests a specific constraint type:
//!
//! - `test_signer`: Tests #[account(signer)] - requires account to be a signer
//! - `test_mutable`: Tests #[account(mut)] - requires account to be writable
//! - `test_owner`: Tests `AccountLoader`<T> - validates owner, discriminator, size
//! - `test_program`: Tests Program<T> - validates executable and address
//! - `test_address`: Tests #[account(address = expr)] - validates exact address
//! - `test_init`: Tests #[account(init, seeds = [...], payer = ...)] - creates PDA

#![cfg_attr(not(any(test, feature = "idl-build")), no_std)]

extern crate alloc;

pub mod error;
pub mod instructions;
pub mod state;

pub use error::*;
pub use instructions::ValidationInstruction;

panchor::program! {
    id = "E6VXXXxTUkibL82Ed41yCkYbXCNPgyiMLspnvwA67aBg",
    instructions = ValidationInstruction,
    accounts = state::ValidationAccount,
}
