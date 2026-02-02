pragma circom 2.0.0;

// =============================================================================
// TRANSACTION CIRCUIT
// Multi-asset shielded transfer with yield accrual
// =============================================================================
//
// OVERVIEW
// --------
// This circuit proves the validity of a shielded transaction that can:
//   - Spend multiple input notes (reveal nullifiers, prove ownership)
//   - Create multiple output notes (commit to new notes)
//   - Deposit or withdraw publicly (on-chain visible amounts)
//   - Accrue yield rewards based on time held in the shielded pool
//
// The circuit supports multiple asset types in a single transaction while
// keeping the specific assets private. Value conservation is enforced
// per-asset to prevent cross-asset value transfer.
//
// ARCHITECTURE: THREE-TIER ROUTING
// --------------------------------
// The circuit uses three tiers to route value flows:
//
//   [PUBLIC TIER]
//   ┌─────────────────────────────────────────────────────────────────────┐
//   │ Reward Registry (nRewardLines)    Public Deltas (nPublicLines)     │
//   │ ─────────────────────────────     ────────────────────────────     │
//   │ (assetId, globalAcc) pairs        (assetId, amount, enabled)       │
//   │ On-chain yield accumulators       Deposits (+) / Withdrawals (-)   │
//   └─────────────────────────────────────────────────────────────────────┘
//                          │                           │
//                          │ rosterRewardLineSel       │ publicLineSlotSel
//                          ▼                           ▼
//   [PRIVATE ROUTING TIER]
//   ┌─────────────────────────────────────────────────────────────────────┐
//   │                    Roster (nRosterSlots)                            │
//   │                    ─────────────────────                            │
//   │    Private array of (assetId, enabled) slots for routing values    │
//   │    Each slot accumulates: inputs + public delta = outputs          │
//   └─────────────────────────────────────────────────────────────────────┘
//                          ▲                           ▲
//                          │ inNoteSlotSel             │ outNoteSlotSel
//                          │                           │
//   [NOTE TIER]
//   ┌─────────────────────────────────────────────────────────────────────┐
//   │ Input Notes (nInputNotes)         Output Notes (nOutputNotes)      │
//   │ ────────────────────────          ─────────────────────────        │
//   │ Notes being spent                 Notes being created              │
//   │ (prove ownership via nullifier)   (commit to new notes)            │
//   └─────────────────────────────────────────────────────────────────────┘
//
// ONE-HOT SELECTOR PATTERN
// ------------------------
// Routing between tiers uses one-hot selectors: binary arrays where exactly
// one bit is set (or zero bits if disabled). The OneHotValidator template
// enforces:
//   - Each selector bit is binary (0 or 1)
//   - Selected slot must be enabled
//   - Sum of bits equals expected count (0 or 1)
//   - Dot product with slot values equals the item's assetId
//
// This binds each note/line to exactly one roster slot with matching assetId,
// without revealing which slot was selected.
//
// VALUE CONSERVATION
// ------------------
// For each roster slot j, the circuit enforces:
//
//   Σ(input values routed to j) + publicDelta[j] = Σ(output amounts routed to j)
//
// Where input "value" includes accrued yield: amount + (globalAcc - noteAcc) * amount / 1e18
//
// TEMPLATE PARAMETERS
// -------------------
//   levels        - Merkle tree depth (e.g., 26 for ~67M leaves)
//   nInputNotes   - Number of input notes that can be spent
//   nOutputNotes  - Number of output notes that can be created
//   zeroLeaf      - Zero value for empty Merkle leaves
//   nRewardLines  - Size of the public reward registry (see below)
//   nPublicLines  - Number of public deposit/withdrawal lines (see below)
//   nRosterSlots  - Number of private routing slots (see below)
//
// PARAMETER DETAILS
// -----------------
//
// nRewardLines (Reward Registry Size)
//   The reward registry is a PUBLIC array of (assetId, globalAccumulator) pairs
//   provided as circuit inputs. It represents the on-chain state of yield
//   accumulators for supported assets.
//
//   Each roster slot selects exactly one reward registry line (via one-hot
//   rosterRewardLineSel) to fetch the current global accumulator for that asset.
//   This enables yield calculation: rewards = (globalAcc - noteAcc) * amount.
//
//   Typical value: 8 (supports 8 yield-bearing asset types)
//
// nPublicLines (Public Delta Lines)
//   Public delta lines represent VISIBLE value flows between the shielded pool
//   and public Solana accounts. Each line has:
//     - assetId: which asset is moving
//     - amount: positive for deposits, negative for withdrawals
//     - enabled: whether this line is active
//
//   These are PUBLIC inputs - the verifier sees exactly which assets and amounts
//   are entering or leaving the pool. Private transfers use amount=0 on all lines.
//
//   Typical value: 2 (supports depositing/withdrawing up to 2 assets per tx)
//
// nRosterSlots (Private Routing Slots)
//   The roster is a PRIVATE array of asset slots used to route value flows.
//   It acts as a "mixing board" where:
//     - Each slot has an assetId and enabled flag (private witnesses)
//     - Input notes select which slot to contribute value to
//     - Output notes select which slot to withdraw value from
//     - Public lines select which slot to add/subtract their delta
//
//   The roster hides which specific assets are involved in a transaction.
//   Even if public lines show a SOL deposit, the roster could contain
//   multiple assets for private-to-private transfers happening simultaneously.
//
//   Typical value: 4 (supports up to 4 different assets in one transaction)
//
//   Constraint: nRosterSlots >= max(assets in any single transaction)
//
// =============================================================================

