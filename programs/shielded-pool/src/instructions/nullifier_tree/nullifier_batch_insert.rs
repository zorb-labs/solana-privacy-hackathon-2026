//! Nullifier batch insertion into the indexed merkle tree.
//!
//! This instruction inserts a batch of pending nullifiers into the indexed tree
//! using a ZK proof that verifies the entire batch insertion is valid.
//!
//! # Index Semantics
//!
//! The tree's `next_index` starts at 1 after initialization because index 0 is
//! reserved for the genesis sentinel leaf. When inserting a batch:
//!
//! - `starting_index = tree.next_index` (first insertion slot, ≥ 1)
//! - Nullifiers must have `pending_index` values matching `[starting_index, starting_index + batch_size)`
//! - After insertion: `next_index += batch_size`
//!
//! # ZK Proof Verification
//!
//! The ZK proof verifies:
//! 1. For each nullifier: low_value < nullifier < low_next_value
//! 2. Low element merkle proofs are valid against old_root
//! 3. Low element updates are correct (pointers updated to new nullifier)
//! 4. New leaf appends are correct (inherit low's old pointers)
//! 5. Final root matches new_root after all insertions
//!
//! This is more efficient than single inserts for large batches because:
//! - The ZK proof verification is ~200k CU regardless of batch size
//! - No on-chain merkle proof verification per nullifier
//! - Subtrees are updated directly from the proof data

use crate::{
    errors::ShieldedPoolError,
    events::{NullifierBatchInsertedEvent, NullifierLeafInsertedEvent, emit_event},
    groth16::{CompressedGroth16Proof, Groth16Verifyingkey, verify_groth16},
    instructions::types::{NullifierBatchInsertData, NullifierBatchInsertProof},
    pda::gen_global_config_seeds,
    state::{GlobalConfig, NULLIFIER_TREE_HEIGHT, Nullifier, NullifierIndexedTree},
    verifying_keys::{
        N_PUBLIC_INPUTS_BATCH_4, N_PUBLIC_INPUTS_BATCH_16, N_PUBLIC_INPUTS_BATCH_64,
        NULLIFIER_BATCH_VK_4, NULLIFIER_BATCH_VK_16, NULLIFIER_BATCH_VK_64,
    },
};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, instruction::Signer as CpiSigner, program_error::ProgramError};
use pinocchio_log::log;

/// Maximum batch size for nullifier batch insertion.
/// This is determined by the circuit design and transaction size limits.
pub const MAX_NULLIFIER_BATCH_SIZE: u8 = 64;

// ============================================================================
// Accounts Struct
// ============================================================================

/// Accounts for NullifierBatchInsert instruction.
/// Note: Additional nullifier PDAs are passed via remaining accounts.
#[derive(Accounts)]
pub struct NullifierBatchInsertAccounts<'info> {
    /// The indexed tree account
    #[account(mut)]
    pub nullifier_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// Global config PDA for event signing
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
    // Remaining accounts: nullifier PDAs to insert (in pending_index order)
}

// ============================================================================
// Proof Verification
// ============================================================================

