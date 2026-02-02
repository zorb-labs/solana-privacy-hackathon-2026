//! Zero-copy account loader for program-owned accounts
//!
//! [`AccountLoader<'info, T>`] is designed for **zero-copy** account access.
//! Zero-copy means the account data is read/written directly from the underlying
//! buffer without deserialization, which is more efficient for large accounts.
//!
//! # When to use `AccountLoader`
//!
//! Use `AccountLoader` for accounts that:
//! - Use zero-copy serialization (implement `bytemuck::Pod`)
//! - Have discriminators for type safety
//! - Are owned by your program
//!
//! # Example
//!
//! ```ignore
//! #[derive(Accounts)]
//! pub struct MyAccounts<'info> {
//!     #[account(mut)]
//!     pub mine: AccountLoader<'info, Mine>,
//! }
//!
//! // Access data with automatic borrow management
//! accounts.mine.inspect(|mine| {
//!     println!("Balance: {}", mine.balance);
//! })?;
//!
//! // Or get a direct reference
//! let mine = accounts.mine.load()?;
//! ```

use core::marker::PhantomData;

use bytemuck::Pod;
use pinocchio::account_info::{AccountInfo, Ref, RefMut};
use pinocchio::program_error::ProgramError;

use pinocchio_contrib::AccountAssertionsNoTrace;

use super::super::AsAccountInfo;
use crate::discriminator::DISCRIMINATOR_LEN;
use crate::space::InitSpace;
use crate::{Discriminator, InnerSize, ProgramOwned};

/// Map Ref<[u8]> to Ref<T>, skipping the discriminator.
/// This is a lightweight helper that doesn't re-validate - validation happened at construction.
///
/// # Panics
///
/// Panics if data is too small (should never happen after AccountLoader::new validation).
fn map_ref<T: Pod + InnerSize>(data: Ref<'_, [u8]>) -> Ref<'_, T> {
    // Use saturating_add to prevent overflow - if it saturates, the slice access
    // will panic with a clear out-of-bounds message rather than undefined behavior
    let end = DISCRIMINATOR_LEN.saturating_add(T::INNER_SIZE);
    Ref::map(data, |bytes: &[u8]| {
        bytemuck::from_bytes(&bytes[DISCRIMINATOR_LEN..end])
    })
}

/// Map `RefMut`<[u8]> to `RefMut`<T>, skipping the discriminator.
/// This is a lightweight helper that doesn't re-validate - validation happened at construction.
///
/// # Panics
///
/// Panics if data is too small (should never happen after AccountLoader::new validation).
fn map_ref_mut<T: Pod + InnerSize>(data: RefMut<'_, [u8]>) -> RefMut<'_, T> {
    // Use saturating_add to prevent overflow - if it saturates, the slice access
    // will panic with a clear out-of-bounds message rather than undefined behavior
    let end = DISCRIMINATOR_LEN.saturating_add(T::INNER_SIZE);
    RefMut::map(data, |bytes: &mut [u8]| {
        bytemuck::from_bytes_mut(&mut bytes[DISCRIMINATOR_LEN..end])
    })
}

/// A zero-copy account loader that validates the account owner, discriminator, and size.
///
/// `AccountLoader<'info, T>` wraps an `AccountInfo` and ensures at construction time that:
/// 1. The account's owner matches `T::PROGRAM_ID` (from the `ProgramOwned` trait)
/// 2. The account's discriminator matches `T::DISCRIMINATOR` (from the `Discriminator` trait)
/// 3. The account's data length is at least `T::INIT_SPACE` (from the `InitSpace` trait)
///
/// This type is designed for **zero-copy** accounts where the data is accessed directly
/// from the underlying buffer without deserialization. This is more efficient for large
/// accounts but requires the account type to implement `bytemuck::Pod`.
///
/// It provides methods for accessing account data with automatic borrow management,
/// ensuring borrows are dropped after the operation completes.
///
/// # Type Parameters
///
/// - `'info` - The lifetime of the account info slice
/// - `T` - The account data type, must implement `ProgramOwned`, `Discriminator`, and `InnerSize`
///
/// # Example
///
/// ```ignore
/// #[derive(Accounts)]
/// pub struct MyAccounts<'info> {
///     #[account(mut)]
///     pub mine: AccountLoader<'info, Mine>,
/// }
///
/// // Access data with automatic borrow management
/// accounts.mine.inspect(|mine| {
///     println!("Balance: {}", mine.balance);
/// })?;
///
/// // Or get a direct reference
/// let mine = accounts.mine.load()?;
/// ```
#[repr(transparent)]
pub struct AccountLoader<'info, T: ProgramOwned + Discriminator + InnerSize> {
    info: &'info AccountInfo,
    _marker: PhantomData<T>,
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> AccountLoader<'info, T> {
    /// Create a new `AccountLoader` wrapper after validating owner, discriminator, and size
    ///
    /// # Errors
    ///
    /// - `ProgramError::IncorrectProgramId` if the account owner doesn't match
    /// - `ProgramError::InvalidAccountData` if the discriminator doesn't match or data is too small
    #[inline]
    pub fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        info.assert_owner_no_trace(&T::PROGRAM_ID)?;

        // Check minimum size (uses InitSpace trait = DISCRIMINATOR_SIZE + INNER_SIZE)
        info.assert_min_data_len_no_trace(T::INIT_SPACE)?;

        // Check discriminator (first 8 bytes)
        info.assert_discriminator_no_trace(T::DISCRIMINATOR)?;

        Ok(Self {
            info,
            _marker: PhantomData,
        })
    }
}

