//! PDA account creation helpers

use bytemuck::Pod;
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_system::instructions::CreateAccount;

use crate::{
    AccountAssertions, InnerSize, ProgramOwned, SetBump, SetDiscriminator,
    constants::SYSTEM_PROGRAM_ID, discriminator::DISCRIMINATOR_LEN,
};

/// Extension trait for creating PDA accounts
///
/// Provides methods to create PDA accounts with automatic size calculation.
///
/// # Example
///
/// ```ignore
/// use panchor::prelude::*;
///
/// // Create and initialize a PDA account
/// mine_info
///     .init_account_with_pda::<Mine>(payer, &seeds, system_program, bump)?
///     .inspect_mut(|mine| {
///         mine.init(params);
///     })?;
/// ```
pub trait CreatePda {
    /// Create a PDA account with explicit space and owner
    ///
    /// Low-level helper for creating PDA accounts. Use this when you need
    /// explicit control over account size and owner, such as for zero-data
    /// accounts that only hold SOL.
    ///
    /// # Arguments
    ///
    /// * `payer` - The account paying for rent
    /// * `signer_seeds` - Seeds for PDA signing (including bump)
    /// * `system_program` - The system program account (validated internally)
    /// * `space` - Account data size in bytes
    /// * `owner` - Program that will own this account
    fn create_pda_account_with_space(
        &self,
        payer: &AccountInfo,
        signer_seeds: &[Seed],
        system_program: &AccountInfo,
        space: usize,
        owner: &Pubkey,
    ) -> Result<&Self, ProgramError>;

    /// Create a PDA account and set discriminator and bump
    ///
    /// Creates a new PDA account using the system program CPI,
    /// automatically calculating the required lamports for rent exemption.
    /// Sets the discriminator and bump using the `SetDiscriminator` and `SetBump` traits.
    ///
    /// This is the preferred method for creating PDA accounts when the account type
    /// implements `SetBump` (via `#[account(..., bump)]`).
    ///
    /// # Type Parameters
    ///
    /// * `T` - The account type that implements the required traits
    ///
    /// # Arguments
    ///
    /// * `payer` - The account paying for rent
    /// * `signer_seeds` - Seeds for PDA signing (including bump)
    /// * `system_program` - The system program account (validated internally)
    /// * `bump` - The PDA bump seed
    fn create_account_with_pda<T: Pod + SetDiscriminator + SetBump + InnerSize + ProgramOwned>(
        &self,
        payer: &AccountInfo,
        signer_seeds: &[Seed],
        system_program: &AccountInfo,
        bump: u8,
    ) -> Result<&Self, ProgramError>;

    /// Create and initialize a PDA account using `SetDiscriminator` and `SetBump`
    ///
    /// Creates the account, sets the discriminator and bump, and returns an
    /// `AccountLoader<T>` for further initialization via `inspect_mut`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The account type that implements the required traits
    ///
    /// # Arguments
    ///
    /// * `payer` - The account paying for rent
    /// * `signer_seeds` - Seeds for PDA signing (including bump)
    /// * `system_program` - The system program account (validated internally)
    /// * `bump` - The PDA bump seed
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = account_info.init_account_with_pda::<MyAccount>(
    ///     payer, &seeds, system_program, bump
    /// )?;
    /// loader.inspect_mut(|data| {
    ///     data.field = value;
    /// })?;
    /// ```
    fn init_account_with_pda<'a, T: Pod + SetDiscriminator + SetBump + InnerSize + ProgramOwned>(
        &'a self,
        payer: &AccountInfo,
        signer_seeds: &[Seed],
        system_program: &AccountInfo,
        bump: u8,
    ) -> Result<crate::AccountLoader<'a, T>, ProgramError>;
}

impl CreatePda for AccountInfo {
    fn create_pda_account_with_space(
        &self,
        payer: &AccountInfo,
        signer_seeds: &[Seed],
        system_program: &AccountInfo,
        space: usize,
        owner: &Pubkey,
    ) -> Result<&Self, ProgramError> {
        // Validate system program
        system_program.assert_program(&SYSTEM_PROGRAM_ID)?;

        // Skip if account already exists
        if !self.data_is_empty() {
            return Ok(self);
        }

        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(space);

        CreateAccount {
            from: payer,
            to: self,
            lamports,
            space: space as u64,
            owner,
        }
        .invoke_signed(&[Signer::from(signer_seeds)])?;

        Ok(self)
    }

    fn create_account_with_pda<T: Pod + SetDiscriminator + SetBump + InnerSize + ProgramOwned>(
        &self,
        payer: &AccountInfo,
        signer_seeds: &[Seed],
        system_program: &AccountInfo,
        bump: u8,
    ) -> Result<&Self, ProgramError> {
        // Create the account with auto-calculated space
        let space = DISCRIMINATOR_LEN
            .checked_add(T::INNER_SIZE)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        self.create_pda_account_with_space(
            payer,
            signer_seeds,
            system_program,
            space,
            &T::PROGRAM_ID,
        )?;

        // Set discriminator and bump
        {
            let mut data = self.try_borrow_mut_data()?;
            T::set_discriminator(&mut data);
            let account_data = data
                .get_mut(DISCRIMINATOR_LEN..)
                .ok_or(ProgramError::AccountDataTooSmall)?;
            let account: &mut T = bytemuck::from_bytes_mut(account_data);
            account.set_bump(bump);
        }

        Ok(self)
    }

    fn init_account_with_pda<'a, T: Pod + SetDiscriminator + SetBump + InnerSize + ProgramOwned>(
        &'a self,
        payer: &AccountInfo,
        signer_seeds: &[Seed],
        system_program: &AccountInfo,
        bump: u8,
    ) -> Result<crate::AccountLoader<'a, T>, ProgramError> {
        // Create the account and set discriminator/bump
        self.create_account_with_pda::<T>(payer, signer_seeds, system_program, bump)?;

        // Return an AccountLoader for further initialization
        crate::AccountLoader::try_from(self)
    }
}
