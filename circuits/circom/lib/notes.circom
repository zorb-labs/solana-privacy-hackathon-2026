pragma circom 2.0.0;

// =============================================================================
// NOTE OPERATIONS
// =============================================================================
// Templates for note commitment, nullifier computation, and uniqueness checks.
//
// Constraint Costs (Poseidon(n) ≈ 168 + 75*n):
//   NoteCommitment:              ~768 (Poseidon(8))
//   ComputeNullifier:            ~393 (Poseidon(3))
//   EnforceNullifierUniqueness:  n*(n-1)/2 * ~244 (IsEqual per pair)
// =============================================================================

include "circomlib/circuits/poseidon.circom";
include "circomlib/circuits/comparators.circom";

include "./constants.circom";  // ZORB_DOMAIN()

// Compute note commitment from note fields (position-independent nullifier model)
// commitment = Poseidon(ZORB_DOMAIN, version, assetId, amount, pk, blinding, rewardAccumulator, rho)
//
// The 8-field structure enables:
// - ZORB_DOMAIN: Protocol-specific domain separator (prevents cross-protocol collisions)
// - version: Circuit isolation field (see below)
// - assetId: Multi-asset support
// - amount: Value of the note
// - pk: Owner's public key (binds to owner)
// - blinding: Randomness for hiding (rcm)
// - rewardAccumulator: Snapshot for yield calculation
// - rho: Uniqueness parameter for position-independent nullifiers (Orchard model)
//
// POSITION-INDEPENDENT NULLIFIERS (RHO FIELD)
// -------------------------------------------
// The `rho` field enables position-independent nullifier derivation:
//
//   nullifier = Poseidon(nk, rho, commitment)
//
// For output notes created by transactions:
//   - rho is derived from the nullifier of the spent note (1:1 pairing)
//   - This creates a chain: spent note's nullifier → new note's rho
//
// Benefits:
//   - Nullifiers don't depend on merkle tree position (pathIndices)
//   - Notes can be inserted at any tree position without changing nullifier
//   - Simplifies wallet recovery (no position tracking required)
//
// CIRCUIT ISOLATION VIA VERSION FIELD
// ------------------------------------
// The `version` field provides isolation between circuits:
//
//   - Transaction circuit enforces: version === 0
//   - Future circuits MUST use: version !== 0
//
// This ensures notes created by one circuit cannot be consumed by another:
//   - Different version → different commitment → different nullifier
//   - No cross-circuit attacks possible
//
// When Zorb evolves to programmable privacy with multiple circuits, the version
// field will be replaced by a proper circuitId in a new commitment scheme.
// Until then, version=0 enforcement provides equivalent security.
template NoteCommitment() {
    signal input version;
    signal input assetId;
    signal input amount;
    signal input pk;
    signal input blinding;
    signal input rewardAccumulator;
    signal input rho;              // Uniqueness parameter (from spent note's nullifier)
    signal output commitment;

    component hasher = Poseidon(8);
    hasher.inputs[0] <== ZORB_DOMAIN();
    hasher.inputs[1] <== version;
    hasher.inputs[2] <== assetId;
    hasher.inputs[3] <== amount;
    hasher.inputs[4] <== pk;
    hasher.inputs[5] <== blinding;
    hasher.inputs[6] <== rewardAccumulator;
    hasher.inputs[7] <== rho;
    commitment <== hasher.out;
}

// Compute nullifier for spending a note (position-independent)
// nullifier = Poseidon(nk, rho, commitment)
//
// Position-independent nullifier derivation (Orchard model):
// - nk: Nullifier deriving key (derived from nsk)
// - rho: Uniqueness parameter (from spent note that created this note)
// - commitment: Defense-in-depth binding to note content
template ComputeNullifier() {
    signal input nk;
    signal input rho;              // Replaces pathIndices
    signal input commitment;       // Kept for defense-in-depth
    signal output nullifier;

    component hasher = Poseidon(3);
    hasher.inputs[0] <== nk;
    hasher.inputs[1] <== rho;
    hasher.inputs[2] <== commitment;
    nullifier <== hasher.out;
}

// Verify that all nullifiers in a transaction are unique
// Prevents double-spending within a single transaction
template EnforceNullifierUniqueness(n) {
    signal input nullifiers[n];

    component eq[n * (n - 1) / 2];

    var idx = 0;
    for (var i = 0; i < n - 1; i++) {
        for (var j = i + 1; j < n; j++) {
            eq[idx] = IsEqual();
            eq[idx].in[0] <== nullifiers[i];
            eq[idx].in[1] <== nullifiers[j];
            eq[idx].out === 0;  // Must not be equal
            idx++;
        }
    }
}
