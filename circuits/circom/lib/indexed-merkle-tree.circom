pragma circom 2.0.0;

// =============================================================================
// INDEXED MERKLE TREE PRIMITIVES
// =============================================================================
// Core operations for indexed merkle trees (Aztec-style).
//
// An indexed merkle tree is a merkle tree where each leaf contains:
//   - value: The stored value (e.g., a nullifier)
//   - next_index: Pointer to the next leaf in sorted order
//   - next_value: The value at next_index
//
// This creates a sorted linked list embedded in a merkle tree, enabling:
//   - Non-membership proofs: Prove a value is NOT in the tree
//   - Efficient insertion: O(log n) with only 2 leaf updates
//
// Reference: Aztec Protocol indexed merkle tree specification
//
// Constraint Costs:
//   IndexedLeafHash:                ~393 (Poseidon(3))
//   IndexedMerkleTreeOrderingCheck: ~2,550 (2 × FieldLessThan + IsZero + OR logic)
//   IndexedMerkleTreeUpdateInPlace(H):     ~492 × H (2 merkle traversals, shared path bits)
//   IndexedMerkleTreeNonMembership: ~7,244 (ordering + leaf hash + merkle proof)
//
// Example for HEIGHT=26:
//   IndexedMerkleTreeUpdateInPlace(26) ≈ 12,800 constraints
// =============================================================================

include "circomlib/circuits/bitify.circom";
include "circomlib/circuits/comparators.circom";
include "circomlib/circuits/poseidon.circom";
include "circomlib/circuits/switcher.circom";

include "./merkle.circom";
include "./field-comparators.circom";

// =============================================================================
// IndexedLeafHash
// =============================================================================
// Compute the hash of an indexed merkle tree leaf.
//
// Hash: Poseidon(value, next_index, next_value)
//
// Following Aztec's indexed merkle tree design with 3-input Poseidon.
//
template IndexedLeafHash() {
    signal input value;
    signal input next_value;
    signal input next_index;
    signal output hash;

    component hasher = Poseidon(3);
    hasher.inputs[0] <== value;
    hasher.inputs[1] <== next_index;
    hasher.inputs[2] <== next_value;

    hash <== hasher.out;
}

// =============================================================================
// IndexedMerkleTreeOrderingCheck
// =============================================================================
// Verifies correct ordering for indexed tree operations.
//
// Checks: low_value < value < low_next_value (or low_next_value == 0)
//
// This is the core constraint for:
//   - Non-membership proofs: Proves value is NOT in the tree
//   - Insertion proofs: Proves correct position in sorted linked list
//
// Edge case: When low_next_value == 0, it represents infinity (last element
// in the sorted list per Aztec's spec). Any value greater than low_value
// is valid in this case.
//
// Uses FieldLessThan for full BN254 field element support since values
// are typically Poseidon hashes which can be any value in [0, p).
//
template IndexedMerkleTreeOrderingCheck() {
    signal input low_value;
    signal input value;
    signal input low_next_value;

    // low_value < value (using field-safe comparison)
    component lt1 = FieldLessThan();
    lt1.in[0] <== low_value;
    lt1.in[1] <== value;
    lt1.out === 1;

    // Check if low_next_value == 0 (represents infinity/last element)
    component isNextZero = IsZero();
    isNextZero.in <== low_next_value;

    // Check if value < low_next_value (using field-safe comparison)
    component lt2 = FieldLessThan();
    lt2.in[0] <== value;
    lt2.in[1] <== low_next_value;

    // Valid if: value < low_next_value OR low_next_value == 0
    // OR gate: a OR b = a + b - a*b
    signal validUpperBound <== lt2.out + isNextZero.out - lt2.out * isNextZero.out;
    validUpperBound === 1;
}

// =============================================================================
// IndexedMerkleTreeUpdateInPlace
// =============================================================================
// Update a leaf at pathIndex, returning both old and new roots.
//
// Uses the same sibling path for both computations (valid because
// only the leaf changes, not the path structure).
//
template IndexedMerkleTreeUpdateInPlace(HEIGHT) {
    signal input old_leaf;
    signal input new_leaf;
    signal input pathElements[HEIGHT];
    signal input pathIndex;
    signal output old_root;
    signal output new_root;

    component indexBits = Num2Bits(HEIGHT);
    indexBits.in <== pathIndex;

    // Compute old root
    component old_switcher[HEIGHT];
    component old_hasher[HEIGHT];

    for (var i = 0; i < HEIGHT; i++) {
        old_switcher[i] = Switcher();
        old_switcher[i].L <== i == 0 ? old_leaf : old_hasher[i - 1].out;
        old_switcher[i].R <== pathElements[i];
        old_switcher[i].sel <== indexBits.out[i];

        old_hasher[i] = Poseidon(2);
        old_hasher[i].inputs[0] <== old_switcher[i].outL;
        old_hasher[i].inputs[1] <== old_switcher[i].outR;
    }
    old_root <== old_hasher[HEIGHT - 1].out;

    // Compute new root with updated leaf
    component new_switcher[HEIGHT];
    component new_hasher[HEIGHT];

    for (var i = 0; i < HEIGHT; i++) {
        new_switcher[i] = Switcher();
        new_switcher[i].L <== i == 0 ? new_leaf : new_hasher[i - 1].out;
        new_switcher[i].R <== pathElements[i];
        new_switcher[i].sel <== indexBits.out[i];

        new_hasher[i] = Poseidon(2);
        new_hasher[i].inputs[0] <== new_switcher[i].outL;
        new_hasher[i].inputs[1] <== new_switcher[i].outR;
    }
    new_root <== new_hasher[HEIGHT - 1].out;
}

// =============================================================================
// IndexedMerkleTreeNonMembership
// =============================================================================
// Proves a value is NOT in the indexed merkle tree.
//
// Algorithm:
//   1. Find the "low element" - the largest value in the tree < target value
//   2. Verify: low_value < target < low_next_value
//   3. Verify the low element exists in the tree via merkle proof
//
// If such a low element exists, target cannot be in the tree (it would
// have to be between low and low_next, but that gap has no elements).
//
template IndexedMerkleTreeNonMembership(HEIGHT) {
    signal input value;
    signal input root;

    // Low element data
    signal input low_index;
    signal input low_value;
    signal input low_next_value;
    signal input low_next_index;
    signal input low_merkle_proof[HEIGHT];

    // -------- CONSTRAINT 1: ORDERING --------
    // low_value < value < low_next_value (or low_next_value == 0)
    component ordering = IndexedMerkleTreeOrderingCheck();
    ordering.low_value <== low_value;
    ordering.value <== value;
    ordering.low_next_value <== low_next_value;

    // -------- CONSTRAINT 2: LOW ELEMENT EXISTS IN TREE --------
    // Compute low element hash
    component low_leaf = IndexedLeafHash();
    low_leaf.value <== low_value;
    low_leaf.next_value <== low_next_value;
    low_leaf.next_index <== low_next_index;

    // Verify merkle proof
    component merkle = MerkleProof(HEIGHT);
    merkle.leaf <== low_leaf.hash;
    merkle.pathIndices <== low_index;
    for (var h = 0; h < HEIGHT; h++) {
        merkle.pathElements[h] <== low_merkle_proof[h];
    }

    // Computed root must match provided root
    merkle.root === root;
}
