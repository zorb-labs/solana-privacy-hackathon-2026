use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

use crate::pda::find_nullifier_pda;
use crate::state::ShieldedPoolAccount;

/// Nullifier account for tracking spent notes.
///
/// Each nullifier PDA represents a spent note and gets a sequential `pending_index`
/// assigned from `NullifierIndexedTree.next_pending_index`. This index determines
/// the order in which nullifiers are inserted into the indexed tree via NullifierBatchInsert.
///
/// # Index Semantics
///
/// The `pending_index` starts at 1 (not 0) because index 0 is reserved for the
/// genesis sentinel leaf in the indexed merkle tree. The first real nullifier
/// created by `ExecuteTransact` receives `pending_index = 1`.
///
/// # Lifecycle
///
/// 1. **Created** by `ExecuteTransact`: `pending_index` assigned, `inserted_epoch = 0`
/// 2. **Inserted** by `NullifierBatchInsert`: `inserted_epoch` set to current epoch
/// 3. **Closable** when `inserted_epoch < earliest_provable_epoch`
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][56-byte struct data]`
///
/// Total on-chain size: 64 bytes
#[account(ShieldedPoolAccount::Nullifier)]
#[repr(C)]
pub struct Nullifier {
    /// Authority that created this nullifier (typically the commitment tree PDA)
    pub authority: Pubkey,
    /// Sequential index for ordered tree insertion.
    ///
    /// Assigned from `NullifierIndexedTree.next_pending_index` during `ExecuteTransact`.
    /// Starts at 1 because index 0 is reserved for the genesis sentinel leaf.
    pub pending_index: u64,
    /// Epoch when this nullifier was inserted into the indexed tree.
    /// - `0` means "not yet inserted" (pending)
    /// - Values >= 1 indicate the actual epoch when inserted
    /// Note: Epochs start at 1 (not 0) so that 0 can be the uninitialized sentinel.
    /// Used to verify the nullifier has been frozen in all provable epoch roots before closure.
    pub inserted_epoch: u64,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 7],
}

impl Nullifier {
    /// Verify that a nullifier account key matches the expected PDA
    pub fn verify_pda(program_id: &Pubkey, nullifier: &[u8; 32], account_key: &Pubkey) -> bool {
        let (expected_pda, _) = find_nullifier_pda(nullifier);
        // Note: find_nullifier_pda uses the program ID from crate::ID
        let _ = program_id; // Kept for API compatibility
        expected_pda == *account_key
    }
}
