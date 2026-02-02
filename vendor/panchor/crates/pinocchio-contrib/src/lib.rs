//! # pinocchio-contrib
//!
//! Utilities and extensions for writing better Pinocchio programs.
//!
//! This crate provides a curated prelude with common re-exports from the Pinocchio
//! ecosystem, as well as useful extension traits and utilities for Solana program
//! development.
//!
//! ## Features
//!
//! - **Prelude**: Common re-exports from `pinocchio`, `pinocchio-log`, and `pinocchio-pubkey`
//! - **Account Assertions**: Extension traits for validating account properties
//! - **Error Utilities**: Helpers for error handling and logging
//! - **Constants**: Well-known Solana program IDs
//!
//! ## Usage
//!
//! ```ignore
//! use pinocchio_contrib::prelude::*;
//!
//! // Account assertions
//! signer.assert_signer()?;
//! account.assert_owner(program_id)?;
//!
//! // Logging with tracing
//! log!("Processing instruction");
//! ```

#![cfg_attr(target_os = "solana", no_std)]

mod account_assertions;
mod account_assertions_no_trace;
mod account_operations;
pub mod constants;
mod error;

pub mod prelude;

pub use account_assertions::AccountAssertions;
pub use account_assertions_no_trace::AccountAssertionsNoTrace;
pub use account_operations::AccountOperations;
pub use error::{log_account_validation_error, log_caller_location, trace};

// Re-export core dependencies for direct access
#[doc(hidden)]
pub use pinocchio;
#[doc(hidden)]
pub use pinocchio_log;
#[doc(hidden)]
pub use pinocchio_pubkey;
