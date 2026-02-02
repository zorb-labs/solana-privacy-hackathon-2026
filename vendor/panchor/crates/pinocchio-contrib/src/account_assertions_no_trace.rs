//! Account assertion trait without tracing for use in derive macros
//!
//! These methods are used by the `#[derive(Accounts)]` macro where we already
//! log the field name via `inspect_err`. Use these when you don't need the
//! additional tracing from `track_caller`.

use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::{Pubkey, pubkey_eq};

/// Core assertion methods without tracing (for use in derive macros)
///
/// These methods are used by the `#[derive(Accounts)]` macro where we already
/// log the field name via `inspect_err`. Use these when you don't need the
/// additional tracing from `track_caller`.
pub trait AccountAssertionsNoTrace {
    /// Assert that this account is a signer without tracing
    ///
    /// Returns `ProgramError::MissingRequiredSignature` if not a signer.
    fn assert_signer_no_trace(&self) -> Result<(), ProgramError>;

    /// Assert that this account is writable without tracing
    ///
    /// Returns `ProgramError::Immutable` if not writable.
    fn assert_writable_no_trace(&self) -> Result<(), ProgramError>;

    /// Assert that this account has the expected key without tracing
    ///
    /// Returns `ProgramError::InvalidAccountData` if key doesn't match.
    fn assert_key_no_trace(&self, expected_key: &Pubkey) -> Result<(), ProgramError>;

    /// Assert that this account's key matches the expected PDA key without tracing
    ///
    /// Returns `ProgramError::InvalidSeeds` if key doesn't match.
    /// Use this for PDA validation to get a more specific error.
    fn assert_key_derived_from_seeds_no_trace(
        &self,
        expected_key: &Pubkey,
    ) -> Result<(), ProgramError>;

    /// Assert that this account is owned by the expected program without tracing
    ///
    /// Returns `ProgramError::IllegalOwner` if owner doesn't match.
    fn assert_owner_no_trace(&self, expected_owner: &Pubkey) -> Result<(), ProgramError>;

    /// Assert that this account's data length is at least `min_len` without tracing
    ///
    /// Returns `ProgramError::AccountDataTooSmall` if data is smaller than required.
    fn assert_min_data_len_no_trace(&self, min_len: usize) -> Result<(), ProgramError>;

    /// Assert that this account's data length matches exactly without tracing
    ///
    /// Returns `ProgramError::InvalidAccountData` if length doesn't match.
    fn assert_data_len_no_trace(&self, expected_len: usize) -> Result<(), ProgramError>;

    /// Assert that this account's key matches the expected program ID without tracing
    ///
    /// Returns `ProgramError::IncorrectProgramId` if key doesn't match or not executable.
    fn assert_program_no_trace(&self, expected_program_id: &Pubkey) -> Result<(), ProgramError>;

    /// Assert that this account is executable without tracing
    ///
    /// Returns `ProgramError::InvalidAccountData` if not executable.
    fn assert_executable_no_trace(&self) -> Result<(), ProgramError>;

    /// Assert that this account is empty (zero data length) without tracing
    ///
    /// Returns `ProgramError::AccountAlreadyInitialized` if account has data.
    fn assert_empty_no_trace(&self) -> Result<(), ProgramError>;

    /// Assert that this account's discriminator matches the expected value without tracing
    ///
    /// Reads the first 8 bytes of account data as a little-endian u64 and compares
    /// to the expected discriminator.
    ///
    /// Returns `ProgramError::InvalidAccountData` if discriminator doesn't match.
    fn assert_discriminator_no_trace(
        &self,
        expected_discriminator: u64,
    ) -> Result<(), ProgramError>;
}