/// Verify the Groth16 proof for nullifier batch insertion.
///
/// # Arguments
/// * `proof` - The ZK proof data
/// * `nullifiers` - The nullifier values (public inputs)
/// * `starting_index` - The starting tree index for insertion
/// * `batch_size` - Number of nullifiers in the batch
///
/// # Returns
/// * `Ok(true)` if the proof is valid
/// * `Err(InvalidProof)` if verification fails
///
#[inline(never)]
fn verify_nullifier_batch_proof(
    proof: &NullifierBatchInsertProof,
    nullifiers: &[[u8; 32]],
    starting_index: u64,
    batch_size: u8,
) -> Result<bool, ProgramError> {
    // Select verification key based on batch size
    let vk: &Groth16Verifyingkey = match batch_size {
        1..=4 => &NULLIFIER_BATCH_VK_4,
        5..=16 => &NULLIFIER_BATCH_VK_16,
        17..=64 => &NULLIFIER_BATCH_VK_64,
        _ => return Err(ShieldedPoolError::InvalidBatchSize.into()),
    };

    // Create compressed proof for shared verification
    let compressed = CompressedGroth16Proof {
        proof_a: &proof.proof_a,
        proof_b: &proof.proof_b,
        proof_c: &proof.proof_c,
    };

    // starting_index as big-endian bytes
    let mut starting_index_bytes = [0u8; 32];
    starting_index_bytes[24..32].copy_from_slice(&starting_index.to_be_bytes());

    // Use match to handle different const generic sizes for public inputs
    match batch_size {
        1..=4 => {
            // Build public inputs: old_root, new_root, starting_index, nullifiers[0..4]
            let mut inputs: [[u8; 32]; N_PUBLIC_INPUTS_BATCH_4] =
                [[0u8; 32]; N_PUBLIC_INPUTS_BATCH_4];
            inputs[0] = proof.old_root;
            inputs[1] = proof.new_root;
            inputs[2] = starting_index_bytes;
            for i in 0..4 {
                if i < nullifiers.len() {
                    inputs[3 + i] = nullifiers[i];
                }
            }

            verify_groth16(&compressed, &inputs, vk)
                .map_err(|_| ShieldedPoolError::InvalidNullifierBatchInsertProof.into())
        }
        5..=16 => {
            // Build public inputs: old_root, new_root, starting_index, nullifiers[0..16]
            let mut inputs: [[u8; 32]; N_PUBLIC_INPUTS_BATCH_16] =
                [[0u8; 32]; N_PUBLIC_INPUTS_BATCH_16];
            inputs[0] = proof.old_root;
            inputs[1] = proof.new_root;
            inputs[2] = starting_index_bytes;
            for i in 0..16 {
                if i < nullifiers.len() {
                    inputs[3 + i] = nullifiers[i];
                }
            }

            verify_groth16(&compressed, &inputs, vk)
                .map_err(|_| ShieldedPoolError::InvalidNullifierBatchInsertProof.into())
        }
        17..=64 => {
            // Build public inputs: old_root, new_root, starting_index, nullifiers[0..64]
            let mut inputs: [[u8; 32]; N_PUBLIC_INPUTS_BATCH_64] =
                [[0u8; 32]; N_PUBLIC_INPUTS_BATCH_64];
            inputs[0] = proof.old_root;
            inputs[1] = proof.new_root;
            inputs[2] = starting_index_bytes;
            for i in 0..64 {
                if i < nullifiers.len() {
                    inputs[3 + i] = nullifiers[i];
                }
            }

            verify_groth16(&compressed, &inputs, vk)
                .map_err(|_| ShieldedPoolError::InvalidNullifierBatchInsertProof.into())
        }
        _ => Err(ShieldedPoolError::InvalidBatchSize.into()),
    }
}

// ============================================================================
// Handler
// ============================================================================

