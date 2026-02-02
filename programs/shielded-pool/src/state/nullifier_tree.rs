//! Indexed nullifier tree for efficient non-membership proofs.
//!
//! This module implements an indexed merkle tree following Aztec's design.
//! The tree stores leaves as a sorted linked list: `(value, next_value, next_index)`.
//!
//! # Key Features
//!
//! - Non-membership proofs via "low nullifier" range checks
//! - Epoch-based root history via EpochRootAccount PDAs (see epoch_root.rs)
//! - Hybrid approach: PDAs for instant double-spend prevention, tree for ZK proofs
//!
//! Historical roots are stored in separate EpochRootAccount PDAs created by
//! AdvanceNullifierEpoch. The tree tracks `earliest_provable_epoch` to determine
//! which epochs are valid for ZK proof verification.
//!
//! # Epoch Lifecycle
//!
//! The nullifier tree uses epochs to batch nullifier insertions for efficient
//! ZK proof verification. Each epoch has a snapshot root that can be used for
//! proofs for a configurable window of time.
//!
//! ## Lifecycle Stages
//!
//! ```text
//! 1. Transaction (ExecuteTransact)
//!    ├── Creates Nullifier PDA with pending_index, inserted_epoch = 0 (sentinel for "not inserted")
//!    └── Updates next_pending_index on tree
//!
//! 2. Insert (NullifierBatchInsert / SingleInsert)
//!    ├── Updates tree root with batch of nullifiers
//!    ├── Sets inserted_epoch on each Nullifier PDA
//!    └── Advances next_index to reflect insertions
//!
//! 3. Advance Epoch (AdvanceNullifierEpoch)
//!    ├── Creates EpochRoot PDA with current root snapshot
//!    ├── Records last_finalized_index for this epoch
//!    └── Increments current_epoch
//!
//! 4. Advance Earliest Provable (AdvanceEarliestProvableEpoch)
//!    ├── Moves earliest_provable_epoch forward
//!    └── Makes older epochs non-provable (safe to close)
//!
//! 5. Cleanup (CloseEpochRoot / ReclaimNullifier)
//!    ├── CloseEpochRoot: Reclaims rent from non-provable epochs
//!    └── ReclaimNullifier: Reclaims rent from inserted nullifiers
//! ```
//!
//! # Key Invariants
//!
//! ## I1: Pending Contract
//!
//! Every Nullifier PDA with `pending_index` in range `[next_index, next_pending_index)`
//! MUST eventually be inserted via NullifierBatchInsert. The relayer is responsible for
//! ensuring all pending nullifiers are inserted before the epoch advances.
//!
//! ## I2: Provable Window
//!
//! For all epochs E where `earliest_provable_epoch <= E < current_epoch`:
//! - An EpochRoot PDA exists with the root snapshot at epoch E
//! - All nullifiers inserted before E are reflected in the epoch E root
//! - Proofs against epoch E roots are valid
//!
//! ## I3: Coverage Guarantee
//!
//! If `earliest_provable_epoch = E`, then for any nullifier N:
//! - If N was inserted at epoch < E: N is frozen in ALL provable roots
//! - If N was inserted at epoch >= E: N may not yet be in all provable roots
//!
//! This means nullifiers can only be safely closed when:
//! `inserted_epoch < earliest_provable_epoch`
//!
//! # Field Relationships
//!
//! ```text
//! next_index           ← Confirmed insertions (tree state)
//! next_pending_index   ← Assigned but not yet inserted
//! current_epoch        ← Active epoch for new transactions
//! earliest_provable_epoch ← Oldest valid epoch for proofs
//! last_finalized_index ← Highest index included in previous epoch root
//! last_epoch_slot      ← Slot when the last epoch was advanced
//!
//! Invariant: next_index <= next_pending_index
//! Invariant: earliest_provable_epoch <= current_epoch
//! Invariant: Epoch advances require current_slot >= last_epoch_slot + MIN_SLOTS_PER_NULLIFIER_EPOCH
//! ```

use super::commitment_tree::COMMITMENT_TREE_HEIGHT;
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

use crate::state::ShieldedPoolAccount;

