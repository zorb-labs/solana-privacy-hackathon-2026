pragma circom 2.0.0;

// =============================================================================
// NULLIFIER TREE OPERATIONS
// =============================================================================
// Nullifier-specific operations built on indexed merkle trees.
//
// The nullifier tree tracks spent notes to prevent double-spending:
//   - Non-membership: Prove a nullifier hasn't been spent
//   - Batch insertion: Add multiple nullifiers atomically
//
// Based on Aztec's indexed merkle tree design.
//
// Constraint Costs (HEIGHT=26):
//   NullifierNonMembership:         ~7,244 (IndexedMerkleTreeNonMembership)
//   NullifierBatchNonMembership(N): ~7,244 * N
//   NullifierTreeAppendWithProof:   ~13,000 (2 merkle traversals + subtree mux)
//   NullifierSingleInsert:          ~26,800 (ordering + low update + append)
//   NullifierBatchInsertSimple(N):  ~26,800 * N + overhead
// =============================================================================

include "circomlib/circuits/bitify.circom";
include "circomlib/circuits/poseidon.circom";
include "circomlib/circuits/mux1.circom";
include "circomlib/circuits/switcher.circom";

include "./indexed-merkle-tree.circom";

// =============================================================================
// ZERO HASHES FOR NULLIFIER TREE (HEIGHT 26)
// =============================================================================
// Precomputed zero hashes for the nullifier indexed merkle tree.
//
// ZERO_HASHES[0] = IndexedLeafHash({value=0, nextIndex=0, nextValue=0}) = Poseidon(0, 0, 0)
// ZERO_HASHES[i] = Poseidon(ZERO_HASHES[i-1], ZERO_HASHES[i-1])
//
// IMPORTANT: Uses 3-input Poseidon for indexed leaf hash at level 0.
//
function NULLIFIER_TREE_ZERO_HASHES(level) {
    var ZERO_HASHES[26];
    ZERO_HASHES[0] = 5317387130258456662214331362918410991734007599705406860481038345552731150762;
    ZERO_HASHES[1] = 5301900180746108365834837840355741695167403565517259206503735319173783315742;
    ZERO_HASHES[2] = 19759440382600727929415049642887307143518671081639244670052489500787514850212;
    ZERO_HASHES[3] = 11575399251628151734428362828441614938772848828475906566857213866326592241179;
    ZERO_HASHES[4] = 6632555919090241659299800894218068745568431736196896666697681740099319754273;
    ZERO_HASHES[5] = 2313232035512824863888346564211238648697583940443483502600731472911335817854;
    ZERO_HASHES[6] = 12219166190744012474665556054784140979314676975916090596913570678231824844496;
    ZERO_HASHES[7] = 16146864604902996392229526390577377437180881860230124064882884440248322100339;
    ZERO_HASHES[8] = 6883543445806624803603297055410892317599264946303553983246148642156945721809;
    ZERO_HASHES[9] = 11376031557295681140127084012245938798408060888509383225192187436273860950878;
    ZERO_HASHES[10] = 13241605803954237324747758640385138335781780544452364878098724458062976117242;
    ZERO_HASHES[11] = 17855149516804167337625231993818327714748909580849949294952537831754058414670;
    ZERO_HASHES[12] = 5150255556564484319136269061916843962561348275990403501481125286754601797805;
    ZERO_HASHES[13] = 6987786980040962217323608240860512602136308242543772977912408457104385595406;
    ZERO_HASHES[14] = 12673791472672914327028296381717349631548592060758239087545042240348016593302;
    ZERO_HASHES[15] = 9311366817918121883031003818542895863321158352954515731060536796838219379679;
    ZERO_HASHES[16] = 19585342603050165772395358149453302999296038452416557172220992666065524588903;
    ZERO_HASHES[17] = 8275043704423853810900845936958744738316525212865659311257431212306169446045;
    ZERO_HASHES[18] = 16186914510693313963181937763227692521094695382771382196248944425969899233840;
    ZERO_HASHES[19] = 767287730589592697964997275831534428290387386582193516309984231823744273525;
    ZERO_HASHES[20] = 8182961934280185552908516081891354585128675946832334410314642727305953230495;
    ZERO_HASHES[21] = 14553789876728003050984909720833228345703341783942046413329913248389004034924;
    ZERO_HASHES[22] = 6278449848160193613534961101404674224795668202070703678497109778769228770164;
    ZERO_HASHES[23] = 8979671514355837952844943277614674271246740514273131428387277329861932324931;
    ZERO_HASHES[24] = 21571534543733545789815777004636730528838914284333679118902566390287667028570;
    ZERO_HASHES[25] = 18924195170311205995329199132962258629761263537596441216670202833476308740987;
    return ZERO_HASHES[level];
}