include "circomlib/circuits/bitify.circom";
include "circomlib/circuits/comparators.circom";

include "./lib/keys.circom";
include "./lib/notes.circom";
include "./lib/rewards.circom";
include "./lib/merkle.circom";
include "./lib/one-hot.circom";

// =============================================================================
// MAIN CIRCUIT: Transaction
// =============================================================================
template Transaction(
    levels,         // Merkle tree depth
    nInputNotes,    // Number of input notes to spend
    nOutputNotes,   // Number of output notes to create
    zeroLeaf,       // Zero leaf value for Merkle tree
    nRewardLines,   // Size of public reward registry
    nPublicLines,   // Number of public deposit/withdrawal lines
    nRosterSlots    // Number of private asset routing slots
) {
    // =========================================================================
    // PUBLIC INPUTS
    // =========================================================================
    // These values are visible to the verifier (on-chain).

    // Merkle root of the commitment tree (proves input notes exist)
    signal input commitmentRoot;

    // Hash of transaction parameters (recipient, relayer, fees, deadline, etc.)
    // Binds the proof to specific external conditions
    signal input transactParamsHash;

    // --- Public Delta Lines ---
    // Visible deposits and withdrawals. Each line specifies an asset and amount.
    // Positive amounts = deposits into pool, negative = withdrawals from pool.
    // Note: publicAmount differs from ext_amount in TransactParams:
    //   - ext_amount = gross amount (fee charged on this)
    //   - publicAmount = pool boundary crossing:
    //       Deposits: net (after deposit fee)
    //       Withdrawals: gross (before withdrawal fee)
    signal input publicAssetId[nPublicLines];      // Asset ID for each line
    signal input publicAmount[nPublicLines];       // Pool boundary amount (see note above)

    // --- Nullifiers and Commitments ---
    // Nullifiers prove input notes are spent; commitments create output notes
    signal input nullifiers[nInputNotes];          // Nullifiers for spent notes
    signal input commitments[nOutputNotes];        // Commitments for new notes

    // --- Reward Registry ---
    // On-chain yield accumulators for supported assets.
    // Used to calculate accrued rewards when spending notes.
    signal input rewardAcc[nRewardLines];          // Current accumulator per asset
    signal input rewardAssetId[nRewardLines];      // Asset ID per registry line

    // =========================================================================
    // PRIVATE INPUTS: Roster and Selectors
    // =========================================================================
    // The roster is the private routing layer. Selectors bind notes/lines to slots.

    // --- Public Line Enabled Flags ---
    // Whether each public delta line is active (not in TransactProof - private witness)
    signal input publicLineEnabled[nPublicLines];  // Whether line is active (0 or 1)

    // --- Roster Definition ---
    // Private array of asset slots. Each slot can hold one asset type.
    signal input rosterAssetId[nRosterSlots];  // Asset ID for each slot (0 if disabled)
    signal input rosterEnabled[nRosterSlots];  // Whether slot is active (0 or 1)

    // --- One-Hot Selectors ---
    // Each selector is a binary array binding an item to exactly one roster slot.

    // Input note → roster slot (which slot receives this note's value?)
    signal input inNoteSlotSel[nInputNotes][nRosterSlots];

    // Output note → roster slot (which slot provides this note's value?)
    signal input outNoteSlotSel[nOutputNotes][nRosterSlots];

    // Public line → roster slot (which slot does this deposit/withdrawal affect?)
    signal input publicLineSlotSel[nPublicLines][nRosterSlots];

    // Roster slot → reward registry line (which registry entry has this asset's accumulator?)
    signal input rosterRewardLineSel[nRosterSlots][nRewardLines];

    // =========================================================================
    // PRIVATE INPUTS: Input Notes
    // =========================================================================
    // Full note data for notes being spent. Prover must know secret keys.

    signal input inNoteVersion[nInputNotes];      // Note format version
    signal input inNoteAssetId[nInputNotes];      // Asset type
    signal input inNoteAmount[nInputNotes];       // Principal amount (0 = dummy note)
    signal input inNoteAsk[nInputNotes];          // Spend authorizing secret key
    signal input inNoteNsk[nInputNotes];          // Nullifier secret key
    signal input inNoteBlinding[nInputNotes];     // Randomness for commitment
    signal input inNoteRewardAcc[nInputNotes];    // Accumulator when note was created
    signal input inNoteRewardRem[nInputNotes];    // Remainder from previous reward calc
    signal input inNoteRho[nInputNotes];          // Uniqueness parameter (position-independent nullifier)
    signal input inNotePathIndex[nInputNotes];    // Leaf index in Merkle tree
    signal input inNotePathElem[nInputNotes][levels];  // Merkle proof siblings

    // =========================================================================
    // PRIVATE INPUTS: Output Notes
    // =========================================================================
    // Note data for notes being created. Only public key needed (recipient).

    signal input outNoteVersion[nOutputNotes];    // Note format version
    signal input outNoteAssetId[nOutputNotes];    // Asset type
    signal input outNoteAmount[nOutputNotes];     // Amount (0 = dummy note)
    signal input outNotePk[nOutputNotes];         // Recipient's public key
    signal input outNoteBlinding[nOutputNotes];   // Randomness for commitment
    signal input outNoteRewardAcc[nOutputNotes];  // Current accumulator (for yield tracking)
    signal input outNoteRho[nOutputNotes];        // Uniqueness parameter (from spent note's nullifier)

    // =========================================================================
    // SECTION 1: Version Enforcement (Circuit Isolation)
    // =========================================================================
    // Enforce version = 0 for all notes in this circuit.
    // This prevents future circuits from creating notes consumable by this circuit.
    //
    // Future circuits MUST use version ≠ 0, ensuring:
    //   - Notes created by circuit A cannot be spent by circuit B
    //   - Clean upgrade path to programmable privacy with circuitId
    //
    // The version field is embedded in the note commitment:
    //   commitment = Poseidon(ZORB_DOMAIN, version, assetId, amount, pk, blinding, acc, rho)
    //
    // Since the commitment is used in nullifier derivation (position-independent model):
    //   nullifier = Poseidon(nk, rho, commitment)
    //
    // Different versions produce different commitments AND nullifiers,
    // providing complete isolation between circuits.
    // =========================================================================

    for (var n = 0; n < nInputNotes; n++) {
        inNoteVersion[n] === 0;
    }

    for (var n = 0; n < nOutputNotes; n++) {
        outNoteVersion[n] === 0;
    }

    // =========================================================================
    // SECTION 2: Boolean Constraints and Canonical Forms
    // =========================================================================
    // Ensure enabled flags are boolean and disabled items have zeroed fields.

    // Roster: enabled must be boolean, disabled slots must have assetId = 0
    component rosterEnabledBool[nRosterSlots];
    for (var j = 0; j < nRosterSlots; j++) {
        rosterEnabledBool[j] = AssertBool();
        rosterEnabledBool[j].in <== rosterEnabled[j];

        // If disabled (enabled=0), assetId must be 0: assetId * (1 - enabled) = 0
        rosterAssetId[j] * (1 - rosterEnabled[j]) === 0;
    }

    // Public lines: enabled must be boolean, disabled lines must have zeroed fields
    component publicEnabledBool[nPublicLines];
    for (var i = 0; i < nPublicLines; i++) {
        publicEnabledBool[i] = AssertBool();
        publicEnabledBool[i].in <== publicLineEnabled[i];

        // If disabled, both assetId and amount must be 0
        publicAssetId[i] * (1 - publicLineEnabled[i]) === 0;
        publicAmount[i]  * (1 - publicLineEnabled[i]) === 0;
    }

    // =========================================================================
    // SECTION 3: Roster Asset Uniqueness
    // =========================================================================
    // CRITICAL INVARIANT: Among enabled roster slots, no two may have the
    // same assetId. This constraint is fundamental to value conservation.
    //
    // WHY UNIQUENESS MATTERS
    // ----------------------
    // The one-hot selectors (inNoteSlotSel, outNoteSlotSel, publicLineSlotSel)
    // bind items to roster slots via dot product:
    //
    //   expectedDot <== noteAssetId
    //   actualDot = Σ_j (selector[j] × rosterAssetId[j])
    //
    // If two roster slots had the same assetId, a note could validly select
    // EITHER slot. This would break value conservation:
    //
    //   Example attack without uniqueness:
    //   - Roster slot 0: assetId = SOL, enabled = 1
    //   - Roster slot 1: assetId = SOL, enabled = 1  (duplicate!)
    //   - Input note (100 SOL) selects slot 0 → sumInBySlot[0] = 100
    //   - Output note (100 SOL) selects slot 1 → sumOutBySlot[1] = 100
    //   - Conservation per slot: slot 0 has 100 in, 0 out (FAILS)
    //                           slot 1 has 0 in, 100 out (FAILS)
    //
    // SECURITY GUARANTEE
    // ------------------
    // With uniqueness enforced, for any assetId X there exists exactly ONE
    // enabled roster slot j where rosterAssetId[j] = X. Therefore:
    //
    //   - ALL input notes with assetId X must route to slot j
    //   - ALL output notes with assetId X must route to slot j
    //   - ALL public lines with assetId X must route to slot j
    //
    // This forces all value flows for a given asset through the same slot,
    // ensuring conservation: sumIn[j] + publicDelta[j] = sumOut[j]
    //
    // The one-hot selector doesn't give the prover "choice" - the slot is
    // uniquely determined by the assetId. The selector is simply a ZK
    // mechanism to prove the binding without revealing which slot.
    // =========================================================================

    component rosterDupCheck[nRosterSlots * (nRosterSlots - 1) / 2];
    signal rosterEnabledProd[nRosterSlots * (nRosterSlots - 1) / 2];

    var dupIdx = 0;
    for (var a = 0; a < nRosterSlots - 1; a++) {
        for (var b = a + 1; b < nRosterSlots; b++) {
            // Check if slots a and b have the same assetId
            rosterDupCheck[dupIdx] = IsEqual();
            rosterDupCheck[dupIdx].in[0] <== rosterAssetId[a];
            rosterDupCheck[dupIdx].in[1] <== rosterAssetId[b];

            // Only enforce uniqueness if both slots are enabled
            rosterEnabledProd[dupIdx] <== rosterEnabled[a] * rosterEnabled[b];

            // If both enabled and equal: enabledProd * isEqual = 0 fails
            rosterEnabledProd[dupIdx] * rosterDupCheck[dupIdx].out === 0;
            dupIdx++;
        }
    }

    // =========================================================================
    // SECTION 4: Reward Registry Selection
    // =========================================================================
    // Each enabled roster slot must select exactly one reward registry line
    // with matching assetId. This fetches the global accumulator for yield calc.

    signal rosterGlobalAcc[nRosterSlots];  // Selected accumulator per slot
    signal rosterAccProducts[nRosterSlots][nRewardLines];
    component rosterRewardSelValidator[nRosterSlots];

    for (var j = 0; j < nRosterSlots; j++) {
        // Validate the one-hot selector
        rosterRewardSelValidator[j] = OneHotValidator(nRewardLines);
        rosterRewardSelValidator[j].bits <== rosterRewardLineSel[j];

        // All reward registry lines are always "enabled" as targets
        for (var k = 0; k < nRewardLines; k++) {
            rosterRewardSelValidator[j].enabled[k] <== 1;
        }

        // Selector values are the registry assetIds
        rosterRewardSelValidator[j].values <== rewardAssetId;

        // If roster slot is enabled, exactly 1 bit set; if disabled, 0 bits
        rosterRewardSelValidator[j].expectedSum <== rosterEnabled[j];

        // Dot product must equal roster's assetId (proves matching asset)
        rosterRewardSelValidator[j].expectedDot <== rosterAssetId[j];

        // Extract the selected global accumulator via dot product
        var acc = 0;
        for (var k = 0; k < nRewardLines; k++) {
            rosterAccProducts[j][k] <== rosterRewardLineSel[j][k] * rewardAcc[k];
            acc += rosterAccProducts[j][k];
        }
        rosterGlobalAcc[j] <== acc;
    }

    // =========================================================================
    // SECTION 5: Public Line Routing
    // =========================================================================
    // Each enabled public line must select exactly one roster slot with
    // matching assetId. This routes deposits/withdrawals to the correct slot.
    //
    // INTENTIONAL: MULTIPLE PUBLIC LINES MAY SHARE THE SAME ASSET ID
    // ---------------------------------------------------------------
    // Unlike the roster (which enforces uniqueness), multiple public lines
    // CAN have the same assetId. This is safe and intentional because:
    //
    //   1. Roster uniqueness forces all lines with assetId X to route to
    //      the same roster slot j (there's only one valid slot for X)
    //
    //   2. The amounts are aggregated into publicBySlot[j]:
    //      publicBySlot[j] = Σ_i (publicLineSlotSel[i][j] × publicAmount[i])
    //
    //   3. The aggregated sum participates in the single conservation equation:
    //      sumIn[j] + publicBySlot[j] = sumOut[j]
    //
    // This enables flexible transaction construction, e.g., a single transaction
    // could have multiple deposit operations for the same asset (though in practice
    // the on-chain program may combine them into a single public line).
    //
    // ON-CHAIN ENFORCEMENT: The verifying contract should validate that public
    // lines match the actual token transfers. The circuit only proves that the
    // prover's claimed public amounts balance with the private note flows.
    // =========================================================================

    component pubLineSelValidator[nPublicLines];
    for (var i = 0; i < nPublicLines; i++) {
        pubLineSelValidator[i] = OneHotValidator(nRosterSlots);
        pubLineSelValidator[i].bits <== publicLineSlotSel[i];
        pubLineSelValidator[i].enabled <== rosterEnabled;
        pubLineSelValidator[i].values <== rosterAssetId;

        // If line enabled: 1 bit set, dot = publicAssetId
        // If line disabled: 0 bits set, dot = 0
        pubLineSelValidator[i].expectedSum <== publicLineEnabled[i];
        pubLineSelValidator[i].expectedDot <== publicAssetId[i];
    }

    // Aggregate public amounts by roster slot
    // publicBySlot[j] = sum of publicAmount for all lines routed to slot j
    signal publicBySlot[nRosterSlots];
    signal pubProd[nRosterSlots][nPublicLines];

    for (var j = 0; j < nRosterSlots; j++) {
        var s = 0;
        for (var i = 0; i < nPublicLines; i++) {
            pubProd[j][i] <== publicLineSlotSel[i][j] * publicAmount[i];
            s += pubProd[j][i];
        }
        publicBySlot[j] <== s;
    }

    // =========================================================================
    // SECTION 6: Note Routing and Accumulator Validation
    // =========================================================================
    // Route each note to its roster slot and validate reward accumulator bounds.

    // --- Input Notes ---
    // Each non-zero input note selects exactly one roster slot.
    // The note's accumulator must be <= the current global accumulator.

    component inIsZero[nInputNotes];
    signal inNoteEnabled[nInputNotes];

    component inSelValidator[nInputNotes];
    signal inSelectedGlobalAcc[nInputNotes];
    signal inAccSelectProd[nInputNotes][nRosterSlots];

    component inAccLeq[nInputNotes];

    for (var n = 0; n < nInputNotes; n++) {
        // Note is enabled if amount != 0
        inIsZero[n] = IsZero();
        inIsZero[n].in <== inNoteAmount[n];
        inNoteEnabled[n] <== 1 - inIsZero[n].out;

        // Validate slot selection
        inSelValidator[n] = OneHotValidator(nRosterSlots);
        inSelValidator[n].bits <== inNoteSlotSel[n];
        inSelValidator[n].enabled <== rosterEnabled;
        inSelValidator[n].values <== rosterAssetId;
        inSelValidator[n].expectedSum <== inNoteEnabled[n];
        inSelValidator[n].expectedDot <== inNoteAssetId[n];

        // Get the global accumulator for the selected slot
        var acc = 0;
        for (var j = 0; j < nRosterSlots; j++) {
            inAccSelectProd[n][j] <== inNoteSlotSel[n][j] * rosterGlobalAcc[j];
            acc += inAccSelectProd[n][j];
        }
        inSelectedGlobalAcc[n] <== acc;

        // Accumulator bound: noteAcc <= globalAcc (can't claim future yield)
        inAccLeq[n] = LessEqThan(248);
        inAccLeq[n].in[0] <== inNoteRewardAcc[n];
        inAccLeq[n].in[1] <== inSelectedGlobalAcc[n];

        // Only enforce if note is enabled
        inAccLeq[n].out * inNoteEnabled[n] === inNoteEnabled[n];
    }

    // --- Output Notes ---
    // Each non-zero output note selects exactly one roster slot.
    // The note's accumulator must equal the current global accumulator.

    component outIsZero[nOutputNotes];
    signal outNoteEnabled[nOutputNotes];

    component outSelValidator[nOutputNotes];
    signal outSelectedGlobalAcc[nOutputNotes];
    signal outAccSelectProd[nOutputNotes][nRosterSlots];

    for (var n = 0; n < nOutputNotes; n++) {
        // Note is enabled if amount != 0
        outIsZero[n] = IsZero();
        outIsZero[n].in <== outNoteAmount[n];
        outNoteEnabled[n] <== 1 - outIsZero[n].out;

        // Validate slot selection
        outSelValidator[n] = OneHotValidator(nRosterSlots);
        outSelValidator[n].bits <== outNoteSlotSel[n];
        outSelValidator[n].enabled <== rosterEnabled;
        outSelValidator[n].values <== rosterAssetId;
        outSelValidator[n].expectedSum <== outNoteEnabled[n];
        outSelValidator[n].expectedDot <== outNoteAssetId[n];

        // Get the global accumulator for the selected slot
        var acc = 0;
        for (var j = 0; j < nRosterSlots; j++) {
            outAccSelectProd[n][j] <== outNoteSlotSel[n][j] * rosterGlobalAcc[j];
            acc += outAccSelectProd[n][j];
        }
        outSelectedGlobalAcc[n] <== acc;

        // Output notes must snapshot current accumulator (for future yield calc)
        // If enabled: outNoteRewardAcc must equal selectedGlobalAcc
        (outNoteRewardAcc[n] - outSelectedGlobalAcc[n]) * outNoteEnabled[n] === 0;
    }

    // =========================================================================
    // SECTION 7: Input Note Processing
    // =========================================================================
    // For each input note: compute commitment, nullifier, verify Merkle proof,
    // and calculate total value including accrued yield.

    signal inValue[nInputNotes];  // Total value = principal + accrued yield

    component inRewardCalc[nInputNotes];
    component inKeys[nInputNotes];
    component inCommit[nInputNotes];
    component inNullifier[nInputNotes];
    component inMerkle[nInputNotes];
    component inRootCheck[nInputNotes];
    component inAmtRange[nInputNotes];

    for (var tx = 0; tx < nInputNotes; tx++) {
        // Calculate total value: principal + yield
        // yield = (globalAcc - noteAcc) * amount / 1e18
        inRewardCalc[tx] = ComputeReward();
        inRewardCalc[tx].amount <== inNoteAmount[tx];
        inRewardCalc[tx].globalAccumulator <== inSelectedGlobalAcc[tx];
        inRewardCalc[tx].noteAccumulator <== inNoteRewardAcc[tx];
        inRewardCalc[tx].remainder <== inNoteRewardRem[tx];
        inValue[tx] <== inRewardCalc[tx].totalValue;

        // Derive public keys from secret keys
        inKeys[tx] = DeriveKeys();
        inKeys[tx].ask <== inNoteAsk[tx];
        inKeys[tx].nsk <== inNoteNsk[tx];

        // Compute note commitment (must match what's in the tree)
        inCommit[tx] = NoteCommitment();
        inCommit[tx].version <== inNoteVersion[tx];
        inCommit[tx].assetId <== inNoteAssetId[tx];
        inCommit[tx].amount <== inNoteAmount[tx];
        inCommit[tx].pk <== inKeys[tx].pk;
        inCommit[tx].blinding <== inNoteBlinding[tx];
        inCommit[tx].rewardAccumulator <== inNoteRewardAcc[tx];
        inCommit[tx].rho <== inNoteRho[tx];

        // Compute nullifier (position-independent: uses rho instead of pathIndices)
        // nullifier = Poseidon(nk, rho, commitment)
        inNullifier[tx] = ComputeNullifier();
        inNullifier[tx].nk <== inKeys[tx].nk;
        inNullifier[tx].rho <== inNoteRho[tx];
        inNullifier[tx].commitment <== inCommit[tx].commitment;
        inNullifier[tx].nullifier === nullifiers[tx];

        // Verify Merkle proof (note exists in commitment tree)
        inMerkle[tx] = MerkleProof(levels);
        inMerkle[tx].leaf <== inCommit[tx].commitment;
        inMerkle[tx].pathIndices <== inNotePathIndex[tx];
        for (var i = 0; i < levels; i++) {
            inMerkle[tx].pathElements[i] <== inNotePathElem[tx][i];
        }

        // Check root matches (only if note is non-zero)
        inRootCheck[tx] = ForceEqualIfEnabled();
        inRootCheck[tx].in[0] <== commitmentRoot;
        inRootCheck[tx].in[1] <== inMerkle[tx].root;
        inRootCheck[tx].enabled <== inNoteAmount[tx];

        // Range check: amount fits in 248 bits
        inAmtRange[tx] = Num2Bits(248);
        inAmtRange[tx].in <== inNoteAmount[tx];
    }

    // =========================================================================
    // SECTION 8: Output Note Processing
    // =========================================================================
    // For each output note: compute commitment and verify it matches public input.
    // Also enforces 1:1 pairing between output rho and input nullifiers.

    component outCommitCalc[nOutputNotes];
    component outAmtRange[nOutputNotes];

    for (var tx = 0; tx < nOutputNotes; tx++) {
        // Compute note commitment
        outCommitCalc[tx] = NoteCommitment();
        outCommitCalc[tx].version <== outNoteVersion[tx];
        outCommitCalc[tx].assetId <== outNoteAssetId[tx];
        outCommitCalc[tx].amount <== outNoteAmount[tx];
        outCommitCalc[tx].pk <== outNotePk[tx];
        outCommitCalc[tx].blinding <== outNoteBlinding[tx];
        outCommitCalc[tx].rewardAccumulator <== outNoteRewardAcc[tx];
        outCommitCalc[tx].rho <== outNoteRho[tx];

        // Commitment must match public input
        outCommitCalc[tx].commitment === commitments[tx];

        // Range check: amount fits in 248 bits
        outAmtRange[tx] = Num2Bits(248);
        outAmtRange[tx].in <== outNoteAmount[tx];
    }

    // =========================================================================
    // SECTION 8.1: Output Rho 1:1 Pairing Enforcement
    // =========================================================================
    // Position-independent nullifier model (Orchard-style):
    //   output[j].rho = nullifier[j]  for j < min(nInputNotes, nOutputNotes)
    //
    // This creates a chain: spent note's nullifier → new note's rho
    // Benefits:
    //   - Nullifiers don't depend on Merkle tree position
    //   - Notes can be inserted at any tree position
    //   - Simplifies wallet recovery (no position tracking)
    //
    // For dummy output notes (amount = 0), rho is unconstrained to allow
    // padding without requiring matching input nullifiers.
    //
    // SECURITY: The 1:1 pairing ensures each output rho is unique and
    // unpredictable (derived from the nullifier, which depends on nk).
    // This prevents rho collision attacks.
    // =========================================================================

    // Enforce 1:1 pairing for output indices that have corresponding inputs
    // If output is enabled (amount != 0) AND has a paired input:
    //   outNoteRho[j] must equal nullifiers[j]
    for (var j = 0; j < nOutputNotes; j++) {
        if (j < nInputNotes) {
            // For enabled output notes with paired inputs: rho === nullifier
            // (outNoteRho[j] - nullifiers[j]) * outNoteEnabled[j] === 0
            (outNoteRho[j] - nullifiers[j]) * outNoteEnabled[j] === 0;
        }
        // For output indices >= nInputNotes, rho is unconstrained
        // (these are "extra" outputs beyond input count, or dummy notes)
    }

    // =========================================================================
    // SECTION 9: Nullifier Uniqueness
    // =========================================================================
    // Prevent double-spending within the same transaction.
    // Cross-transaction double-spend is prevented by on-chain nullifier set.

    component nfUnique = EnforceNullifierUniqueness(nInputNotes);
    nfUnique.nullifiers <== nullifiers;

    // =========================================================================
    // SECTION 10: Value Conservation
    // =========================================================================
    // For each roster slot, total input value + public delta = total output amount.
    // This is enforced per-slot to prevent cross-asset value transfer.

    signal sumInBySlot[nRosterSlots];
    signal sumOutBySlot[nRosterSlots];

    signal inValueProd[nRosterSlots][nInputNotes];
    signal outAmtProd[nRosterSlots][nOutputNotes];

    for (var j = 0; j < nRosterSlots; j++) {
        // Sum input values routed to this slot
        var sIn = 0;
        for (var n = 0; n < nInputNotes; n++) {
            inValueProd[j][n] <== inNoteSlotSel[n][j] * inValue[n];
            sIn += inValueProd[j][n];
        }
        sumInBySlot[j] <== sIn;

        // Sum output amounts routed to this slot
        var sOut = 0;
        for (var n = 0; n < nOutputNotes; n++) {
            outAmtProd[j][n] <== outNoteSlotSel[n][j] * outNoteAmount[n];
            sOut += outAmtProd[j][n];
        }
        sumOutBySlot[j] <== sOut;

        // Conservation: inputs + public delta = outputs
        sumInBySlot[j] + publicBySlot[j] === sumOutBySlot[j];
    }

    // =========================================================================
    // SECTION 11: Public Input Range Binding
    // =========================================================================
    // Constrain public inputs to valid field element ranges.
    // This prevents malleability attacks via overflowing values.

    component bindTxParams = Num2Bits(254);
    bindTxParams.in <== transactParamsHash;

    component bindPubAssetId[nPublicLines];
    // NOTE: publicAmount does NOT have a range check because withdrawals use
    // field representation of negative amounts (FIELD_SIZE - |amount|), which
    // results in ~254-bit values. The value conservation check (Section 9)
    // ensures publicAmount is constrained by the input/output balance.
    for (var i = 0; i < nPublicLines; i++) {
        bindPubAssetId[i] = Num2Bits(254);
        bindPubAssetId[i].in <== publicAssetId[i];
    }

    component bindRewardAssetId[nRewardLines];
    component bindRewardAcc[nRewardLines];
    for (var k = 0; k < nRewardLines; k++) {
        bindRewardAssetId[k] = Num2Bits(254);
        bindRewardAssetId[k].in <== rewardAssetId[k];

        bindRewardAcc[k] = Num2Bits(248);
        bindRewardAcc[k].in <== rewardAcc[k];
    }

    component bindNullifier[nInputNotes];
    component bindCommitment[nOutputNotes];
    for (var n = 0; n < nInputNotes; n++) {
        bindNullifier[n] = Num2Bits(254);
        bindNullifier[n].in <== nullifiers[n];
    }
    for (var n = 0; n < nOutputNotes; n++) {
        bindCommitment[n] = Num2Bits(254);
        bindCommitment[n].in <== commitments[n];
    }
}