/// Process the NullifierBatchInsert instruction.
///
/// See [`NullifierBatchInsertData`] for the instruction data layout.
///
/// # Remaining Accounts
///
/// - nullifier_pdas: The nullifier PDAs to insert (in pending_index order)
pub fn process_nullifier_batch_insert(
    ctx: Context<NullifierBatchInsertAccounts>,
    data: &[u8],
) -> ProgramResult {
    // Parse instruction data into typed struct (zero-copy)
    let data = NullifierBatchInsertData::from_bytes(data)
        .ok_or(ProgramError::InvalidInstructionData)?;

    let batch_size = data.batch_size;
    if batch_size == 0 || batch_size > MAX_NULLIFIER_BATCH_SIZE {
        return Err(ShieldedPoolError::InvalidBatchSize.into());
    }

    // SECURITY: Reject batch sizes > 4 until trusted setup is complete for larger circuits.
    // NULLIFIER_BATCH_VK_16 and NULLIFIER_BATCH_VK_64 are placeholder all-zero keys.
    // Batch size 1-4 uses NULLIFIER_BATCH_VK_4 which has real verification key.
    // See verifying_keys.rs for VK status.
    if batch_size > 4 {
        log!(
            "Batch size {} not yet supported - VK pending trusted setup",
            batch_size
        );
        return Err(ShieldedPoolError::UnsupportedBatchSize.into());
    }

    let proof = data.proof;
    let nullifiers = data.nullifiers;

    let NullifierBatchInsertAccounts {
        nullifier_tree,
        global_config,
        shielded_pool_program,
    } = ctx.accounts;

    // Get global config bump for event signing
    let global_config_bump = global_config.map(|config| config.bump)?;

    // Get nullifier PDAs from remaining accounts
    let nullifier_pdas = ctx.remaining_accounts;
    if nullifier_pdas.len() < batch_size as usize {
        return Err(ShieldedPoolError::MissingAccounts.into());
    }
    let nullifier_pdas = &nullifier_pdas[..batch_size as usize];

    // Get tree values needed for validation
    let (starting_index, current_root, current_epoch) =
        nullifier_tree.map(|tree| (tree.next_index, tree.root, tree.current_epoch))?;

    // Verify old_root matches current tree root
    if current_root != proof.old_root {
        log!("Old root mismatch: proof.old_root does not match tree.root");
        return Err(ShieldedPoolError::InvalidOldRoot.into());
    }

    // Verify all nullifier PDAs:
    // 1. Have correct pending_index
    // 2. Are not already inserted
    // 3. Match the provided nullifier values (PDA verification)
    for (i, (nullifier_pda, nullifier_value)) in
        nullifier_pdas.iter().zip(nullifiers.iter()).enumerate()
    {
        // Verify PDA derivation matches the nullifier value
        if !Nullifier::verify_pda(&crate::ID, nullifier_value, nullifier_pda.key()) {
            log!(
                "Nullifier {} PDA mismatch: provided value doesn't match PDA",
                i
            );
            return Err(ShieldedPoolError::InvalidNullifierPda.into());
        }

        // Load and verify nullifier account using AccountLoader
        let expected_pending_index = starting_index + i as u64;
        AccountLoader::<Nullifier>::new(nullifier_pda)?.try_inspect(|nullifier_account| {
            // Verify pending_index matches expected order
            if nullifier_account.pending_index != expected_pending_index {
                log!(
                    "Invalid pending index for nullifier {}: expected {}, got {}",
                    i,
                    expected_pending_index,
                    nullifier_account.pending_index
                );
                return Err(ShieldedPoolError::InvalidPendingIndex.into());
            }

            // Verify not already inserted in a DIFFERENT epoch
            // Allow inserted_epoch == 0 (not yet inserted) or inserted_epoch == current_epoch (idempotent retry)
            if nullifier_account.inserted_epoch != 0
                && nullifier_account.inserted_epoch != current_epoch
            {
                log!(
                    "Nullifier {} already inserted in epoch {}, current epoch is {}",
                    i,
                    nullifier_account.inserted_epoch,
                    current_epoch
                );
                return Err(ShieldedPoolError::NullifierAlreadyInserted.into());
            }

            Ok(())
        })?;
    }

    // Verify Groth16 proof
    // AUDIT NOTE: starting_index is captured from tree.next_index BEFORE nullifier PDAs are
    // presented. This ensures the ZK proof commits to the correct insertion position regardless
    // of what nullifier accounts are passed in remaining_accounts.
    if !verify_nullifier_batch_proof(proof, nullifiers, starting_index, batch_size)? {
        log!("ZK batch insert: proof verification failed");
        return Err(ShieldedPoolError::InvalidProof.into());
    }

    // Mark all nullifiers as inserted with current epoch (idempotent)
    // If already set to current_epoch, skip (allows retry after partial failure)
    for nullifier_pda in nullifier_pdas.iter() {
        AccountLoader::<Nullifier>::new(nullifier_pda)?.inspect_mut(|nullifier_account| {
            if nullifier_account.inserted_epoch == 0 {
                nullifier_account.inserted_epoch = current_epoch;
            }
            // Already set to current_epoch is fine (idempotent retry)
        })?;
    }

    // Update tree state from proof
    nullifier_tree.try_inspect_mut(|tree| {
        // new_root is ZK-verified (public input to Groth16 verifier)
        tree.root = proof.new_root;
        tree.next_index = starting_index
            .checked_add(batch_size as u64)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        // Copy new subtrees from proof.
        // Note: new_subtrees is NOT a ZK public input — it is trusted from the prover
        // as an optimization cache for O(h) incremental appends. Correctness is maintained
        // because new_root IS ZK-verified, and future batch inserts re-verify old_root == tree.root.
        for i in 0..NULLIFIER_TREE_HEIGHT as usize {
            tree.subtrees[i] = proof.new_subtrees[i];
        }

        Ok(())
    })?;

    log!("ZK batch insert: {} nullifiers inserted", batch_size);

    // Emit events
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);

    // Emit per-nullifier leaf events (enables incremental tree reconstruction)
    // Indexers derive linked list pointers by sorting all nullifiers by value
    for (i, nullifier) in nullifiers.iter().enumerate() {
        let leaf_event = NullifierLeafInsertedEvent {
            nullifier: *nullifier,
            tree_index: starting_index + i as u64,
            inserted_epoch: current_epoch,
        };

        emit_event(
            global_config.account_info(),
            shielded_pool_program,
            CpiSigner::from(&signer_seeds),
            &leaf_event,
        )?;
    }

    // Emit batch summary event (enables root verification)
    let batch_event = NullifierBatchInsertedEvent {
        old_root: proof.old_root,
        new_root: proof.new_root,
        starting_index,
        inserted_epoch: current_epoch,
        batch_size,
        _padding: [0u8; 7],
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        CpiSigner::from(&signer_seeds),
        &batch_event,
    )?;

    Ok(())
}
