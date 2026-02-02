//! Account wrapper types for typed account access
//!
//! This module provides wrapper types that combine an `AccountInfo` with a phantom
//! type parameter for compile-time type safety and automatic validation.

mod account_loader;
mod lazy_account;
mod program;
mod signer;

pub use account_loader::AccountLoader;
pub use lazy_account::LazyAccount;
pub use program::Program;
pub use signer::Signer;
