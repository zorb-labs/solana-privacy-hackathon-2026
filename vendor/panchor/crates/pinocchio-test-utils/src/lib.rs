//! # pinocchio-test-utils
//!
//! Testing utilities for Pinocchio programs.
//!
//! This crate provides utilities for creating mock `AccountInfo` objects
//! for use in unit tests.
//!
//! ## Usage
//!
//! ```rust
//! use pinocchio_test_utils::AccountInfoBuilder;
//! use pinocchio::pubkey::Pubkey;
//!
//! let key = Pubkey::default();
//! let owner = Pubkey::default();
//! let account = AccountInfoBuilder::new()
//!     .key(&key)
//!     .owner(&owner)
//!     .signer(true)
//!     .writable(true)
//!     .lamports(1_000_000)
//!     .data(&[1, 2, 3, 4])
//!     .build();
//!
//! assert!(account.info().is_signer());
//! assert!(account.info().is_writable());
//! ```

mod account_builder;

pub use account_builder::{AccountInfoBuilder, TestAccount};