impl AccountAssertionsNoTrace for AccountInfo {
    #[inline(always)]
    fn assert_signer_no_trace(&self) -> Result<(), ProgramError> {
        if !self.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_writable_no_trace(&self) -> Result<(), ProgramError> {
        if !self.is_writable() {
            return Err(ProgramError::Immutable);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_key_no_trace(&self, expected_key: &Pubkey) -> Result<(), ProgramError> {
        if !pubkey_eq(self.key(), expected_key) {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_key_derived_from_seeds_no_trace(
        &self,
        expected_key: &Pubkey,
    ) -> Result<(), ProgramError> {
        if !pubkey_eq(self.key(), expected_key) {
            return Err(ProgramError::InvalidSeeds);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_owner_no_trace(&self, expected_owner: &Pubkey) -> Result<(), ProgramError> {
        if !pubkey_eq(self.owner(), expected_owner) {
            return Err(ProgramError::IllegalOwner);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_min_data_len_no_trace(&self, min_len: usize) -> Result<(), ProgramError> {
        if self.data_len() < min_len {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_data_len_no_trace(&self, expected_len: usize) -> Result<(), ProgramError> {
        if self.data_len() != expected_len {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_program_no_trace(&self, expected_program_id: &Pubkey) -> Result<(), ProgramError> {
        if !pubkey_eq(self.key(), expected_program_id) {
            return Err(ProgramError::IncorrectProgramId);
        }
        if !self.executable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_executable_no_trace(&self) -> Result<(), ProgramError> {
        if !self.executable() {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_empty_no_trace(&self) -> Result<(), ProgramError> {
        if !self.data_is_empty() {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        Ok(())
    }

    #[inline(always)]
    fn assert_discriminator_no_trace(
        &self,
        expected_discriminator: u64,
    ) -> Result<(), ProgramError> {
        let data = self.try_borrow_data()?;
        if data.len() < 8 {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let disc = u64::from_le_bytes(data[0..8].try_into().unwrap());
        if disc != expected_discriminator {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use pinocchio_test_utils::AccountInfoBuilder;

    use super::*;

    // assert_signer_no_trace tests

    #[test]
    fn test_assert_signer_no_trace_success() {
        let account = AccountInfoBuilder::new().signer(true).build();
        let info = account.info();

        info.assert_signer_no_trace().unwrap();
    }

    #[test]
    fn test_assert_signer_no_trace_failure() {
        let account = AccountInfoBuilder::new().signer(false).build();
        let info = account.info();

        let result = info.assert_signer_no_trace();
        assert_eq!(result, Err(ProgramError::MissingRequiredSignature));
    }

    // assert_writable_no_trace tests

    #[test]
    fn test_assert_writable_no_trace_success() {
        let account = AccountInfoBuilder::new().writable(true).build();
        let info = account.info();

        info.assert_writable_no_trace().unwrap();
    }

    #[test]
    fn test_assert_writable_no_trace_failure() {
        let account = AccountInfoBuilder::new().writable(false).build();
        let info = account.info();

        let result = info.assert_writable_no_trace();
        assert_eq!(result, Err(ProgramError::Immutable));
    }

    // assert_key_no_trace tests

    #[test]
    fn test_assert_key_no_trace_success() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let account = AccountInfoBuilder::new().key(&key).build();
        let info = account.info();

        info.assert_key_no_trace(&key).unwrap();
    }

    #[test]
    fn test_assert_key_no_trace_failure() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let wrong_key = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let account = AccountInfoBuilder::new().key(&key).build();
        let info = account.info();

        let result = info.assert_key_no_trace(&wrong_key);
        assert_eq!(result, Err(ProgramError::InvalidAccountData));
    }

    // assert_key_derived_from_seeds_no_trace tests

    #[test]
    fn test_assert_key_derived_from_seeds_no_trace_success() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let account = AccountInfoBuilder::new().key(&key).build();
        let info = account.info();

        info.assert_key_derived_from_seeds_no_trace(&key).unwrap();
    }

    #[test]
    fn test_assert_key_derived_from_seeds_no_trace_failure() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let wrong_key = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let account = AccountInfoBuilder::new().key(&key).build();
        let info = account.info();

        let result = info.assert_key_derived_from_seeds_no_trace(&wrong_key);
        assert_eq!(result, Err(ProgramError::InvalidSeeds));
    }

    // assert_owner_no_trace tests

    #[test]
    fn test_assert_owner_no_trace_success() {
        let owner = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let account = AccountInfoBuilder::new().owner(&owner).build();
        let info = account.info();

        info.assert_owner_no_trace(&owner).unwrap();
    }

    #[test]
    fn test_assert_owner_no_trace_failure() {
        let owner = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let wrong_owner = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let account = AccountInfoBuilder::new().owner(&owner).build();
        let info = account.info();

        let result = info.assert_owner_no_trace(&wrong_owner);
        assert_eq!(result, Err(ProgramError::IllegalOwner));
    }

    // assert_min_data_len_no_trace tests

    #[test]
    fn test_assert_min_data_len_no_trace_success_exact() {
        let data = [1u8, 2, 3, 4, 5];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        info.assert_min_data_len_no_trace(5).unwrap();
    }

    #[test]
    fn test_assert_min_data_len_no_trace_success_larger() {
        let data = [1u8, 2, 3, 4, 5];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        info.assert_min_data_len_no_trace(3).unwrap();
    }

    #[test]
    fn test_assert_min_data_len_no_trace_failure() {
        let data = [1u8, 2, 3];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        let result = info.assert_min_data_len_no_trace(5);
        assert_eq!(result, Err(ProgramError::AccountDataTooSmall));
    }

    // assert_data_len_no_trace tests

    #[test]
    fn test_assert_data_len_no_trace_success() {
        let data = [1u8, 2, 3, 4, 5];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        info.assert_data_len_no_trace(5).unwrap();
    }

    #[test]
    fn test_assert_data_len_no_trace_failure_too_small() {
        let data = [1u8, 2, 3];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        let result = info.assert_data_len_no_trace(5);
        assert_eq!(result, Err(ProgramError::InvalidAccountData));
    }

    #[test]
    fn test_assert_data_len_no_trace_failure_too_large() {
        let data = [1u8, 2, 3, 4, 5, 6, 7];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        let result = info.assert_data_len_no_trace(5);
        assert_eq!(result, Err(ProgramError::InvalidAccountData));
    }

    // assert_program_no_trace tests

    #[test]
    fn test_assert_program_no_trace_success() {
        let program_id = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let account = AccountInfoBuilder::new()
            .key(&program_id)
            .executable(true)
            .build();
        let info = account.info();

        info.assert_program_no_trace(&program_id).unwrap();
    }

    #[test]
    fn test_assert_program_no_trace_failure_not_executable() {
        let program_id = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let account = AccountInfoBuilder::new()
            .key(&program_id)
            .executable(false)
            .build();
        let info = account.info();

        // Key matches but not executable - returns InvalidAccountData
        let result = info.assert_program_no_trace(&program_id);
        assert_eq!(result, Err(ProgramError::InvalidAccountData));
    }

    #[test]
    fn test_assert_program_no_trace_failure_wrong_key() {
        let program_id = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let wrong_program_id = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let account = AccountInfoBuilder::new()
            .key(&program_id)
            .executable(true)
            .build();
        let info = account.info();

        let result = info.assert_program_no_trace(&wrong_program_id);
        assert_eq!(result, Err(ProgramError::IncorrectProgramId));
    }

    // Multiple assertions test

    #[test]
    fn test_multiple_assertions_success() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let owner = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let data = [1u8, 2, 3, 4, 5];

        let account = AccountInfoBuilder::new()
            .key(&key)
            .owner(&owner)
            .signer(true)
            .writable(true)
            .data(&data)
            .build();
        let info = account.info();

        info.assert_signer_no_trace().unwrap();
        info.assert_writable_no_trace().unwrap();
        info.assert_key_no_trace(&key).unwrap();
        info.assert_owner_no_trace(&owner).unwrap();
        info.assert_min_data_len_no_trace(5).unwrap();
    }

    #[test]
    fn test_multiple_assertions_early_failure() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let owner = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

        let account = AccountInfoBuilder::new()
            .key(&key)
            .owner(&owner)
            .signer(false) // Not a signer - should fail
            .writable(true)
            .build();
        let info = account.info();

        let result = info.assert_signer_no_trace();
        assert_eq!(result, Err(ProgramError::MissingRequiredSignature));
    }

    // assert_discriminator_no_trace tests

    #[test]
    fn test_assert_discriminator_no_trace_success() {
        let discriminator: u64 = 0x123456789ABCDEF0;
        let mut data = [0u8; 16];
        data[0..8].copy_from_slice(&discriminator.to_le_bytes());

        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        info.assert_discriminator_no_trace(discriminator).unwrap();
    }

    #[test]
    fn test_assert_discriminator_no_trace_failure_wrong_discriminator() {
        let discriminator: u64 = 0x123456789ABCDEF0;
        let wrong_discriminator: u64 = 0xFEDCBA9876543210;
        let mut data = [0u8; 16];
        data[0..8].copy_from_slice(&discriminator.to_le_bytes());

        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        let result = info.assert_discriminator_no_trace(wrong_discriminator);
        assert_eq!(result, Err(ProgramError::InvalidAccountData));
    }

    #[test]
    fn test_assert_discriminator_no_trace_failure_data_too_small() {
        let data = [0u8; 4]; // Less than 8 bytes

        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        let result = info.assert_discriminator_no_trace(0x12345678);
        assert_eq!(result, Err(ProgramError::AccountDataTooSmall));
    }

    // assert_empty_no_trace tests

    #[test]
    fn test_assert_empty_no_trace_success() {
        let account = AccountInfoBuilder::new().data(&[]).build();
        let info = account.info();

        info.assert_empty_no_trace().unwrap();
    }

    #[test]
    fn test_assert_empty_no_trace_failure() {
        let data = [1u8, 2, 3];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        let result = info.assert_empty_no_trace();
        assert_eq!(result, Err(ProgramError::AccountAlreadyInitialized));
    }
}
