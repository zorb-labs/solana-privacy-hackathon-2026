//! Account wrapper types for typed account access
//!
//! This module provides wrapper types that combine an `AccountInfo` with a phantom
//! type parameter for compile-time type safety and automatic validation.
//!
//! # Types
//!
//! - [`LazyAccount<'info, T>`] - Lazy account wrapper that validates at construction, deserializes on demand
//! - [`AccountLoader<'info, T>`] - Zero-copy account loader that validates owner, discriminator, and size
//! - [`Signer<'info>`] - Wraps an `AccountInfo` and validates it's a signer
//! - [`Program<'info, T>`] - Wraps an `AccountInfo` and validates it's an executable program
//! - [`Context<'info, T>`] - Wrapper that holds accounts and their PDA bump seeds
//!
//! # Usage
//!
//! ```ignore
//! use panchor::prelude::*;
//!
//! #[derive(Accounts)]
//! pub struct MyAccounts<'info> {
//!     pub mine: AccountLoader<'info, Mine>,        // Zero-copy mutable access to Mine account
//!     pub token: LazyAccount<'info, TokenAccount>, // Lazy loading access to token account
//!     pub payer: Signer<'info>,                    // Validates signer
//!     pub system_program: Program<'info, System>,  // Validates program ID and executable
//! }
//! ```

// Traits
mod account_data_validate;
mod account_deserialize;
mod as_account_info;
mod bumps;
mod id;
mod pda_account;
mod set_bump;

// Utilities
mod close;

// Wrapper types
mod wrappers;

// Re-export traits
pub use account_data_validate::AccountDataValidate;
pub use account_deserialize::AccountDeserialize;
pub use as_account_info::AsAccountInfo;
pub use bumps::Bumps;
pub use id::Id;
pub use pda_account::{PdaAccount, PdaAccountWithBump};
pub use set_bump::SetBump;

// Re-export utilities
pub use close::close_account;

// Re-export wrapper types
pub use wrappers::{AccountLoader, LazyAccount, Program, Signer};
