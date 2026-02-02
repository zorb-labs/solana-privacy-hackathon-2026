//! SPL Token account helpers

use pinocchio::account_info::{AccountInfo, Ref};
use pinocchio::program_error::ProgramError;
use pinocchio::pubkey::Pubkey;
use pinocchio_contrib::AccountAssertionsNoTrace;
use pinocchio_contrib::constants::TOKEN_PROGRAM_ID;
use pinocchio_token::state::{Mint, TokenAccount};

use crate::ProgramOwned;
use crate::accounts::{AccountDataValidate, AccountDeserialize};

// SPL Token account sizes (from spl-token)
const TOKEN_ACCOUNT_LEN: usize = 165;
const MINT_LEN: usize = 82;

/// Extension trait for loading SPL Token accounts
pub trait TokenAccountExt {
    /// Load this account as an SPL Token account
    ///
    /// Returns `ProgramError::InvalidAccountData` if the account is invalid.
    fn as_token_account(&self) -> Result<Ref<'_, TokenAccount>, ProgramError>;

    /// Load this account as an SPL Mint
    ///
    /// Returns `ProgramError::InvalidAccountData` if the account is invalid.
    fn as_mint(&self) -> Result<Ref<'_, Mint>, ProgramError>;
}

impl TokenAccountExt for AccountInfo {
    fn as_token_account(&self) -> Result<Ref<'_, TokenAccount>, ProgramError> {
        TokenAccount::from_account_info(self)
    }

    fn as_mint(&self) -> Result<Ref<'_, Mint>, ProgramError> {
        Mint::from_account_info(self)
    }
}

impl ProgramOwned for TokenAccount {
    const PROGRAM_ID: Pubkey = TOKEN_PROGRAM_ID;
}

impl ProgramOwned for Mint {
    const PROGRAM_ID: Pubkey = TOKEN_PROGRAM_ID;
}

impl AccountDataValidate for TokenAccount {
    fn validate(info: &AccountInfo) -> Result<(), ProgramError> {
        info.assert_data_len_no_trace(TOKEN_ACCOUNT_LEN)?;
        Ok(())
    }
}

impl AccountDataValidate for Mint {
    fn validate(info: &AccountInfo) -> Result<(), ProgramError> {
        info.assert_data_len_no_trace(MINT_LEN)?;
        Ok(())
    }
}

impl AccountDeserialize for TokenAccount {
    fn deserialize(info: &AccountInfo) -> Result<Ref<'_, Self>, ProgramError> {
        // TokenAccount::from_account_info already checks the owner
        Self::from_account_info(info)
    }
}

impl AccountDeserialize for Mint {
    fn deserialize(info: &AccountInfo) -> Result<Ref<'_, Self>, ProgramError> {
        // Mint::from_account_info already checks the owner
        Self::from_account_info(info)
    }
}
