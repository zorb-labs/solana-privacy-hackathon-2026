//! Nullifier earliest epoch advanced event definition.

use super::EventType;
use panchor::prelude::*;

/// Event emitted when the nullifier tree's earliest provable epoch is advanced.
///
/// This event is emitted after a successful `advance_earliest_provable_epoch` instruction,
/// which updates the minimum epoch for which nullifier non-membership proofs are accepted.
/// This enables garbage collection of old epoch root accounts and nullifier PDAs.
///
/// # Usage by Indexers
///
/// 1. Nullifiers with `inserted_epoch < new_epoch` can be garbage collected
/// 2. Epoch root accounts with `epoch < new_epoch` can be closed
/// 3. Track the provable window: `[new_epoch, current_epoch]`
#[event(EventType::NullifierEarliestEpochAdvanced)]
#[repr(C)]
pub struct NullifierEarliestEpochAdvancedEvent {
    /// Previous earliest provable epoch
    pub old_epoch: u64,
    /// New earliest provable epoch
    pub new_epoch: u64,
}
