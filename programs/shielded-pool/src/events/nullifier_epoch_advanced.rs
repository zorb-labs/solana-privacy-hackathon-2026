//! Nullifier epoch advanced event definition.

use super::EventType;
use panchor::prelude::*;

/// Event emitted when the nullifier epoch is advanced.
///
/// This event is emitted after a successful `advance_nullifier_epoch` instruction,
/// which creates a `NullifierEpochRoot` PDA storing a snapshot of the tree root at this epoch.
/// The nullifier epoch root is used for generating and verifying non-membership proofs.
///
/// # Usage by Indexers
///
/// 1. Record the nullifier epoch root snapshot for proof verification
/// 2. Track `finalized_index` to know which nullifiers are included in this epoch
/// 3. Use for garbage collection: nullifiers with `pending_index < finalized_index`
///    can be considered finalized
#[event(EventType::NullifierEpochAdvanced)]
#[repr(C)]
pub struct NullifierEpochAdvancedEvent {
    /// The nullifier epoch that was finalized
    pub nullifier_epoch: u64,
    /// The tree root at this nullifier epoch
    pub root: [u8; 32],
    /// Last tree index included in this nullifier epoch
    pub finalized_index: u64,
}
