//! Prelude with common re-exports for Pinocchio programs.
//!
//! This module provides a convenient single import for commonly used types
//! and traits from the Pinocchio ecosystem.
//!
//! ```ignore
//! use pinocchio_contrib::prelude::*;
//! ```

// Re-export pinocchio itself for convenience
pub use pinocchio;

// Core types from pinocchio
pub use pinocchio::ProgramResult;
pub use pinocchio::account_info::AccountInfo;
pub use pinocchio::program_error::ProgramError;
pub use pinocchio::pubkey::{Pubkey, pubkey_eq};

// Macros from pinocchio-pubkey
pub use pinocchio_pubkey::{declare_id, pubkey};

// Logging macro
pub use pinocchio_log::log;

// Account assertions from this crate
pub use crate::error::{log_account_validation_error, log_caller_location, trace};
pub use crate::{AccountAssertions, AccountAssertionsNoTrace};