// Methods that require Pod bound for data access
impl<'info, T: Pod + ProgramOwned + Discriminator + InnerSize> AccountLoader<'info, T> {
    /// Load immutable access to the account data.
    ///
    /// Returns a `Ref` that automatically manages the borrow lifetime.
    /// Validation already happened at construction time.
    ///
    /// # Warning
    ///
    /// Using this method directly is **not recommended** because borrow drops
    /// are not automatically managed. If the `Ref` is not dropped before
    /// another borrow attempt, you will get a runtime panic.
    ///
    /// Prefer using [`inspect`](Self::inspect) or [`map`](Self::map) instead,
    /// which automatically manage borrow lifetimes.
    #[inline]
    pub fn load(&self) -> Result<Ref<'info, T>, ProgramError> {
        let data = self.info.try_borrow_data()?;
        Ok(map_ref(data))
    }

    /// Load mutable access to the account data.
    ///
    /// Returns a `RefMut` that automatically manages the borrow lifetime.
    /// Validation already happened at construction time.
    ///
    /// # Warning
    ///
    /// Using this method directly is **not recommended** because borrow drops
    /// are not automatically managed. If the `RefMut` is not dropped before
    /// another borrow attempt, you will get a runtime panic.
    ///
    /// Prefer using [`inspect_mut`](Self::inspect_mut) or [`map_mut`](Self::map_mut)
    /// instead, which automatically manage borrow lifetimes.
    #[inline]
    pub fn load_mut(&self) -> Result<RefMut<'info, T>, ProgramError> {
        let data = self.info.try_borrow_mut_data()?;
        Ok(map_ref_mut(data))
    }

    /// Inspect account data immutably with automatic borrow management.
    ///
    /// Loads the account immutably, calls the closure with `&T`,
    /// and automatically drops the borrow when the closure returns.
    ///
    /// # Example
    /// ```ignore
    /// account.inspect(|miner| {
    ///     println!("Balance: {}", miner.balance);
    /// })?;
    /// // Borrow is dropped here, safe to perform other operations
    /// ```
    #[inline]
    pub fn inspect<F>(&self, f: F) -> Result<(), ProgramError>
    where
        F: FnOnce(&T),
    {
        let account = self.load()?;
        f(&account);
        Ok(())
    }

    /// Map account data immutably with automatic borrow management, returning a value.
    ///
    /// Loads the account immutably, calls the closure with `&T`,
    /// returns the closure's result, and automatically drops the borrow.
    ///
    /// # Example
    /// ```ignore
    /// let balance = account.map(|miner| miner.balance)?;
    /// // Borrow is dropped here, safe to perform other operations
    /// ```
    #[inline]
    pub fn map<F, R>(&self, f: F) -> Result<R, ProgramError>
    where
        F: FnOnce(&T) -> R,
    {
        let account = self.load()?;
        Ok(f(&account))
    }

    /// Inspect and modify account data with automatic borrow management.
    ///
    /// Loads the account mutably, calls the closure with `&mut T`,
    /// and automatically drops the borrow when the closure returns.
    ///
    /// # Example
    /// ```ignore
    /// account.inspect_mut(|miner| {
    ///     miner.balance += amount;
    /// })?;
    /// // Borrow is dropped here, safe to perform CPIs
    /// ```
    #[inline]
    pub fn inspect_mut<F>(&self, f: F) -> Result<(), ProgramError>
    where
        F: FnOnce(&mut T),
    {
        let mut account = self.load_mut()?;
        f(&mut account);
        Ok(())
    }

    /// Map account data with automatic borrow management, returning a value.
    ///
    /// Loads the account mutably, calls the closure with `&mut T`,
    /// returns the closure's result, and automatically drops the borrow.
    ///
    /// # Example
    /// ```ignore
    /// let balance = account.map_mut(|miner| {
    ///     miner.balance += amount;
    ///     miner.balance
    /// })?;
    /// // Borrow is dropped here, safe to perform CPIs
    /// ```
    #[inline]
    pub fn map_mut<F, R>(&self, f: F) -> Result<R, ProgramError>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut account = self.load_mut()?;
        Ok(f(&mut account))
    }

    /// Inspect account data immutably with a fallible closure.
    ///
    /// Like `inspect`, but the closure can return an error.
    ///
    /// # Example
    /// ```ignore
    /// account.try_inspect(|mine| {
    ///     mine.flags.assert_not_paused()
    /// })?;
    /// ```
    #[inline]
    pub fn try_inspect<F>(&self, f: F) -> Result<(), ProgramError>
    where
        F: FnOnce(&T) -> Result<(), ProgramError>,
    {
        let account = self.load()?;
        f(&account)
    }

    /// Map account data immutably with a fallible closure, returning a value.
    ///
    /// Like `map`, but the closure can return an error.
    ///
    /// # Example
    /// ```ignore
    /// let value = account.try_map(|mine| {
    ///     mine.flags.assert_not_paused()?;
    ///     Ok(mine.treasury.balance)
    /// })?;
    /// ```
    #[inline]
    pub fn try_map<F, R>(&self, f: F) -> Result<R, ProgramError>
    where
        F: FnOnce(&T) -> Result<R, ProgramError>,
    {
        let account = self.load()?;
        f(&account)
    }

    /// Inspect and modify account data with a fallible closure.
    ///
    /// Like `inspect_mut`, but the closure can return an error.
    ///
    /// # Example
    /// ```ignore
    /// account.try_inspect_mut(|mine| {
    ///     mine.flags.assert_not_paused()?;
    ///     mine.treasury.balance += amount;
    ///     Ok(())
    /// })?;
    /// ```
    #[inline]
    pub fn try_inspect_mut<F>(&self, f: F) -> Result<(), ProgramError>
    where
        F: FnOnce(&mut T) -> Result<(), ProgramError>,
    {
        let mut account = self.load_mut()?;
        f(&mut account)
    }

    /// Map account data mutably with a fallible closure, returning a value.
    ///
    /// Like `map_mut`, but the closure can return an error.
    ///
    /// # Example
    /// ```ignore
    /// let balance = account.try_map_mut(|mine| {
    ///     mine.flags.assert_not_paused()?;
    ///     mine.treasury.balance += amount;
    ///     Ok(mine.treasury.balance)
    /// })?;
    /// ```
    #[inline]
    pub fn try_map_mut<F, R>(&self, f: F) -> Result<R, ProgramError>
    where
        F: FnOnce(&mut T) -> Result<R, ProgramError>,
    {
        let mut account = self.load_mut()?;
        f(&mut account)
    }
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> AsAccountInfo<'info>
    for AccountLoader<'info, T>
{
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> AsAccountInfo<'info>
    for &AccountLoader<'info, T>
{
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> core::ops::Deref
    for AccountLoader<'info, T>
{
    type Target = AccountInfo;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> TryFrom<&'info AccountInfo>
    for AccountLoader<'info, T>
{
    type Error = ProgramError;

    #[inline]
    fn try_from(info: &'info AccountInfo) -> Result<Self, Self::Error> {
        Self::new(info)
    }
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> From<AccountLoader<'info, T>>
    for &'info AccountInfo
{
    #[inline]
    fn from(loader: AccountLoader<'info, T>) -> Self {
        loader.info
    }
}

impl<'info, T: ProgramOwned + Discriminator + InnerSize> From<&AccountLoader<'info, T>>
    for &'info AccountInfo
{
    #[inline]
    fn from(loader: &AccountLoader<'info, T>) -> Self {
        loader.info
    }
}
