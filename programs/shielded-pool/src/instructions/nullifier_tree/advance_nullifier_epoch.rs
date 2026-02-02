//! Advance the nullifier tree epoch.

use crate::{
    errors::ShieldedPoolError,
    events::{NullifierEpochAdvancedEvent, emit_event},
    pda::{find_nullifier_epoch_root_pda, gen_nullifier_epoch_root_seeds, gen_global_config_seeds},
    state::{NullifierEpochRoot, GlobalConfig, NullifierIndexedTree, MIN_SLOTS_PER_NULLIFIER_EPOCH},
};
use panchor::{SetDiscriminator, prelude::*};
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as CpiSigner,
    sysvars::{Sysvar, rent::Rent, clock::Clock},
};
use pinocchio_system::instructions::CreateAccount;

// ============================================================================
// Accounts Struct
// ============================================================================

/// Accounts for AdvanceNullifierEpoch instruction.
///
/// Note: nullifier_epoch_root PDA seeds are dynamic (depend on current nullifier epoch read from
/// nullifier_tree at runtime), so we cannot use panchor's `init` attribute.
/// We use raw AccountInfo and manually create + initialize the account.
#[derive(Accounts)]
pub struct AdvanceNullifierEpochAccounts<'info> {
    /// The indexed tree account
    #[account(mut)]
    pub nullifier_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// The NullifierEpochRoot PDA to create ["nullifier_epoch_root", nullifier_epoch]
    /// Raw AccountInfo since we must skip validation for accounts being created
    #[account(mut)]
    pub nullifier_epoch_root: &'info AccountInfo,

    /// Global config PDA for event signing
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Pays for the new account
    #[account(mut)]
    pub payer: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

// ============================================================================
// Handler
// ============================================================================

/// Advance the nullifier tree epoch, creating a NullifierEpochRoot PDA.
///
/// This instruction creates a new NullifierEpochRoot PDA storing the current root,
/// creating a stable checkpoint for proof verification. After this, proofs can
/// verify against the finalized nullifier epoch root.
///
/// **Requirements**:
/// - At least `MIN_SLOTS_PER_NULLIFIER_EPOCH` slots have passed since the last epoch advance
///
/// Anyone can call this instruction (permissionless).
pub fn process_advance_nullifier_epoch(
    ctx: Context<AdvanceNullifierEpochAccounts>,
) -> ProgramResult {
    let AdvanceNullifierEpochAccounts {
        nullifier_tree,
        nullifier_epoch_root,
        global_config,
        payer,
        system_program,
        shielded_pool_program,
    } = ctx.accounts;

    // Get global config bump for event signing
    let global_config_bump = global_config.map(|config| config.bump)?;

    // Get current slot for time-based epoch advancement check
    let current_slot = Clock::get()?.slot;

    // Read values and update tree atomically
    let (current_nullifier_epoch, current_root, finalized_index, bump) =
        nullifier_tree.try_map_mut(|tree| {
            // Check if enough slots have passed since the last epoch advance.
            // This ensures epochs advance at a regular time-based interval.
            if current_slot < tree.last_epoch_slot.saturating_add(MIN_SLOTS_PER_NULLIFIER_EPOCH) {
                return Err(ShieldedPoolError::EpochAdvanceTooSoon.into());
            }

            // Get current nullifier epoch and root before updating
            let current_nullifier_epoch = tree.current_epoch;
            let current_root = tree.root;
            // finalized_index = next_index means indices [0, next_index) are included
            // On first epoch advance (from init state), this is 1, meaning index 0 (genesis) is included
            let finalized_index = tree.next_index;

            // Verify the nullifier_epoch_root matches expected derivation
            let (expected_pda, bump) = find_nullifier_epoch_root_pda(current_nullifier_epoch);
            if *nullifier_epoch_root.key() != expected_pda {
                return Err(ShieldedPoolError::InvalidNullifierEpochRootPda.into());
            }

            // Update nullifier epoch tracking
            tree.current_epoch = current_nullifier_epoch
                .checked_add(1)
                .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
            tree.last_finalized_index = finalized_index;
            tree.last_epoch_slot = current_slot;

            Ok((current_nullifier_epoch, current_root, finalized_index, bump))
        })?;

    // Get rent sysvar
    let rent = Rent::get()?;

    // Create and initialize the NullifierEpochRoot PDA
    let epoch_bytes = current_nullifier_epoch.to_le_bytes();
    let bump_bytes = [bump];
    let seeds = gen_nullifier_epoch_root_seeds(&epoch_bytes, &bump_bytes);
    let signer = CpiSigner::from(&seeds);

    CreateAccount {
        from: payer,
        to: nullifier_epoch_root,
        lamports: rent.minimum_balance(NullifierEpochRoot::INIT_SPACE),
        space: NullifierEpochRoot::INIT_SPACE as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[signer])?;

    // Set discriminator on the newly created account
    {
        let mut data = nullifier_epoch_root.try_borrow_mut_data()?;
        NullifierEpochRoot::set_discriminator(&mut data);
    }

    // Initialize nullifier_epoch_root fields
    AccountLoader::<NullifierEpochRoot>::new(nullifier_epoch_root)?
        .inspect_mut(|data| {
            data.root = current_root;
            data.nullifier_epoch = current_nullifier_epoch;
            data.finalized_index = finalized_index;
            data.bump = bump;
        })?;

    // Emit event
    let global_bump_bytes = [global_config_bump];
    let global_signer_seeds = gen_global_config_seeds(&global_bump_bytes);
    let global_signer = CpiSigner::from(&global_signer_seeds);

    let event = NullifierEpochAdvancedEvent {
        nullifier_epoch: current_nullifier_epoch,
        root: current_root,
        finalized_index,
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        global_signer,
        &event,
    )?;

    Ok(())
}