// =============================================================================
// NullifierNonMembership
// =============================================================================
// Proves a single nullifier is NOT in the indexed tree.
// Wrapper around IndexedMerkleTreeNonMembership with nullifier-specific naming.
//
template NullifierNonMembership(HEIGHT) {
    signal input nullifier;
    signal input nullifier_tree_root;

    // Low element data
    signal input low_index;
    signal input low_value;
    signal input low_next_value;
    signal input low_next_index;
    signal input low_merkle_proof[HEIGHT];

    component nonMembership = IndexedMerkleTreeNonMembership(HEIGHT);
    nonMembership.value <== nullifier;
    nonMembership.root <== nullifier_tree_root;
    nonMembership.low_index <== low_index;
    nonMembership.low_value <== low_value;
    nonMembership.low_next_value <== low_next_value;
    nonMembership.low_next_index <== low_next_index;
    for (var h = 0; h < HEIGHT; h++) {
        nonMembership.low_merkle_proof[h] <== low_merkle_proof[h];
    }
}

// =============================================================================
// NullifierBatchNonMembership
// =============================================================================
// Proves multiple nullifiers are not in the indexed tree.
// Used for transaction verification with multiple inputs.
//
template NullifierBatchNonMembership(HEIGHT, N_NULLIFIERS) {
    // IMPORTANT: Declaration order determines public signals order in snarkjs output
    // Must be: nullifier_tree_root, then nullifiers
    signal input nullifier_tree_root;
    signal input nullifiers[N_NULLIFIERS];

    // Low element data for each nullifier
    signal input low_indices[N_NULLIFIERS];
    signal input low_values[N_NULLIFIERS];
    signal input low_next_values[N_NULLIFIERS];
    signal input low_next_indices[N_NULLIFIERS];
    signal input low_merkle_proofs[N_NULLIFIERS][HEIGHT];

    component nonMembership[N_NULLIFIERS];

    for (var i = 0; i < N_NULLIFIERS; i++) {
        nonMembership[i] = NullifierNonMembership(HEIGHT);
        nonMembership[i].nullifier <== nullifiers[i];
        nonMembership[i].nullifier_tree_root <== nullifier_tree_root;
        nonMembership[i].low_index <== low_indices[i];
        nonMembership[i].low_value <== low_values[i];
        nonMembership[i].low_next_value <== low_next_values[i];
        nonMembership[i].low_next_index <== low_next_indices[i];
        for (var h = 0; h < HEIGHT; h++) {
            nonMembership[i].low_merkle_proof[h] <== low_merkle_proofs[i][h];
        }
    }
}

