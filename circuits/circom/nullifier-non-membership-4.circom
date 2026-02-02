pragma circom 2.0.0;

include "./lib/constants.circom";
include "./lib/nullifier-tree.circom";

// =============================================================================
// NULLIFIER NON-MEMBERSHIP CIRCUIT - 4 Nullifiers
// =============================================================================
//
// Proves that nullifiers don't exist in the indexed Merkle tree.
// Server-side circuit verified on-chain during transaction submission.
// Uses epoch-stable root for consistency during batch processing.
//
// Template Parameters:
//   HEIGHT = MERKLE_TREE_HEIGHT()  Tree depth (supports ~67M leaves)
//   BATCH_SIZE = 4        Nullifiers checked per proof
//
// Estimated constraints: ~29k
//
// Public Inputs:
//   nullifier_tree_root   Merkle root of the nullifier indexed tree (epoch root)
//   nullifiers[4]         Array of nullifiers to prove non-membership
//
// =============================================================================
component main {
    public [
        nullifier_tree_root,
        nullifiers
    ]
} = NullifierBatchNonMembership(MERKLE_TREE_HEIGHT(), 4);
