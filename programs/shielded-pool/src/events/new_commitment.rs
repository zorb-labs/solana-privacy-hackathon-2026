//! New commitment event definition.
//!
//! # Event Format
//!
//! This event uses a hybrid encoding for variable-length data:
//! - Fixed-size header (Pod-compatible, zero-copy accessible)
//! - Variable-length body (encrypted output data)
//!
//! Wire format: `[discriminator: 8 bytes][header: 48 bytes][encrypted_output: variable]`
//!
//! This approach avoids wasting space on fixed-size buffers while maintaining
//! zero-copy access to the header fields for efficient indexing.

use super::EventType;
use alloc::vec::Vec;
use borsh::{BorshDeserialize, BorshSerialize};
use bytemuck::{Pod, Zeroable};

/// Size of the fixed header portion of NewCommitmentEvent.
/// Layout: index (8) + commitment (32) + encrypted_output_len (4) + _padding (4) = 48 bytes
pub const NEW_COMMITMENT_HEADER_SIZE: usize = 48;

/// Data for a commitment in the merkle tree.
///
/// Used for serializing/deserializing commitment data in session accounts.
/// This version uses Vec<u8> for variable-length encrypted outputs.
#[derive(BorshDeserialize, BorshSerialize, Clone)]
pub struct CommitmentData {
    /// Index of this commitment in the tree
    pub index: u64,
    /// The commitment hash (32 bytes, big-endian)
    pub commitment: [u8; 32],
    /// Encrypted output data (variable length)
    pub encrypted_output: Vec<u8>,
}

/// Fixed-size header for NewCommitmentEvent.
///
/// This header is Pod-compatible for zero-copy deserialization.
/// The variable-length `encrypted_output` data follows immediately after.
///
/// # Wire Format
/// ```text
/// [discriminator: 8 bytes][header: 48 bytes][encrypted_output: variable]
///                         ^^^^^^^^^^^^^^^^
///                         This struct
/// ```
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct NewCommitmentHeader {
    /// Index of this commitment in the tree
    pub index: u64,
    /// The commitment hash (32 bytes, big-endian)
    pub commitment: [u8; 32],
    /// Length of the encrypted output data that follows
    pub encrypted_output_len: u32,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 4],
}

impl NewCommitmentHeader {
    /// Event discriminator (EventType::NewCommitment = 1)
    pub const DISCRIMINATOR: u64 = EventType::NewCommitment as u64;
}

// Manual trait implementations for compatibility with panchor's event system.
// We can't use `#[event]` macro because encrypted_output is variable-length,
// but we implement the core traits for consistency with other events.

impl panchor::Discriminator for NewCommitmentHeader {
    const DISCRIMINATOR: u64 = EventType::NewCommitment as u64;
}

impl panchor::Event for NewCommitmentHeader {
    fn name() -> &'static str {
        "NewCommitment"
    }
}

/// Event emitted when a new commitment is added to the commitment merkle tree.
///
/// Commitments represent shielded notes in the pool. Each commitment is a
/// Poseidon hash of the note's parameters (amount, asset, owner, etc.).
///
/// # Hybrid Encoding
///
/// Uses a fixed-size header + variable-length body to efficiently handle
/// encrypted outputs of any size without wasting space on fixed buffers.
///
/// Wire format: `[discriminator: 8 bytes][header: 48 bytes][encrypted_output: variable]`
///
/// # Parsing
///
/// ```ignore
/// let discriminator = u64::from_le_bytes(data[0..8]);
/// let header: &NewCommitmentHeader = bytemuck::from_bytes(&data[8..56]);
/// let encrypted_output = &data[56..56 + header.encrypted_output_len as usize];
/// ```
pub struct NewCommitmentEvent<'a> {
    /// Fixed header containing index, commitment, and length
    pub header: NewCommitmentHeader,
    /// Variable-length encrypted output data (borrowed)
    pub encrypted_output: &'a [u8],
}

impl<'a> NewCommitmentEvent<'a> {
    /// Create a new commitment event.
    pub fn new(index: u64, commitment: [u8; 32], encrypted_output: &'a [u8]) -> Self {
        Self {
            header: NewCommitmentHeader {
                index,
                commitment,
                encrypted_output_len: encrypted_output.len() as u32,
                _padding: [0u8; 4],
            },
            encrypted_output,
        }
    }

    /// Serialize the event to bytes with discriminator prepended.
    ///
    /// Returns: `[discriminator: 8 bytes][header: 48 bytes][encrypted_output: variable]`
    pub fn to_event_bytes(&self) -> Vec<u8> {
        let total_size = 8 + NEW_COMMITMENT_HEADER_SIZE + self.encrypted_output.len();
        let mut bytes = Vec::with_capacity(total_size);

        // Discriminator (8 bytes, little-endian)
        bytes.extend_from_slice(&NewCommitmentHeader::DISCRIMINATOR.to_le_bytes());

        // Header (48 bytes, Pod serialization)
        bytes.extend_from_slice(bytemuck::bytes_of(&self.header));

        // Variable-length body
        bytes.extend_from_slice(self.encrypted_output);

        bytes
    }

    /// Get the commitment index.
    pub fn index(&self) -> u64 {
        self.header.index
    }

    /// Get the commitment hash.
    pub fn commitment(&self) -> [u8; 32] {
        self.header.commitment
    }
}

/// Parse a NewCommitmentEvent from raw event bytes.
///
/// # Arguments
/// * `data` - Raw event bytes including discriminator
///
/// # Returns
/// * `Ok((header, encrypted_output))` - Parsed header and encrypted output slice
/// * `Err(())` - If data is too short or discriminator doesn't match
pub fn parse_new_commitment_event(data: &[u8]) -> Result<(NewCommitmentHeader, &[u8]), ()> {
    const MIN_SIZE: usize = 8 + NEW_COMMITMENT_HEADER_SIZE;

    if data.len() < MIN_SIZE {
        return Err(());
    }

    // Check discriminator
    let discriminator = u64::from_le_bytes(data[0..8].try_into().unwrap());
    if discriminator != NewCommitmentHeader::DISCRIMINATOR {
        return Err(());
    }

    // Parse header (zero-copy)
    let header: &NewCommitmentHeader =
        bytemuck::from_bytes(&data[8..8 + NEW_COMMITMENT_HEADER_SIZE]);

    // Extract encrypted output
    let encrypted_output_end = MIN_SIZE + header.encrypted_output_len as usize;
    if data.len() < encrypted_output_end {
        return Err(());
    }
    let encrypted_output = &data[MIN_SIZE..encrypted_output_end];

    Ok((*header, encrypted_output))
}