// =============================================================================
// NullifierTreeAppendWithProof
// =============================================================================
// Append a nullifier leaf at index using full merkle proof.
//
// This correctly handles the case where low element updates have modified
// the tree state - the merkle proof is computed against the intermediate root.
//
// Also computes updated subtrees for on-chain storage.
//
template NullifierTreeAppendWithProof(HEIGHT) {
    signal input leaf;
    signal input index;
    signal input siblings[HEIGHT];        // Full merkle proof (sibling hashes)
    signal input subtrees_in[HEIGHT];     // Previous subtree cache
    signal output subtrees_out[HEIGHT];   // Updated subtree cache
    signal output old_root;               // Root before append (should be intermediate root)
    signal output new_root;               // Root after append

    component indexBits = Num2Bits(HEIGHT);
    indexBits.in <== index;

    // Compute old_root (before append) using zero hash at leaf position
    // The position at `index` should be empty (zero hash leaf)
    component old_switcher[HEIGHT];
    component old_hasher[HEIGHT];
    signal old_hashes[HEIGHT + 1];
    old_hashes[0] <== NULLIFIER_TREE_ZERO_HASHES(0);  // Empty leaf = zero hash at level 0

    for (var h = 0; h < HEIGHT; h++) {
        old_switcher[h] = Switcher();
        old_switcher[h].L <== old_hashes[h];
        old_switcher[h].R <== siblings[h];
        old_switcher[h].sel <== indexBits.out[h];

        old_hasher[h] = Poseidon(2);
        old_hasher[h].inputs[0] <== old_switcher[h].outL;
        old_hasher[h].inputs[1] <== old_switcher[h].outR;

        old_hashes[h + 1] <== old_hasher[h].out;
    }
    old_root <== old_hashes[HEIGHT];

    // Compute new_root (after append) using actual leaf
    component new_switcher[HEIGHT];
    component new_hasher[HEIGHT];
    signal new_hashes[HEIGHT + 1];
    new_hashes[0] <== leaf;

    for (var h = 0; h < HEIGHT; h++) {
        new_switcher[h] = Switcher();
        new_switcher[h].L <== new_hashes[h];
        new_switcher[h].R <== siblings[h];
        new_switcher[h].sel <== indexBits.out[h];

        new_hasher[h] = Poseidon(2);
        new_hasher[h].inputs[0] <== new_switcher[h].outL;
        new_hasher[h].inputs[1] <== new_switcher[h].outR;

        new_hashes[h + 1] <== new_hasher[h].out;
    }
    new_root <== new_hashes[HEIGHT];

    // Update subtree cache for on-chain storage
    // When we're a left child (bit=0), the current hash becomes the new subtree
    // When we're a right child (bit=1), keep the existing subtree
    component subtreeMux[HEIGHT];
    for (var h = 0; h < HEIGHT; h++) {
        subtreeMux[h] = Mux1();
        subtreeMux[h].c[0] <== new_hashes[h];     // Left child: update cache
        subtreeMux[h].c[1] <== subtrees_in[h];    // Right child: keep existing
        subtreeMux[h].s <== indexBits.out[h];
        subtrees_out[h] <== subtreeMux[h].out;
    }
}

// =============================================================================
// NullifierSingleInsert
// =============================================================================
// Proves one nullifier insertion: update low element + append new leaf.
// This is the core building block for batch insertion.
//
// Root chaining:
//   root_before -> (update low element) -> intermediate_root -> (append) -> root_after
//
// The append merkle proof must be computed against the intermediate root
// (after low element update), not the original root.
//
template NullifierSingleInsert(HEIGHT) {
    // Inputs
    signal input nullifier;
    signal input new_leaf_index;      // Where new leaf will be appended

    // Low element data
    signal input low_index;
    signal input low_value;
    signal input low_next_value;      // Old next_value (will be replaced)
    signal input low_next_index;      // Old next_index (will be replaced)
    signal input low_merkle_proof[HEIGHT];

    // Append merkle proof (against intermediate root after low element update)
    signal input append_merkle_proof[HEIGHT];

    // Subtree state
    signal input subtrees_in[HEIGHT];
    signal output subtrees_out[HEIGHT];

    // Root transitions
    signal input root_before;
    signal output root_after;

    // -------- CONSTRAINT 1: ORDERING --------
    // low_value < nullifier < low_next_value (or low_next_value == 0)
    component ordering = IndexedMerkleTreeOrderingCheck();
    ordering.low_value <== low_value;
    ordering.value <== nullifier;
    ordering.low_next_value <== low_next_value;

    // -------- CONSTRAINT 2: UPDATE LOW ELEMENT --------
    // Old low leaf: {low_value, low_next_value, low_next_index}
    component old_low_leaf = IndexedLeafHash();
    old_low_leaf.value <== low_value;
    old_low_leaf.next_value <== low_next_value;
    old_low_leaf.next_index <== low_next_index;

    // New low leaf: {low_value, nullifier, new_leaf_index}
    component new_low_leaf = IndexedLeafHash();
    new_low_leaf.value <== low_value;
    new_low_leaf.next_value <== nullifier;
    new_low_leaf.next_index <== new_leaf_index;

    // Update low element in-place, get intermediate root
    component update_low = IndexedMerkleTreeUpdateInPlace(HEIGHT);
    update_low.old_leaf <== old_low_leaf.hash;
    update_low.new_leaf <== new_low_leaf.hash;
    update_low.pathIndex <== low_index;
    for (var h = 0; h < HEIGHT; h++) {
        update_low.pathElements[h] <== low_merkle_proof[h];
    }

    // Verify old root matches
    update_low.old_root === root_before;

    // Intermediate root after low element update
    signal intermediate_root <== update_low.new_root;

    // -------- CONSTRAINT 3: APPEND NEW LEAF --------
    // New nullifier leaf: {nullifier, low_next_value, low_next_index}
    // (inherits low's OLD pointers)
    component new_nullifier_leaf = IndexedLeafHash();
    new_nullifier_leaf.value <== nullifier;
    new_nullifier_leaf.next_value <== low_next_value;
    new_nullifier_leaf.next_index <== low_next_index;

    // Append using full merkle proof against intermediate root
    component append = NullifierTreeAppendWithProof(HEIGHT);
    append.leaf <== new_nullifier_leaf.hash;
    append.index <== new_leaf_index;
    for (var h = 0; h < HEIGHT; h++) {
        append.siblings[h] <== append_merkle_proof[h];
        append.subtrees_in[h] <== subtrees_in[h];
    }

    // Verify append proof is against intermediate root (chains correctly!)
    append.old_root === intermediate_root;

    // Output updated subtrees and root
    for (var h = 0; h < HEIGHT; h++) {
        subtrees_out[h] <== append.subtrees_out[h];
    }
    root_after <== append.new_root;
}

