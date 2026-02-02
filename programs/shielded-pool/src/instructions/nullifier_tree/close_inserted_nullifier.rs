//! Reclaim rent from a nullifier PDA after it has been inserted into the indexed tree.
//!
//! This instruction allows closing nullifier PDAs that are no longer needed for
//! proof verification. The authorization model uses a grace period:
//!
//! - **During grace period**: Only the nullifier's original authority can close
//! - **After grace period**: Anyone can close and claim rent as a GC incentive
//!
//! This creates an incentive structure where the authority has first right to
//! reclaim their own nullifiers, but third parties can clean up old state after
//! the grace period expires.

use crate::{
    errors::ShieldedPoolError,
    events::{NullifierPdaClosedEvent, emit_event},
    pda::gen_global_config_seeds,
    state::{CLEANUP_GRACE_EPOCHS, GlobalConfig, Nullifier, NullifierIndexedTree},
};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, instruction::Signer as CpiSigner};
use pinocchio_log::log;

// ============================================================================
// Accounts Struct
// ============================================================================

/// Accounts for ReclaimNullifier (formerly CloseInsertedNullifier) instruction.
///
/// Note: The instruction enum variant name is kept as CloseInsertedNullifier for
/// backwards compatibility (discriminator 66), but the functionality is renamed
/// to ReclaimNullifier to better describe its purpose.
#[derive(Accounts)]
pub struct CloseInsertedNullifierAccounts<'info> {
    /// The indexed tree account (for epoch checks)
    pub nullifier_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// The nullifier account to close
    #[account(mut)]
    pub nullifier_pda: AccountLoader<'info, Nullifier>,

    /// Where to send reclaimed rent
    #[account(mut)]
    pub destination: &'info AccountInfo,

    /// Signer for authorization.
    /// During grace period: must match Nullifier.authority
    /// After grace period: can be anyone (permissionless GC)
    pub authority: Signer<'info>,

    /// Global config PDA for event signing
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

// ============================================================================
// Handler
// ============================================================================

/// Reclaim rent from a nullifier PDA after it has been inserted into the indexed tree.
///
/// The nullifier must:
/// 1. Have been inserted into the indexed tree (inserted_epoch > 0)
/// 2. Be before earliest_provable_epoch (frozen in all provable roots)
///
/// Authorization:
/// - During grace period (current_epoch < earliest_provable_epoch + CLEANUP_GRACE_EPOCHS):
///   Only the nullifier's original authority can close
/// - After grace period: Anyone can close and claim rent as GC incentive
pub fn process_close_inserted_nullifier(
    ctx: Context<CloseInsertedNullifierAccounts>,
) -> ProgramResult {
    let CloseInsertedNullifierAccounts {
        nullifier_tree,
        nullifier_pda,
        destination,
        authority,
        global_config,
        shielded_pool_program,
    } = ctx.accounts;

    // Load nullifier tree to check epoch information
    let (current_epoch, earliest_provable_epoch) =
        nullifier_tree.map(|tree| (tree.current_epoch, tree.earliest_provable_epoch))?;

    // Get global config bump for event signing
    let global_config_bump = global_config.map(|config| config.bump)?;

    // Verify nullifier account state and authorization, capture inserted_epoch for event
    let inserted_epoch = nullifier_pda.try_map(|nullifier_account| {
        // Must be inserted (inserted_epoch != 0 sentinel)
        if nullifier_account.inserted_epoch == 0 {
            return Err(ShieldedPoolError::NullifierNotInserted.into());
        }

        // Must be before earliest_provable_epoch (frozen in all provable roots)
        if nullifier_account.inserted_epoch >= earliest_provable_epoch {
            return Err(ShieldedPoolError::NullifierStillProvable.into());
        }

        // Time-based authorization check
        // Grace period = earliest_provable_epoch + CLEANUP_GRACE_EPOCHS
        let grace_deadline = earliest_provable_epoch
            .checked_add(CLEANUP_GRACE_EPOCHS)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        if current_epoch < grace_deadline {
            // Within grace period: only nullifier authority can close
            if *authority.key() != nullifier_account.authority {
                log!("reclaim_nullifier: grace period active, authority mismatch");
                return Err(ShieldedPoolError::Unauthorized.into());
            }
        }
        // After grace period: anyone can close (permissionless GC)

        Ok(nullifier_account.inserted_epoch)
    })?;

    // Capture PDA key before closing
    let nullifier_pda_key = *nullifier_pda.key();

    // Transfer lamports to destination and close account
    let lamports = nullifier_pda.lamports();

    // Subtract from nullifier_pda
    unsafe {
        *nullifier_pda.borrow_mut_lamports_unchecked() = 0;
    }

    // Add to destination (with overflow check)
    unsafe {
        *destination.borrow_mut_lamports_unchecked() = destination
            .lamports()
            .checked_add(lamports)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
    }

    // Zero out the account data
    let mut data = nullifier_pda.try_borrow_mut_data()?;
    data.fill(0);

    // Resize to 0 bytes
    drop(data);
    nullifier_pda.resize(0)?;

    // Emit NullifierPdaClosedEvent
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);

    let event = NullifierPdaClosedEvent {
        nullifier_pda: nullifier_pda_key,
        inserted_epoch,
        reclaimed_lamports: lamports,
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        CpiSigner::from(&signer_seeds),
        &event,
    )?;

    Ok(())
}
