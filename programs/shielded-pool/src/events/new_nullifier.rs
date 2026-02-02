//! New nullifier event definition.

use super::EventType;
use panchor::prelude::*;

/// Event emitted when a new nullifier is created (input note spent).
///
/// Nullifiers prevent double-spending by marking notes as consumed.
/// Each nullifier is a hash of the note's secret parameters that can only
/// be computed by the note owner.
///
/// The `pending_index` indicates the position this nullifier will occupy
/// in the indexed merkle tree once batch insertion occurs. Clients use this
/// to maintain a local sorted linked list of nullifiers for generating
/// non-membership proofs.
#[event(EventType::NewNullifier)]
#[repr(C)]
pub struct NewNullifierEvent {
    /// The nullifier hash (32 bytes, big-endian)
    pub nullifier: [u8; 32],
    /// The assigned pending index in the indexed tree.
    ///
    /// This is the index that will be used when the nullifier is batch-inserted
    /// into the indexed merkle tree. Clients should track this to maintain their
    /// local state for proof generation.
    pub pending_index: u64,
}
