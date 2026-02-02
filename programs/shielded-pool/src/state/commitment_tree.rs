use panchor::prelude::*;

use crate::state::ShieldedPoolAccount;

/// Height of the commitment merkle tree (2^26 = 67M leaves)
pub const COMMITMENT_TREE_HEIGHT: u8 = 26;

/// Size of the root history circular buffer.
/// At ~10 roots/min, 256 entries allows proofs against roots from ~25 minutes ago.
pub const ROOT_HISTORY_SIZE: usize = 256;

/// Commitment merkle tree account for storing the commitment tree state.
///
/// This is a standard append-only merkle tree (not an indexed tree). Unlike the
/// nullifier tree, it has no genesis sentinel leaf, so `next_index` starts at 0.
///
/// # Index Semantics
///
/// - `next_index = 0` after initialization (empty tree)
/// - First commitment inserted goes to index 0
/// - Capacity is `2^height` = 67,108,864 commitments
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][struct data]`
// Note: ShankAccount not used due to nested array types not being supported
#[account(ShieldedPoolAccount::CommitmentTree)]
#[repr(C)]
pub struct CommitmentMerkleTree {
    // === Metadata (frequently accessed) ===
    pub authority: Pubkey,
    /// Next index for insertion. Starts at 0 (no genesis leaf in standard merkle trees).
    pub next_index: u64,
    /// Index into root_history (circular buffer cursor)
    pub root_index: u64,
    /// Tree height (constant after init)
    pub height: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Size of root history circular buffer
    pub root_history_size: u16,
    /// Padding for alignment
    pub _padding: [u8; 4],

    // === Tree State (large arrays, less frequently accessed) ===
    /// Current root of the tree
    pub root: [u8; 32],
    /// Subtree hashes for incremental merkle tree
    pub subtrees: [[u8; 32]; COMMITMENT_TREE_HEIGHT as usize],
    /// History of past roots for proof verification
    pub root_history: [[u8; 32]; ROOT_HISTORY_SIZE],
}

impl CommitmentMerkleTree {}
