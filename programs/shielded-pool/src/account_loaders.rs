//! Account loading utilities.
//!
//! # Account Data Layout
//!
//! All accounts follow the layout: `[8-byte discriminator][struct data]`
//!
//! The discriminator is stored in the first 8 bytes but is NOT part of the
//! struct definition. The struct starts at offset 8 in the account data.
//!
//! # Note
//!
//! For most accounts, use panchor's `AccountLoader<T>` for type-safe loading.
//! For variable-length accounts (like `TransactSession`), use the helper
//! methods defined directly on the struct.

/// Discriminator size in bytes (always 8 for u64).
pub const DISCRIMINATOR_SIZE: usize = 8;
