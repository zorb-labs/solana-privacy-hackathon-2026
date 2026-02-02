//! `AccountDeserialize` trait for deserializing account data

use pinocchio::account_info::{AccountInfo, Ref};
use pinocchio::program_error::ProgramError;

use crate::ProgramOwned;

/// Trait for deserializing account data from an `AccountInfo`.
///
/// This trait is automatically implemented for types that implement
/// `ProgramOwned + Pod + Discriminator + InnerSize` using bytemuck for zero-copy access.
///
/// For SPL Token accounts (`TokenAccount`, `Mint`), implementations use
/// the `pinocchio_token` deserialization.
pub trait AccountDeserialize: ProgramOwned + Sized {
    /// Deserialize account data from an `AccountInfo`.
    ///
    /// Implementations should verify the account owner matches `Self::PROGRAM_ID`.
    fn deserialize(info: &AccountInfo) -> Result<Ref<'_, Self>, ProgramError>;
}

/// Blanket implementation for all Pod types with discriminators
impl<T> AccountDeserialize for T
where
    T: ProgramOwned + crate::Discriminator + crate::InnerSize + bytemuck::Pod,
{
    fn deserialize(info: &AccountInfo) -> Result<Ref<'_, Self>, ProgramError> {
        use crate::account_loaders::load_account;

        // load_account checks owner, discriminator, size, and deserializes
        load_account::<Self>(info)
    }
}
