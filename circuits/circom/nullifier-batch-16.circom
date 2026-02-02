pragma circom 2.0.0;

include "./lib/constants.circom";
include "./lib/nullifier-tree.circom";

// =============================================================================
// NULLIFIER BATCH INSERT CIRCUIT - 16 Nullifiers
// =============================================================================
//
// Proves batch insertion of 16 nullifiers into the indexed Merkle tree.
// Uses the Aztec-style indexed tree structure for efficient non-membership proofs.
//
// Template Parameters:
//   HEIGHT = MERKLE_TREE_HEIGHT()  Tree depth (supports ~67M leaves)
//   BATCH_SIZE = 16       Nullifiers inserted per batch
//
// Estimated constraints: ~850k
//
// Public Inputs:
//   old_root              Merkle root before batch insertion
//   new_root              Merkle root after batch insertion
//   starting_index        Index where first nullifier will be appended
//   nullifiers[16]        Array of nullifiers to insert
//
// =============================================================================
component main {
    public [
        old_root,
        new_root,
        starting_index,
        nullifiers
    ]
} = NullifierBatchInsertSimple(MERKLE_TREE_HEIGHT(), 16);
