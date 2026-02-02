//! `AccountInfo` extension trait for loading account data
//!
//! # Safety
//! These methods use `borrow_*_unchecked` which is safe in the single-threaded
//! Solana runtime. Callers must not hold overlapping mutable borrows.

use bytemuck::Pod;
use pinocchio::account_info::{AccountInfo, Ref};
use pinocchio::program_error::ProgramError;
use pinocchio_contrib::{AccountAssertions, trace};

use crate::accounts::AccountLoader;
use crate::discriminator::DISCRIMINATOR_LEN;
use crate::{Discriminator, InnerSize, ProgramOwned};

/// Verify account data has minimum required size for discriminator + type T.
#[track_caller]
fn verify_minimum_size<T: InnerSize>(data: &[u8]) -> Result<(), ProgramError> {
    let min_size = DISCRIMINATOR_LEN
        .checked_add(T::INNER_SIZE)
        .ok_or_else(|| trace("size overflow", ProgramError::ArithmeticOverflow))?;
    if data.len() < min_size {
        return Err(trace(
            "invalid account data size",
            ProgramError::InvalidAccountData,
        ));
    }
    Ok(())
}

/// Read discriminator from account data and verify it matches expected value.
#[track_caller]
fn verify_discriminator<T: Discriminator>(data: &[u8]) -> Result<(), ProgramError> {
    let discriminator = u64::from_le_bytes(data[..DISCRIMINATOR_LEN].try_into().unwrap());
    if discriminator != T::DISCRIMINATOR {
        return Err(trace(
            "invalid discriminator",
            ProgramError::InvalidAccountData,
        ));
    }
    Ok(())
}

/// Map Ref<[u8]> to Ref<T>, skipping the discriminator.
#[track_caller]
fn map_ref<T: Pod + InnerSize>(data: Ref<'_, [u8]>) -> Result<Ref<'_, T>, ProgramError> {
    Ref::try_map(data, |bytes: &[u8]| {
        let end = DISCRIMINATOR_LEN
            .checked_add(T::INNER_SIZE)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        let slice = bytes
            .get(DISCRIMINATOR_LEN..end)
            .ok_or(ProgramError::AccountDataTooSmall)?;
        bytemuck::try_from_bytes(slice).map_err(|_| ProgramError::InvalidAccountData)
    })
    .map_err(|(_, e)| trace("bytemuck cast failed", e))
}

/// Load account data with ownership and discriminator check (immutable)
///
/// Verifies the account is owned by `T::PROGRAM_ID` before loading.
/// Used by `AccountDeserialize` trait.
#[track_caller]
pub fn load_account<T: Pod + Discriminator + InnerSize + ProgramOwned>(
    info: &AccountInfo,
) -> Result<Ref<'_, T>, ProgramError> {
    info.assert_owner(&T::PROGRAM_ID)?;

    let data = info.try_borrow_data()?;

    verify_minimum_size::<T>(&data)?;
    verify_discriminator::<T>(&data)?;

    map_ref(data)
}

/// Extension trait for `AccountInfo` loading methods
///
/// Provides methods for loading account data with discriminator validation
/// and zero-copy deserialization.
///
/// # Example
/// ```ignore
/// use panchor::prelude::*;
///
/// // Load as typed AccountLoader wrapper
/// mine_info.load::<Mine>()?.inspect(|mine| { ... })?;
///
/// // Or with mutable access
/// miner_info.load::<Miner>()?.inspect_mut(|miner| {
///     miner.balance += amount;
/// })?;
/// ```
pub trait AccountLoaders {
    /// Load an account as a typed wrapper
    ///
    /// Returns an `AccountLoader<T>` wrapper that validates owner, discriminator, and size.
    /// Use the returned wrapper's `inspect`, `inspect_mut`, `map`, `map_mut` methods
    /// to access data with automatic borrow management.
    ///
    /// # Example
    /// ```ignore
    /// mine_info.load::<Mine>()?.inspect(|mine| {
    ///     println!("Balance: {}", mine.treasury.balance);
    /// })?;
    /// ```
    fn load<T: Pod + Discriminator + InnerSize + ProgramOwned>(
        &self,
    ) -> Result<AccountLoader<'_, T>, ProgramError>;
}

impl AccountLoaders for AccountInfo {
    #[track_caller]
    fn load<T: Pod + Discriminator + InnerSize + ProgramOwned>(
        &self,
    ) -> Result<AccountLoader<'_, T>, ProgramError> {
        AccountLoader::try_from(self)
    }
}
