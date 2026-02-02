use crate::{errors::ShieldedPoolError, state::CommitmentMerkleTree};
use alloc::vec;
use light_hasher::Hasher;
use pinocchio::program_error::ProgramError;
use pinocchio_log::log;

/// Standard append-only merkle tree operations for the commitment tree.
///
/// Unlike the indexed nullifier tree, this tree has no genesis sentinel leaf.
/// The `next_index` field starts at 0 after initialization, and the first
/// commitment is inserted at index 0.
pub struct MerkleTree;

impl MerkleTree {
    /// Initialize the commitment merkle tree with zero values.
    ///
    /// After initialization:
    /// - `next_index = 0` (first insertion goes to index 0)
    /// - `root` = zero hash at tree height
    /// - `root_history[0]` = initial root
    pub fn initialize<H: Hasher>(
        merkle_tree_account: &mut CommitmentMerkleTree,
    ) -> Result<(), ProgramError> {
        let height = merkle_tree_account.height as usize;

        let zero_bytes = H::zero_bytes();
        for i in 0..height {
            merkle_tree_account.subtrees[i] = zero_bytes[i];
        }

        let initial_root = H::zero_bytes()[height];
        merkle_tree_account.root = initial_root;
        merkle_tree_account.root_history[0] = initial_root;

        Ok(())
    }

    pub fn append<H: Hasher>(
        leaf: [u8; 32],
        merkle_tree_account: &mut CommitmentMerkleTree,
    ) -> Result<alloc::vec::Vec<[u8; 32]>, ProgramError> {
        let height = merkle_tree_account.height as usize;
        let root_history_size = merkle_tree_account.root_history_size as usize;

        let max_capacity = 1u64 << height;
        if merkle_tree_account.next_index >= max_capacity {
            return Err(ShieldedPoolError::MerkleTreeFull.into());
        }

        let mut current_index = merkle_tree_account.next_index as usize;
        let mut current_level_hash = leaf;
        let mut left;
        let mut right;
        let mut proof = vec![[0u8; 32]; height];

        for i in 0..height {
            let subtree = &mut merkle_tree_account.subtrees[i];
            let zero_byte = H::zero_bytes()[i];

            if current_index.is_multiple_of(2) {
                left = current_level_hash;
                right = zero_byte;
                *subtree = current_level_hash;
                proof[i] = right;
            } else {
                left = *subtree;
                right = current_level_hash;
                proof[i] = left;
            }
            current_level_hash = H::hashv(&[&left, &right]).map_err(|_| {
                log!("merkle hash error");
                ShieldedPoolError::ArithmeticOverflow
            })?;
            current_index /= 2;
        }

        merkle_tree_account.root = current_level_hash;
        merkle_tree_account.next_index = merkle_tree_account
            .next_index
            .checked_add(1)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        let new_root_index = (merkle_tree_account.root_index as usize)
            .checked_add(1)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?
            % root_history_size;
        merkle_tree_account.root_index = new_root_index as u64;
        merkle_tree_account.root_history[new_root_index] = current_level_hash;

        Ok(proof)
    }

