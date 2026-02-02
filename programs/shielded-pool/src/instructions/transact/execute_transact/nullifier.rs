//! Nullifier verification and PDA management for execute_transact.
//!
//! This module handles nullifier-related operations that prevent double-spending:
//! - ZK proof verification that nullifiers are not in the finalized tree
//! - PDA creation for current-epoch nullifiers
//! - Event emission for indexers
//!
//! # Security Considerations
//! - Double-spend prevention relies on two mechanisms:
//!   1. ZK non-membership proof (finalized epochs)
//!   2. PDA existence check (current epoch)
//! - Nullifier PDAs are derived from nullifier hash, making them deterministic
//! - Event emission enables off-chain indexers to track nullifier state

use crate::{
    errors::ShieldedPoolError,
    events::NewNullifierEvent,
    groth16::{CompressedGroth16Proof, verify_groth16},
    instructions::types::{N_INS, NullifierNonMembershipProofData},
    pda::{NULLIFIER_SEED, find_nullifier_epoch_root_pda, find_nullifier_pda, gen_global_config_seeds},
    state::{Nullifier, NullifierEpochRoot, NullifierIndexedTree},
    verifying_keys::NULLIFIER_NON_MEMBERSHIP_VK,
};
use panchor::{SetDiscriminator, prelude::*};
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{Seed, Signer as CpiSigner},
    program_error::ProgramError,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_contrib::AccountAssertions;
use pinocchio_system::instructions::CreateAccount;

// ============================================================================
// Constants
// ============================================================================

/// Number of public inputs for nullifier non-membership proof.
/// Layout: [nullifier_root, nullifier_0, nullifier_1, nullifier_2, nullifier_3]
pub const N_NULLIFIER_NM_PUBLIC_INPUTS: usize = 1 + N_INS;

// ============================================================================
// Non-Membership Proof Verification
// ============================================================================

/// Verify nullifier non-membership ZK proof against the indexed merkle tree.
/// This proves all N_INS nullifiers are NOT present in the finalized nullifier tree
/// (past epochs). Current epoch nullifiers are checked via PDA existence.
///
/// # Security
/// - Validates root is either current or from a valid NullifierEpochRoot PDA
/// - Verifies Groth16 proof against the non-membership circuit
/// - Historical roots must be from epochs that are still provable
///
/// # Arguments
/// * `tree` - The nullifier indexed merkle tree account
/// * `epoch_root_pda` - Optional PDA for historical root validation
/// * `nullifiers` - The N_INS nullifier hashes from the transact proof
/// * `proof_data` - The Groth16 proof and nullifier root
///
/// # Returns
/// * `Ok(())` if the proof is valid and root is known
/// * `Err(UnknownNullifierRoot)` if the root is not in root_history
/// * `Err(InvalidNullifierNonMembershipProof)` if the ZK proof verification fails
#[inline(never)]
pub fn verify_nullifier_non_membership_proof(
    tree: &NullifierIndexedTree,
    epoch_root_pda: Option<&AccountInfo>,
    nullifiers: &[[u8; 32]; N_INS],
    proof_data: &NullifierNonMembershipProofData,
) -> Result<(), ProgramError> {
    // 1. Verify root is known (current or historical via NullifierEpochRoot PDA)
    if !tree.is_current_root(&proof_data.nullifier_root) {
        // Historical root - must validate against NullifierEpochRoot PDA
        let epoch_root_account = epoch_root_pda
            .ok_or(ProgramError::from(ShieldedPoolError::UnknownNullifierRoot))?;

        // Load and validate NullifierEpochRoot using AccountLoader
        AccountLoader::<NullifierEpochRoot>::new(epoch_root_account)
            .map_err(|_| ProgramError::from(ShieldedPoolError::InvalidNullifierEpochRootPda))?
            .try_inspect(|nullifier_epoch_root| {
                // Verify root matches
                if nullifier_epoch_root.root != proof_data.nullifier_root {
                    return Err(ShieldedPoolError::UnknownNullifierRoot.into());
                }

                // Verify nullifier epoch is still provable
                if nullifier_epoch_root.nullifier_epoch < tree.earliest_provable_epoch {
                    return Err(ShieldedPoolError::EpochTooOld.into());
                }

                // Verify PDA derivation
                let (expected_pda, _) = find_nullifier_epoch_root_pda(nullifier_epoch_root.nullifier_epoch);
                if *epoch_root_account.key() != expected_pda {
                    return Err(ShieldedPoolError::InvalidNullifierEpochRootPda.into());
                }

                Ok(())
            })?;
    }

    // 2. Build public inputs for verification
    // Public inputs order matches circuit declaration: [nullifier_tree_root, nullifiers[0..N]]
    // All values are in big-endian format for circuit compatibility
    let mut public_inputs: [[u8; 32]; N_NULLIFIER_NM_PUBLIC_INPUTS] =
        [[0u8; 32]; N_NULLIFIER_NM_PUBLIC_INPUTS];

    // First input: nullifier_root
    public_inputs[0] = proof_data.nullifier_root;

    // Remaining inputs: nullifiers[0..N_INS]
    for i in 0..N_INS {
        public_inputs[1 + i] = nullifiers[i];
    }

    // 3. Verify Groth16 proof
    let compressed = CompressedGroth16Proof {
        proof_a: &proof_data.proof_a,
        proof_b: &proof_data.proof_b,
        proof_c: &proof_data.proof_c,
    };

    let verified = verify_groth16(&compressed, &public_inputs, &NULLIFIER_NON_MEMBERSHIP_VK)
        .map_err(|_| ShieldedPoolError::InvalidNullifierNonMembershipProof)?;

    if !verified {
        return Err(ShieldedPoolError::InvalidNullifierNonMembershipProof.into());
    }

    Ok(())
}

