//! Context wrapper for accounts and bump seeds
//!
//! The `Context` type wraps an accounts struct along with the PDA bump seeds
//! that were derived during account validation.

use core::ops::Deref;

use crate::accounts::Bumps;
use pinocchio::account_info::AccountInfo;

/// Result of parsing accounts via `try_into_context`.
///
/// For accounts with `init_idempotent` constraints, this enum allows
/// signaling that the instruction should return early (account already exists).
pub enum ParseResult<'info, T: Bumps> {
    /// Successfully parsed accounts - continue with instruction processing
    Parsed(Parsed<'info, T>),
    /// An `init_idempotent` account already exists - skip instruction processing
    SkipIdempotent,
}

impl<'info, T: Bumps> ParseResult<'info, T> {
    /// Returns the parsed accounts if present, or None if idempotent skip.
    #[inline]
    pub fn into_option(self) -> Option<Parsed<'info, T>> {
        match self {
            ParseResult::Parsed(p) => Some(p),
            ParseResult::SkipIdempotent => None,
        }
    }
}

/// Context wrapper that holds a reference to accounts and their PDA bump seeds.
///
/// This type is used by instruction handlers to access both the validated
/// accounts and the bump seeds that were derived during validation.
///
/// # Usage
///
/// ```ignore
/// use panchor::prelude::*;
///
/// pub fn process_my_instruction(ctx: Context<MyAccounts>) -> ProgramResult {
///     // Access accounts via Deref
///     let mine = ctx.mine.load()?;
///
///     // Access bump seeds
///     let stake_bump = ctx.bumps.stake;
///
///     // Access remaining accounts
///     for account in ctx.remaining_accounts {
///         // process extra accounts
///     }
///
///     Ok(())
/// }
/// ```
pub struct Context<'a, 'info, T: Bumps> {
    /// The validated accounts (reference)
    pub accounts: &'a T,
    /// PDA bump seeds derived during validation
    pub bumps: <T as Bumps>::Bumps,
    /// Remaining accounts not parsed into the struct
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'a, 'info, T: Bumps> Context<'a, 'info, T> {
    /// Create a new Context with accounts reference, bumps, and remaining accounts
    #[inline]
    pub fn new(
        accounts: &'a T,
        bumps: <T as Bumps>::Bumps,
        remaining_accounts: &'info [AccountInfo],
    ) -> Self {
        Self {
            accounts,
            bumps,
            remaining_accounts,
        }
    }
}

impl<'a, 'info, T: Bumps> Deref for Context<'a, 'info, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.accounts
    }
}

/// Parsed accounts result that owns the accounts and bumps.
///
/// This struct is returned by `try_into_context` and owns the accounts.
/// Use `.into_context()` or access fields directly.
pub struct Parsed<'info, T: Bumps> {
    /// The validated accounts
    pub accounts: T,
    /// PDA bump seeds derived during validation
    pub bumps: <T as Bumps>::Bumps,
    /// Remaining accounts not parsed into the struct
    pub remaining_accounts: &'info [AccountInfo],
}

impl<'info, T: Bumps> Parsed<'info, T> {
    /// Create a new Parsed with accounts, bumps, and remaining accounts
    #[inline]
    pub fn new(
        accounts: T,
        bumps: <T as Bumps>::Bumps,
        remaining_accounts: &'info [AccountInfo],
    ) -> Self {
        Self {
            accounts,
            bumps,
            remaining_accounts,
        }
    }

    /// Get a Context reference to this parsed accounts
    #[inline]
    pub fn as_context(&self) -> Context<'_, 'info, T>
    where
        <T as Bumps>::Bumps: Copy,
    {
        Context::new(&self.accounts, self.bumps, self.remaining_accounts)
    }
}

impl<'info, T: Bumps> Deref for Parsed<'info, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.accounts
    }
}

impl<'info, T: Bumps> core::ops::DerefMut for Parsed<'info, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.accounts
    }
}
