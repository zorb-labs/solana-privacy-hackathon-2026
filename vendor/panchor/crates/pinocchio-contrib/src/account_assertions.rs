//! Account assertion trait for validating account properties
//!
//! Provides chainable assertion methods for `AccountInfo` validation.

use pinocchio::account_info::AccountInfo;
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::{Pubkey, find_program_address, pubkey_eq};

use crate::account_assertions_no_trace::AccountAssertionsNoTrace;
use crate::constants::{ASSOCIATED_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID};
use crate::{bail_err, require};

/// Extension trait for `AccountInfo` validation and assertion methods
///
/// Provides chainable assertion methods that can be called inline:
///
/// ```ignore
/// use pinocchio_contrib::prelude::*;
///
/// // Chain multiple assertions
/// signer.assert_signer()?.assert_writable()?;
/// ```
pub trait AccountAssertions {
    /// Assert that this account is a signer
    ///
    /// Returns `ProgramError::MissingRequiredSignature` if not a signer.
    fn assert_signer(&self) -> Result<&Self, ProgramError>;

    /// Assert that this account is writable
    ///
    /// Returns `ProgramError::Immutable` if not writable.
    fn assert_writable(&self) -> Result<&Self, ProgramError>;

    /// Assert that this account is owned by the expected program
    ///
    /// Returns `ProgramError::IllegalOwner` if owner doesn't match.
    fn assert_owner(&self, expected_owner: &Pubkey) -> Result<&Self, ProgramError>;

    /// Assert that this account has the expected key
    ///
    /// Returns `ProgramError::InvalidAccountData` if key doesn't match.
    fn assert_key(&self, expected_key: &Pubkey) -> Result<&Self, ProgramError>;

    /// Assert that this account is the expected authority
    ///
    /// Same as `assert_key` but returns `ProgramError::IncorrectAuthority`.
    fn assert_is_authority(&self, expected_authority: &Pubkey) -> Result<&Self, ProgramError>;

    /// Assert that this account is empty (uninitialized)
    ///
    /// Returns `ProgramError::AccountAlreadyInitialized` if not empty.
    fn assert_empty(&self) -> Result<&Self, ProgramError>;

    /// Assert that this account is not empty (initialized)
    ///
    /// Returns `ProgramError::UninitializedAccount` if empty.
    fn assert_not_empty(&self) -> Result<&Self, ProgramError>;

    /// Assert that this account's key matches the expected PDA derived from seeds
    ///
    /// Derives the expected PDA from the given seeds and `program_id`, then verifies
    /// this account's key matches. Returns `ProgramError::InvalidSeeds` if the
    /// derived key doesn't match.
    ///
    /// # Arguments
    /// * `seeds` - The seeds to derive the expected PDA
    /// * `program_id` - The program ID to derive the PDA from
    ///
    /// # Example
    /// ```ignore
    /// let seeds = &[b"stake", mine_info.key().as_ref(), signer.key().as_ref()];
    /// stake_info.assert_seeds(seeds, program_id)?;
    /// ```
    fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError>;

    /// Assert that this account is an Associated Token Account (ATA) for the given wallet and mint
    ///
    /// Derives the expected ATA address and verifies this account's key matches.
    /// Also verifies the account is owned by the Token Program.
    ///
    /// # Arguments
    /// * `wallet` - The wallet address that owns the ATA
    /// * `mint` - The mint address for the token
    /// * `token_program` - The token program ID (usually SPL Token)
    fn assert_ata(
        &self,
        wallet: &Pubkey,
        mint: &Pubkey,
        token_program: &Pubkey,
    ) -> Result<&Self, ProgramError>;

    /// Assert that this account's key matches the expected program ID
    ///
    /// Returns `ProgramError::IncorrectProgramId` if key doesn't match.
    /// Use this for validating program accounts passed for CPI.
    fn assert_program(&self, expected_program_id: &Pubkey) -> Result<&Self, ProgramError>;

    /// Check if this account's key is the system program (default pubkey / all zeros)
    ///
    /// Useful for checking if an optional account was provided.
    fn is_system_program(&self) -> bool;