// =============================================================================
// NullifierBatchInsertSimple
// =============================================================================
// Batch insert where all low elements come from the existing tree.
// No PENDING optimization (each nullifier's low element is already in tree).
//
// Root chaining per insertion:
//   root[i] -> (update low) -> intermediate -> (append) -> root[i+1]
//
// The append_merkle_proofs must be computed against the intermediate roots
// (after each low element update), which requires the prover to incrementally
// update the tree state.
//
template NullifierBatchInsertSimple(HEIGHT, BATCH_SIZE) {
    // ============ PUBLIC INPUTS ============
    // Order: old_root, new_root, starting_index, nullifiers (nullifiers last)
    signal input old_root;
    signal input new_root;
    signal input starting_index;
    signal input nullifiers[BATCH_SIZE];

    // ============ PRIVATE INPUTS ============
    signal input low_indices[BATCH_SIZE];
    signal input low_values[BATCH_SIZE];
    signal input low_next_values[BATCH_SIZE];
    signal input low_next_indices[BATCH_SIZE];
    signal input low_merkle_proofs[BATCH_SIZE][HEIGHT];
    signal input append_merkle_proofs[BATCH_SIZE][HEIGHT];
    signal input initial_subtrees[HEIGHT];

    // ============ STATE TRACKING ============
    signal subtrees[BATCH_SIZE + 1][HEIGHT];
    signal roots[BATCH_SIZE + 1];

    // Initialize
    for (var h = 0; h < HEIGHT; h++) {
        subtrees[0][h] <== initial_subtrees[h];
    }
    roots[0] <== old_root;

    // ============ PROCESS EACH NULLIFIER ============
    component inserts[BATCH_SIZE];

    for (var i = 0; i < BATCH_SIZE; i++) {
        inserts[i] = NullifierSingleInsert(HEIGHT);

        inserts[i].nullifier <== nullifiers[i];
        inserts[i].new_leaf_index <== starting_index + i;
        inserts[i].low_index <== low_indices[i];
        inserts[i].low_value <== low_values[i];
        inserts[i].low_next_value <== low_next_values[i];
        inserts[i].low_next_index <== low_next_indices[i];

        for (var h = 0; h < HEIGHT; h++) {
            inserts[i].low_merkle_proof[h] <== low_merkle_proofs[i][h];
            inserts[i].append_merkle_proof[h] <== append_merkle_proofs[i][h];
            inserts[i].subtrees_in[h] <== subtrees[i][h];
        }

        inserts[i].root_before <== roots[i];

        // Chain outputs to next iteration
        for (var h = 0; h < HEIGHT; h++) {
            subtrees[i + 1][h] <== inserts[i].subtrees_out[h];
        }
        roots[i + 1] <== inserts[i].root_after;
    }

    // ============ FINAL CONSTRAINT ============
    roots[BATCH_SIZE] === new_root;
}