/// Height of the nullifier indexed tree (same as commitment tree)
pub const NULLIFIER_TREE_HEIGHT: u8 = COMMITMENT_TREE_HEIGHT;

/// Maximum value for next_value sentinel, must be less than BN254 Fr modulus.
/// Stored in **big-endian** format as required by light_hasher::Poseidon.
/// BN254 Fr modulus (big-endian): 0x30644e72...
/// We use 0x2f at byte[0] (MSB) to stay safely under the modulus.
/// This represents "no next element" in the linked list.
pub const MAX_NULLIFIER_VALUE: [u8; 32] = [
    0x2f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
];

/// Grace period in epochs before permissionless nullifier cleanup is allowed.
///
/// During this window (after a nullifier becomes closable), only the nullifier's
/// original authority can reclaim the rent. After this window expires, anyone can
/// close the nullifier PDA and claim the rent as a garbage collection incentive.
///
/// At ~400ms per slot and 1 slot per epoch (conservative estimate), this is roughly:
/// - 43200 epochs × 0.4s ≈ 4.8 hours of exclusive reclaim window
///
/// This creates an incentive structure where:
/// 1. Authority has first right to reclaim their own nullifiers
/// 2. After grace period, third parties can clean up old state and earn rent
pub const CLEANUP_GRACE_EPOCHS: u64 = 43200;

/// Minimum number of slots that must pass before an epoch can advance when pending nullifiers exist.
///
/// Epochs can advance in two ways:
/// 1. All pending nullifiers are inserted (next_index >= next_pending_index)
/// 2. At least MIN_SLOTS_PER_NULLIFIER_EPOCH slots have passed since the last epoch advance
///
/// The second condition prevents epochs from being blocked indefinitely by pending
/// nullifiers that haven't been inserted yet. This is important for:
/// - Ensuring epoch advancement happens at a reasonable rate based on time
/// - Preventing a backlog of pending nullifiers from stalling the epoch lifecycle
/// - Allowing provers to use recent epoch roots for their proofs
///
/// At ~400ms per slot, 9000 slots ≈ 1 hour minimum epoch duration.
pub const MIN_SLOTS_PER_NULLIFIER_EPOCH: u64 = 9000;

/// Indexed leaf in the nullifier tree.
///
/// Each leaf contains:
/// - `value`: The nullifier hash
/// - `next_value`: Next larger nullifier in sorted order
/// - `next_index`: Tree index of the next leaf
#[repr(C)]
#[derive(Pod, Zeroable, Copy, Clone, Debug, PartialEq, Eq)]
pub struct IndexedLeaf {
    /// The nullifier value
    pub value: [u8; 32],
    /// Next larger value in sorted order (MAX_VALUE for last element)
    pub next_value: [u8; 32],
    /// Tree index of the next leaf (0 for last element pointing to genesis)
    pub next_index: u64,
}

impl IndexedLeaf {
    /// Create a new indexed leaf
    pub const fn new(value: [u8; 32], next_value: [u8; 32], next_index: u64) -> Self {
        Self {
            value,
            next_value,
            next_index,
        }
    }

    /// Create the genesis leaf (value=0, next_value=0, next_index=0)
    ///
    /// Per Aztec spec: next_value=0 represents infinity (end of sorted list).
    /// The circuit's OrderingCheck has special handling for this sentinel value.
    ///
    /// @see https://docs.aztec.network/developers/docs/foundational-topics/advanced/storage/indexed_merkle_tree
    pub const fn genesis() -> Self {
        Self {
            value: [0u8; 32],
            next_value: [0u8; 32], // 0 = infinity per Aztec spec
            next_index: 0,
        }
    }
}

