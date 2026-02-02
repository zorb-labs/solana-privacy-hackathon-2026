//! Nullifier epoch root closed event definition.

use super::EventType;
use panchor::prelude::*;

/// Event emitted when a nullifier epoch root PDA is closed and rent reclaimed.
///
/// This event provides an audit trail for nullifier epoch cleanup operations,
/// enabling indexers to track which epochs have been garbage collected.
///
/// # Closure Conditions
///
/// A nullifier epoch root can be closed when:
/// - `nullifier_epoch < earliest_provable_epoch` (no longer needed for proof verification)
///
/// # Usage by Indexers
///
/// 1. Remove nullifier epoch from provable history tracking
/// 2. Update GC metrics (reclaimed lamports, epochs cleaned)
/// 3. Verify no proofs will reference this epoch going forward
#[event(EventType::NullifierEpochRootClosed)]
#[repr(C)]
pub struct NullifierEpochRootClosedEvent {
    /// The nullifier epoch number that was closed
    pub nullifier_epoch: u64,
    /// Lamports reclaimed from the closed account
    pub reclaimed_lamports: u64,
}
