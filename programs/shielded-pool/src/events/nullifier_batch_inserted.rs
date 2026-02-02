//! Nullifier batch inserted event definition.

use super::EventType;
use panchor::prelude::*;

/// Event emitted when nullifiers are batch inserted into the indexed merkle tree.
///
/// This event is emitted after a successful `nullifier_batch_insert` instruction,
/// which verifies a ZK proof that the batch insertion is valid. The event enables
/// indexers to verify their simulated tree state matches the on-chain state.
///
/// # Usage by Indexers
///
/// 1. Match nullifiers by `pending_index` range: `[starting_index, starting_index + batch_size)`
/// 2. Verify simulated `new_root` matches the event's `new_root`
/// 3. Update `inserted_epoch` on matched nullifiers for garbage collection tracking
#[event(EventType::NullifierBatchInserted)]
#[repr(C)]
pub struct NullifierBatchInsertedEvent {
    /// Tree root before insertions
    pub old_root: [u8; 32],
    /// Tree root after all insertions
    pub new_root: [u8; 32],
    /// First tree index in this batch
    pub starting_index: u64,
    /// Epoch when inserted
    pub inserted_epoch: u64,
    /// Number of nullifiers inserted (1-64)
    pub batch_size: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}
