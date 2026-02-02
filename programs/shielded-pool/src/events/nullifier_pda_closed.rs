//! Nullifier closed event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when a nullifier PDA is closed and rent reclaimed.
///
/// This event provides an audit trail for garbage collection operations,
/// enabling indexers to track which nullifiers have been cleaned up.
///
/// # Closure Conditions
///
/// A nullifier can be closed when either:
/// 1. `inserted_epoch < earliest_provable_epoch` (no longer needed for proofs)
/// 2. Grace period expired and authority initiates closure
///
/// # Usage by Indexers
///
/// 1. Match `nullifier_pda` with tracked nullifier accounts
/// 2. Remove nullifier from active tracking
/// 3. Update GC metrics (reclaimed lamports, closure rate)
/// 4. Verify closure was authorized (check epoch conditions)
///
/// Note: The nullifier hash is not included because it cannot be recovered
/// from the PDA alone. Indexers should correlate via the PDA address which
/// they tracked from the original `NewNullifierEvent`.
#[event(EventType::NullifierPdaClosed)]
#[repr(C)]
pub struct NullifierPdaClosedEvent {
    /// The nullifier PDA that was closed
    pub nullifier_pda: Pubkey,
    /// The epoch when this nullifier was inserted into the indexed tree
    pub inserted_epoch: u64,
    /// Lamports reclaimed from the closed account
    pub reclaimed_lamports: u64,
}
