//! New receipt event definition.

use super::EventType;
use crate::instructions::types::{N_INS, N_OUTS, N_PUBLIC_LINES};
use alloc::vec::Vec;
use borsh::BorshSerialize;
use panchor::prelude::*;

/// Current receipt format version.
/// Increment this when the Receipt struct layout changes.
pub const RECEIPT_VERSION: u8 = 2;

/// Receipt data structure - leaf content for the receipt merkle tree.
///
/// Contains cryptographic essentials for ZK proof verification:
/// - Public input slots (asset IDs, amounts)
/// - Commitments (new UTXOs)
/// - Nullifiers (spent UTXOs)
///
/// Operational metadata (tx_type, fees, relayer info, recipients) has been
/// removed - these are tracked elsewhere or derivable from public_amounts.
///
/// This struct is Borsh-serialized and SHA256-hashed to create the merkle leaf.
/// The serialized bytes are also emitted in the NewReceiptEvent for indexer verification.
#[derive(BorshSerialize, Clone)]
pub struct Receipt {
    /// Receipt format version (allows future schema evolution)
    pub version: u8,
    /// Solana slot when the transaction was processed
    pub slot: u64,
    /// Solana epoch when the transaction was processed (~2-3 days on mainnet).
    /// Used for correlating with staking reward snapshots in unified SOL pool.
    pub epoch: u64,
    /// Commitment tree root after transaction (includes new output commitments)
    pub commitment_root: [u8; 32],
    /// Index of the last output commitment in the tree (next_index - 1)
    pub last_commitment_index: u64,
    /// Commitments created by this transaction
    pub commitments: [[u8; 32]; N_OUTS],
    /// Nullifiers consumed by this transaction
    pub nullifiers: [[u8; 32]; N_INS],
    /// Hash of transact params for verification
    pub transact_params_hash: [u8; 32],
    /// Public asset IDs (zero for unused slots)
    pub public_asset_ids: [[u8; 32]; N_PUBLIC_LINES],
    /// Public amounts per asset as field elements
    pub public_amounts: [[u8; 32]; N_PUBLIC_LINES],
}

impl Receipt {
    /// Compute the hash of this receipt to be used as a merkle leaf.
    /// Simply SHA256 hashes the Borsh-serialized struct.
    pub fn to_leaf_hash(&self) -> Result<[u8; 32], pinocchio::program_error::ProgramError> {
        use solana_program::hash::hash;

        let serialized = self.to_bytes()?;
        Ok(hash(&serialized).to_bytes())
    }

    /// Serialize the receipt to Borsh bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, pinocchio::program_error::ProgramError> {
        let mut serialized = Vec::new();
        self.serialize(&mut serialized)
            .map_err(|_| pinocchio::program_error::ProgramError::InvalidInstructionData)?;
        Ok(serialized)
    }
}

/// Event header for NewReceiptEvent.
///
/// This is the fixed-size header that precedes the serialized Receipt data.
/// The full event format is:
/// ```text
/// [discriminator: 8 bytes (u64 LE)]
/// [receipt_index: 8 bytes (u64 LE)]
/// [receipt_hash: 32 bytes]
/// [receipt_data: variable (Borsh-serialized Receipt)]
/// ```
///
/// Indexers can verify data integrity: `receipt_hash == sha256(receipt_data)`
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct NewReceiptEventHeader {
    /// Index of this receipt in the receipt merkle tree
    pub receipt_index: u64,
    /// The computed receipt leaf hash (SHA256 of Borsh-serialized Receipt)
    pub receipt_hash: [u8; 32],
}

/// Marker struct for NewReceiptEvent discriminator and event name.
///
/// The actual event data is built manually using `build_new_receipt_event_bytes`.
pub struct NewReceiptEvent;

impl panchor::Discriminator for NewReceiptEvent {
    const DISCRIMINATOR: u64 = EventType::NewReceipt as u64;
}

impl panchor::Event for NewReceiptEvent {
    fn name() -> &'static str {
        "NewReceipt"
    }
}

/// Build the complete NewReceiptEvent bytes for emission.
///
/// Format:
/// - discriminator (8 bytes): EventType::NewReceipt as u64 LE
/// - receipt_index (8 bytes): u64 LE
/// - receipt_hash (32 bytes): SHA256 of receipt_data
/// - receipt_data (variable): Borsh-serialized Receipt
///
/// Indexers verify: `receipt_hash == sha256(receipt_data)`
pub fn build_new_receipt_event_bytes(
    receipt_index: u64,
    receipt_hash: [u8; 32],
    receipt: &Receipt,
) -> Result<Vec<u8>, pinocchio::program_error::ProgramError> {
    let receipt_data = receipt.to_bytes()?;

    // Pre-allocate: discriminator(8) + receipt_index(8) + receipt_hash(32) + receipt_data
    let mut bytes = Vec::with_capacity(8 + 8 + 32 + receipt_data.len());

    // Discriminator
    bytes.extend_from_slice(&NewReceiptEvent::DISCRIMINATOR.to_le_bytes());
    // Receipt index
    bytes.extend_from_slice(&receipt_index.to_le_bytes());
    // Receipt hash
    bytes.extend_from_slice(&receipt_hash);
    // Serialized receipt data
    bytes.extend_from_slice(&receipt_data);

    Ok(bytes)
}
