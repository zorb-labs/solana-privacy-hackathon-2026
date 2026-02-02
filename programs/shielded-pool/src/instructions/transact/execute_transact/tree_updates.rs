//! Tree state update helpers for execute_transact.
//!
//! This module handles Merkle tree state transitions:
//! - Appending commitments to the commitment tree
//! - Computing and appending receipt hashes to the receipt tree
//! - Emitting events for off-chain indexers
//!
//! # Security Considerations
//! - Commitment tree integrity is critical for proving note existence
//! - Receipt tree provides transaction audit trail
//! - Events must match on-chain state for indexer consistency

use crate::{
    CommitmentMerkleTree,
    events::{NewCommitmentEvent, Receipt, RECEIPT_VERSION, build_new_receipt_event_bytes},
    instructions::types::TransactProofData,
    merkle_tree::MerkleTree,
    pda::gen_global_config_seeds,
};
use light_hasher::Poseidon;
use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer as CpiSigner,
    program_error::ProgramError,
    pubkey::Pubkey,
};

// ============================================================================
// Commitment Tree Updates
// ============================================================================

/// Append a commitment to the commitment tree and emit event.
/// This helper is at module level to give the compiler better control over stack frames.
///
/// # Security
/// - Commitment is appended to Merkle tree using Poseidon hasher
/// - Event contains encrypted output for wallet synchronization
/// - Returns the index of the appended commitment
///
/// # Arguments
/// * `program_id` - This program's ID
/// * `commitment_tree` - Mutable reference to the commitment Merkle tree
/// * `commitment` - The 32-byte commitment hash to append
/// * `encrypted_output` - Encrypted note data for wallet sync
/// * `global_config_account` - Global config PDA for CPI signing
/// * `shielded_pool_program` - The shielded pool program account (required for self-CPI)
/// * `global_config_bump` - Bump seed for global config PDA
#[inline(never)]
pub fn append_commitment<'a>(
    program_id: &Pubkey,
    commitment_tree: &mut CommitmentMerkleTree,
    commitment: [u8; 32],
    encrypted_output: &[u8],
    global_config_account: &'a AccountInfo,
    shielded_pool_program: &'a AccountInfo,
    global_config_bump: u8,
) -> Result<u64, ProgramError> {
    let index = commitment_tree.next_index;
    MerkleTree::append::<Poseidon>(commitment, commitment_tree)?;

    // Emit commitment event (hybrid header + variable body encoding)
    let event = NewCommitmentEvent::new(index, commitment, encrypted_output);
    let event_data = event.to_event_bytes();

    // Emit via CPI using global_config as signer
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);
    let signer = CpiSigner::from(&signer_seeds);
    crate::utils::emit_cpi_log(
        program_id,
        global_config_account,
        shielded_pool_program,
        &[signer],
        event_data,
    )?;

    Ok(index)
}

// ============================================================================
// Receipt Computation and Emission
// ============================================================================

/// Compute receipt and its hash from components. Separating this reduces stack pressure in main function.
///
/// Returns both the Receipt struct and its hash. The Receipt is needed for event emission
/// (we emit the serialized Receipt bytes so indexers can verify receipt_hash == sha256(receipt_data)).
///
/// # Security
/// - Receipt hash is SHA256 of Borsh-serialized receipt data
/// - Ensures consistency between emitted data and stored hash
#[inline(never)]
pub fn compute_receipt_and_hash(
    slot: u64,
    epoch: u64,
    commitment_root: [u8; 32],
    last_commitment_index: u64,
    proof: &TransactProofData,
    transact_params_hash: [u8; 32],
) -> Result<(Receipt, [u8; 32]), ProgramError> {
    let receipt = Receipt {
        version: RECEIPT_VERSION,
        slot,
        epoch,
        commitment_root,
        last_commitment_index,
        commitments: proof.commitments,
        nullifiers: proof.nullifiers,
        transact_params_hash,
        public_asset_ids: proof.public_asset_ids,
        public_amounts: proof.public_amounts,
    };

    let hash = receipt.to_leaf_hash()?;

    Ok((receipt, hash))
}

/// Emit the receipt event. Separating this reduces stack pressure in main function.
///
/// Emits the NewReceiptEvent containing:
/// - receipt_index: position in the receipt merkle tree
/// - receipt_hash: SHA256 of the Borsh-serialized Receipt
/// - receipt_data: the full Borsh-serialized Receipt for indexer verification
///
/// # Security
/// - Receipt data is fully serialized for indexer verification
/// - Event emission uses CPI with global_config as signer
#[inline(never)]
pub fn emit_receipt_event<'a>(
    program_id: &Pubkey,
    receipt_index: u64,
    receipt: &Receipt,
    receipt_hash: [u8; 32],
    global_config_account: &'a AccountInfo,
    shielded_pool_program: &'a AccountInfo,
    global_config_bump: u8,
) -> Result<(), ProgramError> {
    // Build receipt event bytes: discriminator + receipt_index + receipt_hash + receipt_data
    let receipt_event_data = build_new_receipt_event_bytes(receipt_index, receipt_hash, receipt)?;

    // Emit via CPI using global_config as signer
    let bump_bytes = [global_config_bump];
    let signer_seeds = gen_global_config_seeds(&bump_bytes);
    let signer = CpiSigner::from(&signer_seeds);
    crate::utils::emit_cpi_log(
        program_id,
        global_config_account,
        shielded_pool_program,
        &[signer],
        receipt_event_data,
    )?;

    Ok(())
}
