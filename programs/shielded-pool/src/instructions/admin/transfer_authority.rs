//! `TransferAuthority` instruction handler.
//!
//! Initiates a two-step authority transfer by setting `pending_authority`.
//! The new authority must call `accept_authority` to complete the transfer.

use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    msg,
    sysvars::{Sysvar, clock::Clock},
};

use crate::{
    errors::ShieldedPoolError,
    events::{AuthorityTransferInitiatedEvent, emit_event},
    pda::gen_global_config_seeds,
    state::GlobalConfig,
};

/// Accounts for the `TransferAuthority` instruction.
#[derive(Accounts)]
pub struct TransferAuthorityAccounts<'info> {
    /// Global config PDA ["global_config"]
    #[account(mut, owner = crate::ID)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
    /// Current authority (must be signer, must match global_config.authority)
    pub authority: Signer<'info>,
    /// New authority address (read-only)
    pub new_authority: &'info AccountInfo,
    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Process transfer authority instruction.
///
/// Sets the `pending_authority` field on the global config. The new authority
/// must call `accept_authority` to complete the transfer.
///
/// # Accounts
/// 0. `[writable]` - Global config PDA
/// 1. `[signer]` - Current authority
/// 2. `[]` - New authority address
/// 3. `[]` - Shielded pool program (for event emission)
pub fn process_transfer_authority(ctx: Context<TransferAuthorityAccounts>) -> ProgramResult {
    let TransferAuthorityAccounts {
        global_config,
        authority,
        new_authority,
        shielded_pool_program,
    } = ctx.accounts;

    // Get current slot for event
    let clock = Clock::get()?;

    // Update config and get bump for event emission
    let (current_authority, bump) = global_config.try_map_mut(|config| {
        // Verify signer is current authority
        if config.authority != *authority.key() {
            msg!("transfer_authority: unauthorized");
            return Err(ShieldedPoolError::Unauthorized.into());
        }

        let current = config.authority;

        // Set pending authority
        config.pending_authority = *new_authority.key();

        msg!("transfer_authority: pending authority set");
        Ok((current, config.bump))
    })?;

    // Emit event
    let bump_bytes = [bump];
    let seeds = gen_global_config_seeds(&bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    let event = AuthorityTransferInitiatedEvent {
        current_authority,
        pending_authority: *new_authority.key(),
        slot: clock.slot,
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        signer,
        &event,
    )?;

    Ok(())
}
