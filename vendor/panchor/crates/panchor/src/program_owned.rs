//! `ProgramOwned` trait for accounts that belong to a specific program
//!
//! This trait provides a standardized way to declare which program owns
//! a particular account type, enabling compile-time association between
//! account structs and their owning programs.

use pinocchio::pubkey::Pubkey;

/// Trait for account types that are owned by a specific program.
///
/// Implementing this trait allows the `as_program_account` and
/// `as_program_account_mut` methods to automatically verify that
/// an account is owned by the expected program before loading its data.
///
/// # Example
///
/// ```ignore
/// use panchor::prelude::*;
///
/// impl ProgramOwned for Mine {
///     const PROGRAM_ID: Pubkey = MINES_PROGRAM_ID;
/// }
///
/// // Now you can use as_program_account which checks ownership:
/// let mine = mine_info.as_program_account::<Mine>()?;
/// ```
pub trait ProgramOwned {
    /// The program ID that owns accounts of this type.
    const PROGRAM_ID: Pubkey;
}
