//! Nullifier indexed tree instructions.
//!
//! This module contains instructions for managing the indexed merkle tree
//! for nullifier storage:
//!
//! ## Tree Management
//! - `AdvanceNullifierEpoch`: Finalize current root into NullifierEpochRoot PDA
//! - `AdvanceEarliestProvableEpoch`: Move earliest provable epoch forward
//!
//! ## Insertion (Permissionless Crank)
//! - `NullifierBatchInsert`: Insert batch with ZK proof
//!
//! ## Cleanup
//! - `CloseInsertedNullifier`: Close a single nullifier PDA after tree insertion
//! - `CloseNullifierEpochRoot`: Close a single NullifierEpochRoot PDA after epoch is no longer provable
//!
//! ## Removed Instructions
//! - Discriminator 64: `InitNullifierTree` - now part of `Initialize`
//! - Discriminator 67: `SingleInsertNullifier` - replaced by `NullifierBatchInsert`

mod advance_earliest_provable_epoch;
mod advance_nullifier_epoch;
mod close_inserted_nullifier;
mod close_nullifier_epoch_root;
mod nullifier_batch_insert;

// Panchor Handlers
pub use advance_earliest_provable_epoch::AdvanceEarliestProvableEpochAccounts;
pub use advance_earliest_provable_epoch::AdvanceEarliestProvableEpochData;
pub use advance_earliest_provable_epoch::process_advance_earliest_provable_epoch;
pub use advance_nullifier_epoch::AdvanceNullifierEpochAccounts;
pub use advance_nullifier_epoch::process_advance_nullifier_epoch;
pub use close_inserted_nullifier::CloseInsertedNullifierAccounts;
pub use close_inserted_nullifier::process_close_inserted_nullifier;
pub use close_nullifier_epoch_root::CloseNullifierEpochRootAccounts;
pub use close_nullifier_epoch_root::process_close_nullifier_epoch_root;
pub use nullifier_batch_insert::NullifierBatchInsertAccounts;
pub use nullifier_batch_insert::process_nullifier_batch_insert;

// Re-export constants
pub use nullifier_batch_insert::MAX_NULLIFIER_BATCH_SIZE;
