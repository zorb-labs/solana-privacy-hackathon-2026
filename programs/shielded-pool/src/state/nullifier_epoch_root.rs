//! Nullifier Epoch Root Account for nullifier tree historical root storage.
//!
//! Each epoch boundary creates a new NullifierEpochRoot PDA that stores the
//! finalized tree root. This replaces the fixed ring buffer approach,
//! enabling flexible retention and rent reclamation.

use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

use crate::pda::find_nullifier_epoch_root_pda;
use crate::state::ShieldedPoolAccount;

/// Minimum number of nullifier epochs that must remain provable.
/// Prevents advancing `earliest_provable_epoch` too close to current epoch.
pub const MIN_PROVABLE_NULLIFIER_EPOCHS: u64 = 30;

/// Stores a finalized nullifier tree root for a specific epoch.
///
/// Created by `AdvanceNullifierEpoch` when finalizing an epoch boundary.
/// Can be closed via `CloseNullifierEpochRoot` after the epoch is no longer provable.
///
/// # PDA Seeds
/// `["nullifier_epoch_root", nullifier_epoch.to_le_bytes()]`
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][56-byte struct data]`
///
/// Total on-chain size: 64 bytes
#[account(ShieldedPoolAccount::NullifierEpochRoot)]
#[repr(C)]
pub struct NullifierEpochRoot {
    /// Finalized merkle root at epoch boundary
    pub root: [u8; 32],
    /// Nullifier epoch number this root corresponds to
    pub nullifier_epoch: u64,
    /// Tree index at finalization (last_finalized_index)
    pub finalized_index: u64,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment to 8 bytes
    pub _padding: [u8; 7],
}

impl NullifierEpochRoot {
    /// Verify that a nullifier epoch root account key matches the expected PDA
    pub fn verify_pda(program_id: &Pubkey, nullifier_epoch: u64, account_key: &Pubkey) -> bool {
        let _ = program_id; // Uses crate::ID internally
        let (expected_pda, _) = find_nullifier_epoch_root_pda(nullifier_epoch);
        expected_pda == *account_key
    }
}