/// Indexed merkle tree account for nullifier storage.
///
/// Key concepts:
/// - `root` is updated incrementally with each insertion
/// - Historical roots stored in EpochRootAccount PDAs (created by AdvanceNullifierEpoch)
/// - Non-membership proofs verify against EpochRootAccount PDAs or current root
/// - `next_index` tracks inserted leaves, `next_pending_index` tracks assigned PDAs
/// - `earliest_provable_epoch` determines which epochs are valid for proofs
///
/// # Genesis Leaf and Index Semantics
///
/// The tree is initialized with a **genesis leaf** at index 0. This sentinel leaf
/// `(value=0, next_value=0, next_index=0)` anchors the sorted linked list structure.
/// Per Aztec spec, `next_value=0` represents infinity (end of list).
///
/// As a result, both `next_index` and `next_pending_index` start at **1** after
/// initialization, not 0. Real nullifiers occupy indices 1 through `2^height - 1`,
/// giving a capacity of `2^height - 1` nullifiers (67,108,863 for height 26).
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][struct data]`
///
/// Field ordering is for proper alignment (u64 fields first, then [u8; 32], then u8).
#[account(ShieldedPoolAccount::NullifierIndexedTree)]
#[repr(C)]
pub struct NullifierIndexedTree {
    /// Next tree leaf index for insertions.
    ///
    /// Starts at 1 after initialization (index 0 is the genesis sentinel leaf).
    /// Incremented when nullifiers are actually inserted into the tree via NullifierBatchInsert.
    pub next_index: u64,

    /// Next pending index to assign to new nullifier PDAs.
    ///
    /// Starts at 1 after initialization (index 0 is reserved for genesis).
    /// Incremented when ExecuteTransact creates new nullifier PDAs.
    /// Invariant: `next_index <= next_pending_index`
    pub next_pending_index: u64,

    /// Current epoch number
    pub current_epoch: u64,

    /// Oldest epoch that is valid for ZK proof verification.
    /// Epochs older than this can have their EpochRootAccount PDAs closed.
    /// Updated via AdvanceEarliestProvableEpoch instruction.
    pub earliest_provable_epoch: u64,

    /// Last tree index included in the most recent epoch root
    pub last_finalized_index: u64,

    /// Slot when the last epoch was advanced.
    /// Used to enforce MIN_SLOTS_PER_EPOCH between epoch advances when pending nullifiers exist.
    pub last_epoch_slot: u64,

    /// Authority (global config authority)
    pub authority: Pubkey,

    /// Current root (updated on each insertion)
    pub root: [u8; 32],

    /// Subtrees for incremental merkle updates (sibling hashes on the path)
    pub subtrees: [[u8; 32]; COMMITMENT_TREE_HEIGHT as usize],

    /// Tree height (26)
    pub height: u8,

    /// PDA bump seed
    pub bump: u8,

    /// Padding for alignment to 8 bytes
    pub _padding: [u8; 6],
}

impl NullifierIndexedTree {
    /// Check if the given root matches the current tree root.
    /// For historical roots, use EpochRootAccount PDA validation instead.
    #[inline]
    pub fn is_current_root(&self, root: &[u8; 32]) -> bool {
        self.root == *root
    }

    /// Check if tree is full.
    ///
    /// Returns true when `next_index >= 2^height`. Since genesis occupies index 0
    /// and `next_index` starts at 1, this allows `2^height - 1` real nullifiers.
    pub fn is_full(&self) -> bool {
        self.next_index >= (1u64 << self.height)
    }

    /// Get total leaf slots in the tree (including genesis).
    ///
    /// Note: Actual nullifier capacity is `capacity() - 1` since index 0
    /// is reserved for the genesis sentinel leaf.
    pub fn capacity(&self) -> u64 {
        1u64 << self.height
    }

    /// Get the number of nullifiers that can still be inserted.
    ///
    /// This accounts for the genesis leaf at index 0.
    pub fn remaining_capacity(&self) -> u64 {
        self.capacity().saturating_sub(self.next_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_leaf() {
        // Per Aztec spec: genesis leaf has next_value = 0 (infinity)
        // @see https://docs.aztec.network/developers/docs/foundational-topics/advanced/storage/indexed_merkle_tree
        let genesis = IndexedLeaf::genesis();
        assert_eq!(genesis.value, [0u8; 32]);
        assert_eq!(genesis.next_value, [0u8; 32]); // 0 = infinity per Aztec spec
        assert_eq!(genesis.next_index, 0);
    }

    #[test]
    fn test_max_nullifier_value() {
        // Ensure MAX_NULLIFIER_VALUE is less than BN254 field modulus (big-endian)
        // The first byte (MSB in big-endian) should be 0x2f (47) which is < modulus byte 0x30
        assert_eq!(MAX_NULLIFIER_VALUE[0], 0x2f);
    }
}
