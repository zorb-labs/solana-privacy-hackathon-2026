//! Program account wrapper
//!
//! [`Program<'info, T>`] wraps an `AccountInfo` and validates the account is executable
//! and matches the expected program ID at construction time.

use core::marker::PhantomData;

use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;

use super::super::{AsAccountInfo, Id};

/// A program account wrapper that validates the account is executable and has the correct ID.
///
/// `Program<'info, T>` wraps an `AccountInfo` and ensures at construction time that:
/// 1. The account's key matches `T::ID` (from the `Id` trait)
/// 2. The account is executable
///
/// # Type Parameters
///
/// - `'info` - The lifetime of the account info slice
/// - `T` - The program marker type, must implement `Id`
///
/// # Example
///
/// ```ignore
/// // Define a program marker
/// pub struct TokenProgram;
///
/// impl Id for TokenProgram {
///     const ID: Pubkey = TOKEN_PROGRAM_ID;
/// }
///
/// #[derive(Accounts)]
/// pub struct MyAccounts<'info> {
///     pub token_program: Program<'info, TokenProgram>,
/// }
/// ```
#[repr(transparent)]
pub struct Program<'info, T: Id> {
    info: &'info AccountInfo,
    _marker: PhantomData<T>,
}

impl<'info, T: Id> Program<'info, T> {
    /// Create a new Program wrapper after validating the address and executable flag
    ///
    /// # Errors
    ///
    /// - `ProgramError::IncorrectProgramId` if the account key doesn't match
    /// - `ProgramError::InvalidAccountData` if the account is not executable
    #[inline]
    pub fn new(info: &'info AccountInfo) -> Result<Self, ProgramError> {
        // Check address
        if info.key() != &T::ID {
            return Err(ProgramError::IncorrectProgramId);
        }

        // Check executable
        if !info.executable() {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(Self {
            info,
            _marker: PhantomData,
        })
    }

    /// Create a new Program wrapper without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the account key matches the expected program ID
    /// and the account is executable. Using this with an incorrect account can
    /// lead to calling malicious programs.
    #[inline]
    pub const unsafe fn new_unchecked(info: &'info AccountInfo) -> Self {
        Self {
            info,
            _marker: PhantomData,
        }
    }
}

impl<'info, T: Id> AsAccountInfo<'info> for Program<'info, T> {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info, T: Id> AsAccountInfo<'info> for &Program<'info, T> {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self.info
    }
}

impl<'info, T: Id> core::ops::Deref for Program<'info, T> {
    type Target = AccountInfo;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'info, T: Id> TryFrom<&'info AccountInfo> for Program<'info, T> {
    type Error = ProgramError;

    #[inline]
    fn try_from(info: &'info AccountInfo) -> Result<Self, Self::Error> {
        Self::new(info)
    }
}

impl<'info, T: Id> From<Program<'info, T>> for &'info AccountInfo {
    #[inline]
    fn from(program: Program<'info, T>) -> Self {
        program.info
    }
}

impl<'info, T: Id> From<&Program<'info, T>> for &'info AccountInfo {
    #[inline]
    fn from(program: &Program<'info, T>) -> Self {
        program.info
    }
}
