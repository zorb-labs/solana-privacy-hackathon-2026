//! Authority transfer helpers.
//!
//! Provides a trait and helper functions for implementing two-step authority transfer
//! across pool programs. This eliminates duplicated logic in transfer_authority and
//! accept_authority instruction handlers.
//!
//! # Usage
//!
//! 1. Implement `HasAuthority` for your config type
//! 2. Call `transfer_authority_impl` or `accept_authority_impl` from your handler
//!
//! # Example
//!
//! ```ignore
//! impl HasAuthority for MyPoolConfig {
//!     fn authority(&self) -> &Pubkey { &self.authority }
//!     fn authority_mut(&mut self) -> &mut Pubkey { &mut self.authority }
//!     fn pending_authority(&self) -> &Pubkey { &self.pending_authority }
//!     fn pending_authority_mut(&mut self) -> &mut Pubkey { &mut self.pending_authority }
//! }
//!
//! pub fn process_transfer_authority(ctx: Context<...>) -> ProgramResult {
//!     pool_config.try_inspect_mut(|config| {
//!         transfer_authority_impl(config, authority.key(), new_authority.key())
//!     })
//! }
//! ```

use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

/// Trait for config types that support two-step authority transfer.
pub trait HasAuthority {
    /// Get the current authority pubkey.
    fn authority(&self) -> &Pubkey;
    /// Get mutable reference to authority pubkey.
    fn authority_mut(&mut self) -> &mut Pubkey;
    /// Get the pending authority pubkey.
    fn pending_authority(&self) -> &Pubkey;
    /// Get mutable reference to pending authority pubkey.
    fn pending_authority_mut(&mut self) -> &mut Pubkey;
}

/// Implements the transfer_authority logic for any config implementing `HasAuthority`.
///
/// Sets the pending_authority field to the new authority address.
///
/// # Arguments
/// * `config` - Mutable reference to the config account data
/// * `signer` - The pubkey of the signing authority
/// * `new_authority` - The pubkey of the new authority to set as pending
///
/// # Returns
/// * `Ok(())` if the transfer was initiated
/// * `Err(ProgramError::IllegalOwner)` if signer is not the current authority
#[inline]
pub fn transfer_authority_impl<T: HasAuthority>(
    config: &mut T,
    signer: &Pubkey,
    new_authority: &Pubkey,
) -> Result<(), ProgramError> {
    // Verify signer is current authority
    if config.authority() != signer {
        return Err(ProgramError::IllegalOwner);
    }

    // Set pending authority
    *config.pending_authority_mut() = *new_authority;

    Ok(())
}

/// Implements the accept_authority logic for any config implementing `HasAuthority`.
///
/// Completes the two-step transfer by moving pending_authority to authority.
///
/// # Arguments
/// * `config` - Mutable reference to the config account data
/// * `signer` - The pubkey of the signing pending authority
///
/// # Returns
/// * `Ok(())` if the transfer was completed
/// * `Err(ProgramError::IllegalOwner)` if signer is not the pending authority
/// * `Err(ProgramError::UninitializedAccount)` if no pending authority is set
#[inline]
pub fn accept_authority_impl<T: HasAuthority>(
    config: &mut T,
    signer: &Pubkey,
) -> Result<(), ProgramError> {
    // Verify pending authority is set
    if *config.pending_authority() == Pubkey::default() {
        return Err(ProgramError::UninitializedAccount);
    }

    // Verify signer is pending authority
    if config.pending_authority() != signer {
        return Err(ProgramError::IllegalOwner);
    }

    // Transfer authority role
    *config.authority_mut() = *config.pending_authority();
    *config.pending_authority_mut() = Pubkey::default();

    Ok(())
}
