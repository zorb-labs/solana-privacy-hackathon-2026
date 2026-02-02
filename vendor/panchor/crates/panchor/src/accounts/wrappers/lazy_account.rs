//! Lazy account wrapper for validated account access
//!
//! [`LazyAccount<'info, T>`] provides type-safe access to account data with
//! lazy loading. Validation happens at construction time, but deserialization
//! is deferred until `load()` is called.
//!
//! # Example
//!
//! ```ignore
//! #[derive(Accounts)]
//! pub struct MyAccounts<'info> {
//!     pub mine: LazyAccount<'info, Mine>,
//!     pub token_account: LazyAccount<'info, TokenAccount>,
//! }
//!
//! // Access data when needed
//! let mine = accounts.mine.load()?;
//! let balance = accounts.token_account.load()?.amount();
//! ```

use core::marker::PhantomData;

use pinocchio::account_info::{AccountInfo, Ref};
use pinocchio::program_error::ProgramError;

use super::super::{AccountDataValidate, AccountDeserialize, AsAccountInfo};
use crate::{AccountAssertionsNoTrace, ProgramOwned};

/// A lazy account wrapper that validates at construction but deserializes on demand.
///
/// `LazyAccount<'info, T>` wraps an `AccountInfo` and ensures at construction time that:
/// 1. The account's owner matches `T::PROGRAM_ID` (from the `ProgramOwned` trait)
/// 2. The account data passes validation (via `AccountDataValidate`)
///
/// Deserialization is deferred until `load()` is called, making construction cheap.
///
/// # Type Parameters
///
/// - `'info` - The lifetime of the account info slice
/// - `T` - The account data type, must implement `ProgramOwned + AccountDataValidate`
///
/// # Example
///
/// ```ignore
/// #[derive(Accounts)]
/// pub struct MyAccounts<'info> {
///     pub mine: LazyAccount<'info, Mine>,
///     pub token_account: LazyAccount<'info, TokenAccount>,
/// }
///
/// // Validation happens at construction, deserialization is lazy
/// accounts.mine.inspect(|mine| {
///     println!("Creator: {:?}", mine.creator);
/// })?;
/// ```
#[repr(transparent)]
pub struct LazyAccount<'info, T: ProgramOwned + AccountDataValidate> {
    info: &'info AccountInfo,
    _marker: PhantomData<T>,
}

impl<'info, T: ProgramOwned + AccountDataValidate> LazyAccount<'info, T> {
    /// Create a new `LazyAccount` wrapper after validating owner and data.
    ///
    /// # Errors
    ///
    /// - `ProgramError::IncorrectProgramId` if owner doesn't match
    /// - `ProgramError::InvalidAccountData` if validation fails
    #[inline]
    pub fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        // Check owner
        info.assert_owner_no_trace(&T::PROGRAM_ID)?;

        // Validate data structure (doesn't deserialize)
        T::validate(info)?;

        Ok(Self {
            info,
            _marker: PhantomData,
        })
    }

    /// Create a new `LazyAccount` wrapper without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure:
    /// 1. The account's owner matches `T::PROGRAM_ID`
    /// 2. The account data is valid for type `T`
    ///
    /// Using this with an invalid account can lead to type confusion attacks.
    #[inline]
    pub const unsafe fn new_unchecked(info: &'info AccountInfo) -> Self {
        Self {
            info,
            _marker: PhantomData,
        }
    }

    /// Returns the underlying `AccountInfo`.
    #[inline]
    pub const fn info(&self) -> &'info AccountInfo {
        self.info
    }

    /// Returns the account's public key.
    #[inline]
    pub fn key(&self) -> &pinocchio::pubkey::Pubkey {
        self.info.key()
    }
}

/// Methods that require `AccountDeserialize` for loading data
impl<'info, T: ProgramOwned + AccountDataValidate + AccountDeserialize> LazyAccount<'info, T> {
    /// Load immutable access to the account data.
    ///
    /// Returns a `Ref` that automatically manages the borrow lifetime.
    ///
    /// # Warning
    ///
    /// Using this method directly is **not recommended** because borrow drops
    /// are not automatically managed. If the `Ref` is not dropped before
    /// another borrow attempt, you will get a runtime panic.
    ///
    /// Prefer using [`inspect`](Self::inspect) or [`map`](Self::map) instead,
    /// which automatically manage borrow lifetimes.
    #[track_caller]
    #[inline]
    pub fn load(&self) -> Result<Ref<'info, T>, ProgramError> {
        T::deserialize(self.info)
    }

    /// Inspect account data immutably with automatic borrow management.
    ///
    /// Loads the account immutably, calls the closure with `&T`,
    /// and automatically drops the borrow when the closure returns.
    ///
    /// # Example
    /// ```ignore
    /// account.inspect(|data| {
    ///     println!("Value: {}", data.value);
    /// })?;
    /// // Borrow is dropped here, safe to perform other operations
    /// ```
    #[track_caller]
    pub fn inspect<F>(&self, f: F) -> Result<(), ProgramError>
    where
        F: FnOnce(&T),
    {
        let data = self.load()?;
        f(&data);
        Ok(())
    }

    /// Map account data to a value with automatic borrow management.
    ///
    /// Loads the account immutably, calls the closure with `&T`,
    /// and returns the result after dropping the borrow.
    ///
    /// # Example
    /// ```ignore
    /// let balance = account.map(|data| data.balance)?;
    /// // Borrow is dropped, value is returned
    /// ```
    #[track_caller]
    pub fn map<F, R>(&self, f: F) -> Result<R, ProgramError>
    where
        F: FnOnce(&T) -> R,
    {
        let data = self.load()?;
        Ok(f(&data))
    }
}

impl<'info, T: ProgramOwned + AccountDataValidate> AsAccountInfo<'info> for LazyAccount<'info, T> {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info, T: ProgramOwned + AccountDataValidate> AsAccountInfo<'info> for &LazyAccount<'info, T> {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info, T: ProgramOwned + AccountDataValidate> core::ops::Deref for LazyAccount<'info, T> {
    type Target = AccountInfo;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'info, T: ProgramOwned + AccountDataValidate> TryFrom<&'info AccountInfo>
    for LazyAccount<'info, T>
{
    type Error = ProgramError;

    #[inline]
    fn try_from(info: &'info AccountInfo) -> Result<Self, Self::Error> {
        Self::new(info)
    }
}

impl<'info, T: ProgramOwned + AccountDataValidate> From<LazyAccount<'info, T>>
    for &'info AccountInfo
{
    #[inline]
    fn from(lazy: LazyAccount<'info, T>) -> Self {
        lazy.info
    }
}

impl<'info, T: ProgramOwned + AccountDataValidate> From<&LazyAccount<'info, T>>
    for &'info AccountInfo
{
    #[inline]
    fn from(lazy: &LazyAccount<'info, T>) -> Self {
        lazy.info
    }
}
