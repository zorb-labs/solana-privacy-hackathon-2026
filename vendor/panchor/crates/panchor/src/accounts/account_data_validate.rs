//! `AccountDataValidate` trait for validating account data without deserializing

use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;

use crate::AccountAssertionsNoTrace;
use crate::discriminator::DISCRIMINATOR_LEN;

/// Trait for validating account data without deserializing.
///
/// This trait is used by `LazyAccount` to validate accounts at construction time
/// without the overhead of full deserialization. The actual deserialization
/// happens later via `AccountDeserialize::deserialize()`.
///
/// Automatically implemented for types with `InitSpace + Discriminator`.
pub trait AccountDataValidate {
    /// Validate that the account data is valid for this type.
    ///
    /// This should check:
    /// - Minimum size requirements
    /// - Discriminator (if applicable)
    /// - Any other structural validation
    ///
    /// Does NOT deserialize the data.
    fn validate(info: &AccountInfo) -> Result<(), ProgramError>;
}

/// Blanket implementation for types with `InitSpace` + Discriminator (program accounts)
impl<T> AccountDataValidate for T
where
    T: crate::space::InitSpace + crate::Discriminator,
{
    fn validate(info: &AccountInfo) -> Result<(), ProgramError> {
        // Check minimum size
        info.assert_min_data_len_no_trace(T::INIT_SPACE)?;

        let data = info.try_borrow_data()?;
        // Check discriminator
        let discriminator = u64::from_le_bytes(data[..DISCRIMINATOR_LEN].try_into().unwrap());
        if discriminator != T::DISCRIMINATOR {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(())
    }
}
