//! Nullifier leaf inserted event definition.

use super::EventType;
use panchor::prelude::*;

/// Event emitted for each nullifier inserted during ZK batch insertion.
///
/// This event records the nullifier value, its tree position, and the epoch
/// when it was inserted. Indexers can reconstruct the full indexed merkle
/// tree structure by:
/// 1. Collecting all (nullifier, tree_index) pairs
/// 2. Sorting nullifiers by value
/// 3. Computing linked list pointers from the sorted order
///
/// The `inserted_epoch` field enables:
/// - Lifecycle tracking for nullifier PDA garbage collection
/// - Epoch-based queries for incremental sync
/// - Correlation with `NullifierBatchInsertedEvent` for verification
#[event(EventType::NullifierLeafInserted)]
#[repr(C)]
pub struct NullifierLeafInsertedEvent {
    /// The nullifier value (32 bytes, big-endian)
    pub nullifier: [u8; 32],
    /// Tree index where this leaf was inserted
    pub tree_index: u64,
    /// Epoch when this nullifier was inserted into the indexed tree
    pub inserted_epoch: u64,
}
