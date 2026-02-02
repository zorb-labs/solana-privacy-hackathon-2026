//! Advance the earliest provable epoch for the nullifier tree.

use crate::{
    errors::ShieldedPoolError,
    events::{NullifierEarliestEpochAdvancedEvent, emit_event},
    pda::gen_global_config_seeds,
    state::{GlobalConfig, MIN_PROVABLE_NULLIFIER_EPOCHS, NullifierIndexedTree},
};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, instruction::Signer as CpiSigner};

// ============================================================================
// Accounts Struct
// ============================================================================

/// Accounts for AdvanceEarliestProvableEpoch instruction.
#[derive(Accounts)]
pub struct AdvanceEarliestProvableEpochAccounts<'info> {
    /// The indexed tree account
    #[account(mut)]
    pub nullifier_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// Global config PDA for authority verification and event signing
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Must be global config authority
    pub authority: Signer<'info>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

// ============================================================================
// Data Structs
// ============================================================================

/// Instruction data for AdvanceEarliestProvableEpoch.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct AdvanceEarliestProvableEpochData {
    /// The new earliest provable epoch value
    pub new_earliest_epoch: u64,
}

// ============================================================================
// Handler
// ============================================================================

/// Advance the earliest provable epoch for the nullifier tree.
///
/// This allows old EpochRootAccount PDAs to be closed and their rent reclaimed.
/// The new value must be:
/// - >= current earliest_provable_epoch (no going back)
/// - <= current_epoch - MIN_PROVABLE_NULLIFIER_EPOCHS (maintain coverage invariant)
pub fn process_advance_earliest_provable_epoch(
    ctx: Context<AdvanceEarliestProvableEpochAccounts>,
    data: AdvanceEarliestProvableEpochData,
) -> ProgramResult {
    let accounts = ctx.accounts;

    // Get global config bump and verify authority
    let global_config_bump = accounts.global_config.map(|config| config.bump)?;
    accounts.global_config.try_inspect(|config| {
        if *accounts.authority.key() != config.authority {
            return Err(ShieldedPoolError::Unauthorized.into());
        }
        Ok(())
    })?;

    // Capture old_epoch and update tree
    let old_epoch = accounts.nullifier_tree.try_map_mut(|tree| {
        // Capture old value before update
        let old_epoch = tree.earliest_provable_epoch;

        // Verify new value is >= current (no going back)
        if data.new_earliest_epoch < old_epoch {
            return Err(ShieldedPoolError::InvalidEarliestEpoch.into());
        }

        // Verify current_epoch >= MIN_PROVABLE_NULLIFIER_EPOCHS to prevent underflow
        if tree.current_epoch < MIN_PROVABLE_NULLIFIER_EPOCHS {
            return Err(ShieldedPoolError::InvalidEarliestEpoch.into());
        }

        // Verify new value maintains coverage invariant
        // new_earliest <= current_epoch - MIN_PROVABLE_NULLIFIER_EPOCHS
        let max_allowed = tree.current_epoch - MIN_PROVABLE_NULLIFIER_EPOCHS;
        if data.new_earliest_epoch > max_allowed {
            return Err(ShieldedPoolError::InvalidEarliestEpoch.into());
        }

        // Update earliest_provable_epoch
        tree.earliest_provable_epoch = data.new_earliest_epoch;

        Ok(old_epoch)
    })?;

    // Emit event
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);
    let signer = CpiSigner::from(&signer_seeds);

    let event = NullifierEarliestEpochAdvancedEvent {
        old_epoch,
        new_epoch: data.new_earliest_epoch,
    };

    emit_event(
        accounts.global_config.account_info(),
        accounts.shielded_pool_program,
        signer,
        &event,
    )?;

    Ok(())
}
