//! Close a NullifierEpochRoot PDA after the nullifier epoch is no longer provable.
//!
//! # Safety Guarantees
//!
//! This instruction is inherently safe and cannot cause accidental harm because:
//!
//! 1. **Epoch is read from the account** - The nullifier epoch value is stored in the
//!    NullifierEpochRoot account itself, not passed as a parameter. You cannot specify
//!    the "wrong" epoch.
//!
//! 2. **Provability check is enforced** - The handler verifies that
//!    `nullifier_epoch_root.nullifier_epoch < earliest_provable_epoch`. If the epoch is
//!    still needed for ZK proof verification, the instruction fails with `EpochStillProvable`.
//!
//! 3. **PDA derivation is verified** - The handler confirms the account key matches
//!    the expected PDA derivation for the epoch. You cannot pass an arbitrary account.
//!
//! 4. **No system harm** - By definition, epochs older than `earliest_provable_epoch`
//!    are no longer valid for proof verification. Closing them reclaims rent without
//!    affecting system integrity.
//!
//! No confirmation parameter is needed because the existing checks guarantee that
//! only non-provable epochs can be closed.

use crate::{
    errors::ShieldedPoolError,
    events::{NullifierEpochRootClosedEvent, emit_event},
    pda::{find_nullifier_epoch_root_pda, gen_global_config_seeds},
    state::{NullifierEpochRoot, GlobalConfig, NullifierIndexedTree},
};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, instruction::Signer as CpiSigner};

// ============================================================================
// Accounts Struct
// ============================================================================

/// Accounts for CloseNullifierEpochRoot instruction.
#[derive(Accounts)]
pub struct CloseNullifierEpochRootAccounts<'info> {
    /// The indexed tree account (for earliest_provable_epoch check)
    pub nullifier_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// The nullifier epoch root account to close
    #[account(mut)]
    pub nullifier_epoch_root_pda: AccountLoader<'info, NullifierEpochRoot>,

    /// Where to send reclaimed rent
    #[account(mut)]
    pub destination: &'info AccountInfo,

    /// Global config PDA for authority verification and event signing
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Must be global config authority
    pub authority: Signer<'info>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

// ============================================================================
// Handler
// ============================================================================

/// Close a NullifierEpochRoot PDA after the nullifier epoch is no longer provable.
///
/// Reclaims the rent from the nullifier epoch root account. The epoch must be older than
/// `earliest_provable_epoch` to be closable.
pub fn process_close_nullifier_epoch_root(ctx: Context<CloseNullifierEpochRootAccounts>) -> ProgramResult {
    let CloseNullifierEpochRootAccounts {
        nullifier_tree,
        nullifier_epoch_root_pda,
        destination,
        global_config,
        authority,
        shielded_pool_program,
    } = ctx.accounts;

    // Verify authority matches global config and get bump for event signing
    let global_config_bump = global_config.try_map(|config| {
        if *authority.key() != config.authority {
            return Err(ShieldedPoolError::Unauthorized.into());
        }
        Ok(config.bump)
    })?;

    // Load nullifier tree to check earliest_provable_epoch
    let earliest_provable_epoch = nullifier_tree.map(|tree| tree.earliest_provable_epoch)?;

    // Verify nullifier epoch root account is closable and capture epoch for event
    let nullifier_epoch = nullifier_epoch_root_pda.try_map(|epoch_root| {
        // Verify the epoch is no longer provable
        if epoch_root.nullifier_epoch >= earliest_provable_epoch {
            return Err(ShieldedPoolError::EpochStillProvable.into());
        }

        // Verify PDA derivation matches
        let (expected_pda, _) = find_nullifier_epoch_root_pda(epoch_root.nullifier_epoch);
        if *nullifier_epoch_root_pda.key() != expected_pda {
            return Err(ShieldedPoolError::InvalidNullifierEpochRootPda.into());
        }

        Ok(epoch_root.nullifier_epoch)
    })?;

    // Transfer lamports to destination and close account
    let lamports = nullifier_epoch_root_pda.lamports();

    // Subtract from nullifier_epoch_root_pda
    unsafe {
        *nullifier_epoch_root_pda.borrow_mut_lamports_unchecked() = 0;
    }

    // Add to destination (with overflow check)
    unsafe {
        *destination.borrow_mut_lamports_unchecked() = destination
            .lamports()
            .checked_add(lamports)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
    }

    // Zero out the account data
    let mut data = nullifier_epoch_root_pda.try_borrow_mut_data()?;
    data.fill(0);

    // Resize to 0 bytes
    drop(data);
    nullifier_epoch_root_pda.resize(0)?;

    // Emit NullifierEpochRootClosedEvent
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);

    let event = NullifierEpochRootClosedEvent {
        nullifier_epoch,
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
