pragma circom 2.0.0;

include "./lib/constants.circom";
include "./transaction.circom";

// =============================================================================
// TRANSACTION-16 CIRCUIT - Multi-Asset Shielded Pool (16 inputs, 2 outputs)
// =============================================================================
//
// Batch consolidation circuit for aggregating many small notes.
//
// Template Parameters:
//   levels = 26           Merkle tree depth (supports ~67M leaves)
//   nInputNotes = 16      Input notes per transaction (high for consolidation)
//   nOutputNotes = 2      Output notes per transaction
//   zeroLeaf              COMMITMENT_TREE_ZERO_LEAF() from lib/constants.circom
//   nRewardLines = 2      Reduced reward registry (manages circuit size)
//   nPublicLines = 2      Max public deposit/withdrawal lines
//   nRosterSlots = 4      Private routing slots for multi-asset
//
// Public Inputs:
//   commitmentRoot        Merkle root of commitment tree
//   transactParamsHash    Hash of tx params (recipient, relayer, fees, deadline)
//   publicAssetId[2]      Asset IDs for deposits/withdrawals (0 = disabled)
//   publicAmount[2]       Deposit (+) or withdrawal (-) amounts
//   nullifiers[16]        Nullifiers for spent notes
//   commitments[2]        Commitments for new output notes
//   rewardAcc[2]          Current reward accumulator per registry line
//   rewardAssetId[2]      Asset ID per reward registry line
//
// =============================================================================
component main {
    public [
        commitmentRoot,
        transactParamsHash,
        publicAssetId,
        publicAmount,
        nullifiers,
        commitments,
        rewardAcc,
        rewardAssetId
    ]
} = Transaction(
    MERKLE_TREE_HEIGHT(),           // levels
    16,                             // nInputNotes
    2,                              // nOutputNotes
    COMMITMENT_TREE_ZERO_LEAF(),    // zeroLeaf
    2,                              // nRewardLines (reduced for 16 inputs)
    2,                              // nPublicLines
    4                               // nRosterSlots
);
