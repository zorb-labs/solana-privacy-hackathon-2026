use crate::errors::ShieldedPoolError;
use crate::state::ShieldedPoolAccount;
use light_hasher::Hasher;
use panchor::prelude::*;
use pinocchio::program_error::ProgramError;
use pinocchio_log::log;

/// Height of the receipt merkle tree
pub const RECEIPT_TREE_HEIGHT: u8 = 26;

/// Simple append-only receipt merkle tree account.
/// No history tracking needed - just stores the current root and subtrees.
///
/// This is a standard append-only merkle tree (not an indexed tree). Unlike the
/// nullifier tree, it has no genesis sentinel leaf, so `next_index` starts at 0.
///
/// # Index Semantics
///
/// - `next_index = 0` after initialization (empty tree)
/// - First receipt inserted goes to index 0
/// - Capacity is `2^height` receipts
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][struct data]`
#[account(ShieldedPoolAccount::ReceiptTree)]
#[repr(C)]
pub struct ReceiptMerkleTree {
    // === Metadata (frequently accessed) ===
    /// Authority that can manage this tree
    pub authority: Pubkey,
    /// Next index for insertion (also represents total receipts).
    /// Starts at 0 (no genesis leaf in standard merkle trees).
    pub next_index: u64,
    /// Current root of the tree
    pub root: [u8; 32],
    /// Tree height
    pub height: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 6],

    // === Tree State (large array, less frequently accessed) ===
    /// Subtree hashes for incremental merkle tree
    pub subtrees: [[u8; 32]; RECEIPT_TREE_HEIGHT as usize],
}

impl ReceiptMerkleTree {
    /// Initialize the receipt tree with zero values
    pub fn initialize<H: Hasher>(&mut self) -> Result<(), ProgramError> {
        let height = self.height as usize;

        let zero_bytes = H::zero_bytes();
        for i in 0..height {
            self.subtrees[i] = zero_bytes[i];
        }

        self.root = H::zero_bytes()[height];
        Ok(())
    }

    /// Append a receipt leaf hash to the tree
    pub fn append<H: Hasher>(&mut self, leaf: [u8; 32]) -> Result<(), ProgramError> {
        let height = self.height as usize;

        log!(
            "receipt_tree.append: height={}, next_index={}",
            height,
            self.next_index
        );

        let max_capacity = 1u64 << height;
        if self.next_index >= max_capacity {
            log!(
                "receipt_tree.append: tree full, next_index={}, max_capacity={}",
                self.next_index,
                max_capacity
            );
            return Err(ShieldedPoolError::MerkleTreeFull.into());
        }

        let mut current_index = self.next_index as usize;
        let mut current_level_hash = leaf;

        for i in 0..height {
            let zero_byte = H::zero_bytes()[i];

            let (left, right) = if current_index.is_multiple_of(2) {
                self.subtrees[i] = current_level_hash;
                (current_level_hash, zero_byte)
            } else {
                (self.subtrees[i], current_level_hash)
            };

            log!(
                "receipt_tree.append: level={}, current_index={}",
                i,
                current_index
            );
            current_level_hash = H::hashv(&[&left, &right]).map_err(|_e| {
                log!("receipt_tree.append: hashv failed at level={}", i);
                ShieldedPoolError::ArithmeticOverflow
            })?;
            current_index /= 2;
        }

        self.root = current_level_hash;
        self.next_index = self.next_index.checked_add(1).ok_or_else(|| {
            log!("receipt_tree.append: next_index overflow");
            ShieldedPoolError::ArithmeticOverflow
        })?;

        log!(
            "receipt_tree.append: success, new next_index={}",
            self.next_index
        );
        Ok(())
    }
}

// Receipt and NewReceiptEvent have been moved to the events module.
// See crate::events for these types.