    /// Assert that this account's data length matches the expected size
    ///
    /// Returns `ProgramError::InvalidAccountData` if the data length doesn't match.
    fn assert_data_len(&self, expected_len: usize) -> Result<&Self, ProgramError>;
}

impl AccountAssertions for AccountInfo {
    #[track_caller]
    fn assert_signer(&self) -> Result<&Self, ProgramError> {
        self.assert_signer_no_trace()
            .map_err(|e| crate::error::trace("assert_signer failed", e))
            .map(|()| self)
    }

    #[track_caller]
    fn assert_writable(&self) -> Result<&Self, ProgramError> {
        self.assert_writable_no_trace()
            .map_err(|e| crate::error::trace("assert_writable failed", e))
            .map(|()| self)
    }

    #[track_caller]
    fn assert_owner(&self, expected_owner: &Pubkey) -> Result<&Self, ProgramError> {
        self.assert_owner_no_trace(expected_owner)
            .map_err(|e| crate::error::trace("assert_owner failed", e))
            .map(|()| self)
    }

    #[track_caller]
    fn assert_key(&self, expected_key: &Pubkey) -> Result<&Self, ProgramError> {
        self.assert_key_no_trace(expected_key)
            .map_err(|e| crate::error::trace("assert_key failed", e))
            .map(|()| self)
    }

    #[track_caller]
    fn assert_is_authority(&self, expected_authority: &Pubkey) -> Result<&Self, ProgramError> {
        require!(
            pubkey_eq(self.key(), expected_authority),
            ProgramError::IncorrectAuthority,
            "assert_is_authority failed"
        );
        Ok(self)
    }

    #[track_caller]
    fn assert_empty(&self) -> Result<&Self, ProgramError> {
        require!(
            self.data_is_empty(),
            ProgramError::AccountAlreadyInitialized,
            "assert_empty failed"
        );
        Ok(self)
    }

    #[track_caller]
    fn assert_not_empty(&self) -> Result<&Self, ProgramError> {
        require!(
            !self.data_is_empty(),
            ProgramError::UninitializedAccount,
            "assert_not_empty failed"
        );
        Ok(self)
    }

    #[track_caller]
    fn assert_seeds(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<&Self, ProgramError> {
        let (expected_key, _bump) = find_program_address(seeds, program_id);
        require!(
            pubkey_eq(self.key(), &expected_key),
            ProgramError::InvalidSeeds,
            "assert_seeds failed"
        );
        Ok(self)
    }

    #[track_caller]
    fn assert_ata(
        &self,
        wallet: &Pubkey,
        mint: &Pubkey,
        token_program: &Pubkey,
    ) -> Result<&Self, ProgramError> {
        // Derive expected ATA address
        // seeds = [wallet, token_program, mint]
        let seeds: &[&[u8]] = &[wallet.as_ref(), token_program.as_ref(), mint.as_ref()];
        let (expected_ata, _bump) = find_program_address(seeds, &ASSOCIATED_TOKEN_PROGRAM_ID);

        require!(
            pubkey_eq(self.key(), &expected_ata),
            ProgramError::InvalidAccountData,
            "assert_ata key mismatch"
        );

        // Verify owner is token program (skip if account not yet created)
        if !self.data_is_empty() && !pubkey_eq(self.owner(), token_program) {
            bail_err!(ProgramError::IllegalOwner, "assert_ata owner mismatch");
        }

        Ok(self)
    }

    #[track_caller]
    fn assert_program(&self, expected_program_id: &Pubkey) -> Result<&Self, ProgramError> {
        self.assert_program_no_trace(expected_program_id)
            .map_err(|e| crate::error::trace("assert_program failed", e))
            .map(|()| self)
    }

    fn is_system_program(&self) -> bool {
        pubkey_eq(self.key(), &SYSTEM_PROGRAM_ID)
    }

    #[track_caller]
    fn assert_data_len(&self, expected_len: usize) -> Result<&Self, ProgramError> {
        self.assert_data_len_no_trace(expected_len)
            .map_err(|e| crate::error::trace("assert_data_len failed", e))
            .map(|()| self)
    }
}
