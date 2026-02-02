pragma circom 2.0.0;

// =============================================================================
// FIELD ELEMENT COMPARATORS
// =============================================================================
// Comparison templates for full BN254 field elements.
//
// circomlib's LessThan(n) only works for values < 2^n. These templates
// correctly handle the full field range [0, p) where p ≈ 2^254.
//
// Use cases:
//   - Comparing Poseidon hashes (can be any value in [0, p))
//   - Nullifier ordering in indexed merkle trees
//   - Any comparison where values may exceed 2^252
//
// Constraint Costs:
//   FieldLessThan:    ~1,271 (2 × Num2Bits(254) + bit comparisons)
//   FieldLessEqThan:  ~1,271 (delegates to FieldLessThan)
//   FieldGreaterThan: ~1,271 (delegates to FieldLessThan)
// =============================================================================

include "circomlib/circuits/bitify.circom";

// =============================================================================
// FieldLessThan
// =============================================================================
// Compares two BN254 field elements: returns 1 if in[0] < in[1], 0 otherwise.
//
// Algorithm:
//   1. Decompose both inputs to 254 bits
//   2. Compare bit by bit from MSB (bit 253) to LSB (bit 0)
//   3. The first differing bit determines the result:
//      - If in[0]'s bit is 0 and in[1]'s bit is 1 → in[0] < in[1]
//      - If in[0]'s bit is 1 and in[1]'s bit is 0 → in[0] > in[1]
//      - If all bits equal → in[0] == in[1] (not less than)
//
// Constraints: ~1271 (vs ~504 for LessThan(252))
// Trade-off: More constraints but correct for ALL field elements
//
template FieldLessThan() {
    signal input in[2];
    signal output out;

    // Decompose both inputs to 254 bits
    // BN254 field modulus p < 2^254, so all field elements fit in 254 bits
    component bits0 = Num2Bits(254);
    component bits1 = Num2Bits(254);
    bits0.in <== in[0];
    bits1.in <== in[1];

    // For each bit position:
    // lt[i] = 1 iff bits0[i] < bits1[i] (i.e., bits0[i]=0 AND bits1[i]=1)
    // eq[i] = 1 iff bits0[i] == bits1[i]
    signal lt[254];
    signal eq[254];

    for (var i = 0; i < 254; i++) {
        // bits0 < bits1 at position i means: bits0=0 AND bits1=1
        lt[i] <== (1 - bits0.out[i]) * bits1.out[i];

        // bits0 == bits1 at position i
        // eq = (1-a)(1-b) + ab = 1 - a - b + 2ab
        eq[i] <== 1 - bits0.out[i] - bits1.out[i] + 2 * bits0.out[i] * bits1.out[i];
    }

    // allEqAbove[i] = 1 iff all bits from MSB (253) down to position i are equal
    // This tracks whether we've found a differing bit yet (scanning from MSB)
    signal allEqAbove[255];
    allEqAbove[254] <== 1;  // No bits above position 253

    for (var i = 253; i >= 0; i--) {
        allEqAbove[i] <== allEqAbove[i + 1] * eq[i];
    }

    // contribution[i] = 1 iff position i is the FIRST differing bit AND in[0] < in[1] there
    // At most one contribution[i] can be 1 (the first difference)
    signal contribution[254];
    for (var i = 0; i < 254; i++) {
        contribution[i] <== allEqAbove[i + 1] * lt[i];
    }

    // Sum all contributions - result is 0 or 1
    // (0 if in[0] >= in[1], 1 if in[0] < in[1])
    // Using linear combination (free in R1CS) instead of intermediate signals
    var total = 0;
    for (var i = 0; i < 254; i++) {
        total += contribution[i];
    }
    out <== total;
}

// =============================================================================
// FieldLessEqThan
// =============================================================================
// Compares two BN254 field elements: returns 1 if in[0] <= in[1], 0 otherwise.
//
// Implementation: a <= b is equivalent to NOT(b < a)
//
template FieldLessEqThan() {
    signal input in[2];
    signal output out;

    component lt = FieldLessThan();
    lt.in[0] <== in[1];  // Swap: check if in[1] < in[0]
    lt.in[1] <== in[0];

    out <== 1 - lt.out;  // NOT(in[1] < in[0]) = in[0] <= in[1]
}

// =============================================================================
// FieldGreaterThan
// =============================================================================
// Compares two BN254 field elements: returns 1 if in[0] > in[1], 0 otherwise.
//
// Implementation: a > b is equivalent to b < a
//
template FieldGreaterThan() {
    signal input in[2];
    signal output out;

    component lt = FieldLessThan();
    lt.in[0] <== in[1];  // Swap: check if in[1] < in[0]
    lt.in[1] <== in[0];

    out <== lt.out;
}
