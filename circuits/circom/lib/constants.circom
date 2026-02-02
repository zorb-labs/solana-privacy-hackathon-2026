pragma circom 2.0.0;

// =============================================================================
// CIRCUIT CONSTANTS
// =============================================================================
// Shared constants used across Zorb protocol circuits.
// =============================================================================

// =============================================================================
// COMMITMENT_TREE_ZERO_LEAF
// =============================================================================
// Zero leaf value for the commitment merkle tree.
//
// Derivation: Poseidon(z, z) where z = keccak256("tornado") % p
//
// This follows the Tornado Cash convention for deterministic zero values.
// The "tornado" string provides a well-known, auditable seed value.
//
// z = keccak256("tornado") mod p
//   = 0x1a...74 (truncated to BN254 scalar field)
//
// COMMITMENT_TREE_ZERO_LEAF = Poseidon(z, z)
//   = 11850551329423159860688778991827824730037759162201783566284850822760196767874
//
function COMMITMENT_TREE_ZERO_LEAF() {
    return 11850551329423159860688778991827824730037759162201783566284850822760196767874;
}

// =============================================================================
// MERKLE_TREE_HEIGHT
// =============================================================================
// Default merkle tree height for commitment and nullifier trees.
//
// Height 26 supports 2^26 ≈ 67 million leaves, sufficient for protocol growth.
//
function MERKLE_TREE_HEIGHT() {
    return 26;
}

// =============================================================================
// ZORB_DOMAIN
// =============================================================================
// Protocol domain separator for note commitments.
//
// Derivation: Poseidon(0x7a6f7262) where 0x7a6f7262 is "zorb" as ASCII bigint.
//
// This ensures commitments are protocol-specific and cannot collide with other
// protocols using the same commitment structure. Domain separation is a standard
// cryptographic practice for preventing cross-protocol attacks.
//
// ASCII encoding: 'z'=0x7a, 'o'=0x6f, 'r'=0x72, 'b'=0x62
// Combined: 0x7a6f7262 = 2054386274
//
// ZORB_DOMAIN = Poseidon(2054386274)
//   = 13585635423589395198278902149970508553677724666160675593377523211102802660896
//
function ZORB_DOMAIN() {
    return 13585635423589395198278902149970508553677724666160675593377523211102802660896;
}

// =============================================================================
// AMOUNT_BITS
// =============================================================================
// Bit width for amount range checks.
//
// 248 bits is chosen to:
//   - Prevent overflow in (amount × accumulator) multiplication
//   - Leave headroom below the 254-bit field size
//   - Support amounts up to ~4.5 × 10^74 (vastly exceeds any token supply)
//
// Used in: LessEqThan, Num2Bits for amount validation
//
function AMOUNT_BITS() {
    return 248;
}

// =============================================================================
// FIELD_BITS
// =============================================================================
// Bit width of BN254 scalar field elements.
//
// The BN254 scalar field has order ~2^254, so all field elements fit in 254 bits.
// Used for:
//   - Range binding public inputs (prevents malleability)
//   - Field element decomposition in FieldLessThan
//
function FIELD_BITS() {
    return 254;
}

// =============================================================================
// ACCUMULATOR_SCALE
// =============================================================================
// Fixed-point scale for reward accumulators (1e18).
//
// The accumulator stores yield rates as fixed-point numbers scaled by 1e18.
// This is the standard DeFi convention (same as Ethereum's wei/ether ratio).
//
// Reward calculation: reward = amount × (globalAcc - noteAcc) / ACCUMULATOR_SCALE
//
// The scale 1e18 provides 18 decimal places of precision, sufficient for
// accurate yield calculations even with very small per-block rates.
//
function ACCUMULATOR_SCALE() {
    return 1000000000000000000;
}
