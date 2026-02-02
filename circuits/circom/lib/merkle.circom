pragma circom 2.0.0;

// =============================================================================
// MERKLE PROOF VERIFICATION
// =============================================================================
// Verifies that a merkle proof is correct for a given leaf and path.
// Returns the computed root.
//
// Parameters:
//   - levels: Height of the merkle tree
//
// Inputs:
//   - leaf: The leaf value to verify
//   - pathElements[levels]: Sibling hashes along the path
//   - pathIndices: Bit-packed path direction (0 = left, 1 = right)
//
// Output:
//   - root: The computed merkle root
//
// Constraints: levels * (Poseidon(2) + Switcher) + Num2Bits(levels)
//   ≈ levels * 246 + levels (for Poseidon(2) ≈ 243, Switcher ≈ 3)
//
// Example: MerkleProof(20) ≈ 4,940 constraints
//
// =============================================================================

include "circomlib/circuits/bitify.circom";
include "circomlib/circuits/poseidon.circom";
include "circomlib/circuits/switcher.circom";

template MerkleProof(levels) {
    signal input leaf;
    signal input pathElements[levels];
    signal input pathIndices;
    signal output root;

    component switcher[levels];
    component hasher[levels];

    component indexBits = Num2Bits(levels);
    indexBits.in <== pathIndices;

    for (var i = 0; i < levels; i++) {
        switcher[i] = Switcher();
        switcher[i].L <== i == 0 ? leaf : hasher[i - 1].out;
        switcher[i].R <== pathElements[i];
        switcher[i].sel <== indexBits.out[i];

        hasher[i] = Poseidon(2);
        hasher[i].inputs[0] <== switcher[i].outL;
        hasher[i].inputs[1] <== switcher[i].outR;
    }

    root <== hasher[levels - 1].out;
}