    pub fn is_known_root(merkle_tree_account: &CommitmentMerkleTree, root: [u8; 32]) -> bool {
        if root == [0u8; 32] {
            return false;
        }

        let root_history_size = merkle_tree_account.root_history_size as usize;
        let current_root_index = merkle_tree_account.root_index as usize;

        // Search backwards through the circular root history buffer
        for offset in 0..root_history_size {
            let i = (current_root_index + root_history_size - offset) % root_history_size;
            if root == merkle_tree_account.root_history[i] {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{COMMITMENT_TREE_HEIGHT, ROOT_HISTORY_SIZE};
    use light_hasher::Poseidon;
    use std::{format, println, string::String};

    fn create_test_tree() -> CommitmentMerkleTree {
        CommitmentMerkleTree {
            authority: [0u8; 32],
            next_index: 0,
            root_index: 0,
            height: COMMITMENT_TREE_HEIGHT,
            bump: 0,
            root_history_size: ROOT_HISTORY_SIZE as u16,
            _padding: [0u8; 4],
            root: [0u8; 32],
            subtrees: [[0u8; 32]; COMMITMENT_TREE_HEIGHT as usize],
            root_history: [[0u8; 32]; ROOT_HISTORY_SIZE],
        }
    }

    #[test]
    fn test_initial_root_matches_poseidon_zero_bytes() {
        let mut tree = create_test_tree();
        MerkleTree::initialize::<Poseidon>(&mut tree).unwrap();

        // The initial root should be zero_bytes[height]
        let expected_root = Poseidon::zero_bytes()[COMMITMENT_TREE_HEIGHT as usize];

        assert_eq!(
            tree.root, expected_root,
            "Initial root should match Poseidon zero_bytes at height {}",
            COMMITMENT_TREE_HEIGHT
        );

        // Print the initial root for reference
        println!(
            "Initial root for height {}: {:?}",
            COMMITMENT_TREE_HEIGHT, tree.root
        );
    }

    #[test]
    fn test_initial_root_in_history() {
        let mut tree = create_test_tree();
        MerkleTree::initialize::<Poseidon>(&mut tree).unwrap();

        let expected_root = Poseidon::zero_bytes()[COMMITMENT_TREE_HEIGHT as usize];

        // Initial root should be stored in root_history[0]
        assert_eq!(
            tree.root_history[0], expected_root,
            "Initial root should be in root_history[0]"
        );
    }

    #[test]
    fn test_initial_root_is_known() {
        let mut tree = create_test_tree();
        MerkleTree::initialize::<Poseidon>(&mut tree).unwrap();

        // The initial root should be recognized as known
        assert!(
            MerkleTree::is_known_root(&tree, tree.root),
            "Initial root should be a known root"
        );
    }

    #[test]
    fn test_zero_root_is_not_known() {
        let mut tree = create_test_tree();
        MerkleTree::initialize::<Poseidon>(&mut tree).unwrap();

        // All-zero root should never be recognized as valid
        let zero_root = [0u8; 32];
        assert!(
            !MerkleTree::is_known_root(&tree, zero_root),
            "Zero root should not be a known root"
        );
    }

    #[test]
    fn test_initial_subtrees_are_zero_bytes() {
        let mut tree = create_test_tree();
        MerkleTree::initialize::<Poseidon>(&mut tree).unwrap();

        let zero_bytes = Poseidon::zero_bytes();

        // Each subtree level should be initialized with the corresponding zero_bytes
        for i in 0..COMMITMENT_TREE_HEIGHT as usize {
            assert_eq!(
                tree.subtrees[i], zero_bytes[i],
                "Subtree at level {} should be zero_bytes[{}]",
                i, i
            );
        }
    }

    #[test]
    fn test_initial_root_value_for_height_26() {
        // This test verifies the exact expected initial root value for height 26
        // Used as a regression test to ensure consistency across updates
        let mut tree = create_test_tree();
        assert_eq!(tree.height, 26, "This test is for height 26");

        MerkleTree::initialize::<Poseidon>(&mut tree).unwrap();

        // Expected initial root for Poseidon with height 26
        // This is Poseidon::zero_bytes()[26]
        let expected_root: [u8; 32] = [
            18, 12, 88, 241, 67, 212, 145, 233, 89, 2, 247, 245, 39, 119, 120, 162, 224, 173, 81,
            104, 246, 173, 215, 86, 105, 147, 38, 48, 206, 97, 21, 24,
        ];

        assert_eq!(
            tree.root, expected_root,
            "Initial root for height 26 should match expected value"
        );

        // Print hex representation for easy verification
        println!(
            "Initial root (hex): 0x{}",
            tree.root
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>()
        );
    }
}
