//! Signer account wrapper
//!
//! [`Signer<'info>`] wraps an `AccountInfo` and validates the account is a signer
//! at construction time.

use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;

use super::super::AsAccountInfo;

/// A signer account wrapper that validates the account is a signer.
///
/// `Signer<'info>` wraps an `AccountInfo` and ensures at construction time that
/// the account has signed the transaction.
///
/// # Type Parameters
///
/// - `'info` - The lifetime of the account info slice
///
/// # Example
///
/// ```ignore
/// #[derive(Accounts)]
/// pub struct MyAccounts<'info> {
///     pub payer: Signer<'info>,
/// }
/// ```
#[repr(transparent)]
pub struct Signer<'info> {
    info: &'info AccountInfo,
}

impl<'info> Signer<'info> {
    /// Create a new Signer wrapper after validating the account is a signer
    ///
    /// # Errors
    ///
    /// Returns `ProgramError::MissingRequiredSignature` if the account is not a signer
    #[inline]
    pub fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        if !info.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(Self { info })
    }

    /// Create a new Signer wrapper without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the account is a signer. Using this with a
    /// non-signer account can lead to unauthorized actions.
    #[inline]
    pub const unsafe fn new_unchecked(info: &'info AccountInfo) -> Self {
        Self { info }
    }
}

impl<'info> AsAccountInfo<'info> for Signer<'info> {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info> AsAccountInfo<'info> for &Signer<'info> {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info> core::ops::Deref for Signer<'info> {
    type Target = AccountInfo;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'info> TryFrom<&'info AccountInfo> for Signer<'info> {
    type Error = ProgramError;

    #[inline]
    fn try_from(info: &'info AccountInfo) -> Result<Self, Self::Error> {
        Self::new(info)
    }
}

impl<'info> From<Signer<'info>> for &'info AccountInfo {
    #[inline]
    fn from(signer: Signer<'info>) -> Self {
        signer.info
    }
}

impl<'info> From<&Signer<'info>> for &'info AccountInfo {
    #[inline]
    fn from(signer: &Signer<'info>) -> Self {
        signer.info
    }
}
