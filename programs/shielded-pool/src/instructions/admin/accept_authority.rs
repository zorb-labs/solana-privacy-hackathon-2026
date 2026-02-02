//! `AcceptAuthority` instruction handler.
//!
//! Completes the two-step authority transfer by accepting the pending authority role.
//! Must be called by the `pending_authority` address.

use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{Sysvar, clock::Clock},
};

use crate::{
    events::{AuthorityTransferCompletedEvent, emit_event},
    pda::gen_global_config_seeds,
    state::GlobalConfig,
};

/// Accounts for the `AcceptAuthority` instruction.
#[derive(Accounts)]
pub struct AcceptAuthorityAccounts<'info> {
    /// Global config PDA ["global_config"]
    #[account(mut, owner = crate::ID)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
    /// Pending authority (must be signer, must match global_config.pending_authority)
    pub signer: Signer<'info>,
    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Process accept authority instruction.
///
/// Completes the two-step authority transfer. The signer must match the
/// `pending_authority` field on the global config.
///
/// # Accounts
/// 0. `[writable]` - Global config PDA
/// 1. `[signer]` - Pending authority (must match `global_config.pending_authority`)
/// 2. `[]` - Shielded pool program (for event emission)
pub fn process_accept_authority(ctx: Context<AcceptAuthorityAccounts>) -> ProgramResult {
    let AcceptAuthorityAccounts {
        global_config,
        signer,
        shielded_pool_program,
    } = ctx.accounts;

    // Get current slot for event
    let clock = Clock::get()?;

    // Update config and get data for event emission
    let (previous_authority, new_authority, bump) = global_config.try_map_mut(|config| {
        // AUDIT FIX (CRIT-02): Verify signer pubkey matches pending authority.
        // The Signer<'info> type already guarantees this account signed the transaction.
        // We only need to verify the pubkey matches the expected pending_authority.
        if *signer.key() != config.pending_authority {
            return Err(ProgramError::IllegalOwner);
        }

        // Verify pending authority is not default (zero)
        if config.pending_authority == Pubkey::default() {
            return Err(ProgramError::UninitializedAccount);
        }

        let previous = config.authority;
        let new = config.pending_authority;

        // Transfer authority role
        config.authority = config.pending_authority;
        config.pending_authority = Pubkey::default();

        Ok((previous, new, config.bump))
    })?;

    // Emit event
    let bump_bytes = [bump];
    let seeds = gen_global_config_seeds(&bump_bytes);
    let cpi_signer = PinocchioSigner::from(&seeds);

    let event = AuthorityTransferCompletedEvent {
        previous_authority,
        new_authority,
        slot: clock.slot,
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        cpi_signer,
        &event,
    )?;

    Ok(())
}