// ============================================================================
// Nullifier PDA Creation
// ============================================================================

/// Verify nullifier is unused, create nullifier PDA, and emit event.
/// This helper is at module level to give the compiler better control over stack frames.
///
/// # Security
/// - Validates PDA derivation matches expected address
/// - Ensures account is uninitialized (prevents double-spend)
/// - Sets discriminator to mark account as Nullifier type
/// - Emits event for indexer tracking
#[inline(never)]
pub fn verify_and_create_nullifier<'a>(
    nullifier_account: &'a AccountInfo,
    payer: &'a AccountInfo,
    _system_program: &'a AccountInfo,
    nullifier_hash: &[u8; 32],
    commitment_tree_account: &'a AccountInfo,
    global_config_account: &'a AccountInfo,
    shielded_pool_program: &'a AccountInfo,
    global_config_bump: u8,
    pending_index: u64,
) -> Result<(), ProgramError> {
    let (expected_pda, bump) = find_nullifier_pda(nullifier_hash);
    nullifier_account.assert_key(&expected_pda)?;
    crate::validation::require_uninitialized(nullifier_account)
        .map_err(|_| ShieldedPoolError::NullifierAlreadyUsed)?;

    // Create nullifier account
    create_nullifier_pda(nullifier_account, payer, nullifier_hash, bump)?;

    // Set discriminator on the newly created account
    {
        let mut data = nullifier_account.try_borrow_mut_data()?;
        Nullifier::set_discriminator(&mut data);
    }

    // Initialize nullifier account fields
    // Note: inserted_epoch defaults to 0 (sentinel for "not yet inserted")
    AccountLoader::<Nullifier>::new(nullifier_account)?
        .inspect_mut(|data| {
            data.authority = *commitment_tree_account.key();
            data.pending_index = pending_index;
        })?;

    // Emit nullifier event using panchor EventBytes
    use panchor::prelude::EventBytes;
    let nullifier_event = NewNullifierEvent {
        nullifier: *nullifier_hash,
        pending_index,
    };
    let event_data = nullifier_event.to_event_bytes();

    // Emit via CPI using global_config as signer
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);
    let signer = CpiSigner::from(&signer_seeds);

    crate::utils::emit_cpi_log(
        &crate::ID,
        global_config_account,
        shielded_pool_program,
        &[signer],
        event_data,
    )?;

    Ok(())
}

/// Creates a nullifier PDA account using the system program (does not initialize discriminator)
///
/// # Security
/// - Uses PDA derivation with nullifier hash as seed
/// - Account is owned by shielded pool program
/// - Rent-exempt balance is calculated from Nullifier::INIT_SPACE
pub fn create_nullifier_pda(
    nullifier_account: &AccountInfo,
    payer: &AccountInfo,
    nullifier: &[u8; 32],
    bump: u8,
) -> ProgramResult {
    // Get rent sysvar
    let rent = Rent::get()?;

    let bump_slice = [bump];
    let seeds = [
        Seed::from(NULLIFIER_SEED),
        Seed::from(nullifier.as_ref()),
        Seed::from(&bump_slice),
    ];
    let signer = CpiSigner::from(&seeds);

    CreateAccount {
        from: payer,
        to: nullifier_account,
        lamports: rent.minimum_balance(Nullifier::INIT_SPACE),
        space: Nullifier::INIT_SPACE as u64,
        owner: &crate::ID,
    }
    .invoke_signed(&[signer])
}
