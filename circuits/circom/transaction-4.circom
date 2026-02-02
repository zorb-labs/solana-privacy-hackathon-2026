pragma circom 2.0.0;

include "./lib/constants.circom";
include "./transaction.circom";

// =============================================================================
// TRANSACTION-4 CIRCUIT - Multi-Asset Shielded Pool (4 inputs, 4 outputs)
// =============================================================================
//
// Extended transaction circuit for higher-throughput operations.
//
// Template Parameters:
//   levels = 26           Merkle tree depth (supports ~67M leaves)
//   nInputNotes = 4       Input notes per transaction
//   nOutputNotes = 4      Output notes per transaction
//   zeroLeaf              COMMITMENT_TREE_ZERO_LEAF() from lib/constants.circom
//   nRewardLines = 8      Reward registry entries for yield accrual
//   nPublicLines = 2      Max public deposit/withdrawal lines
//   nRosterSlots = 4      Private routing slots for multi-asset
//
// Public Inputs:
//   commitmentRoot        Merkle root of commitment tree
//   transactParamsHash    Hash of tx params (recipient, relayer, fees, deadline)
//   publicAssetId[2]      Asset IDs for deposits/withdrawals (0 = disabled)
//   publicAmount[2]       Deposit (+) or withdrawal (-) amounts
//   nullifiers[4]         Nullifiers for spent notes
//   commitments[4]        Commitments for new output notes
//   rewardAcc[8]          Current reward accumulator per registry line
//   rewardAssetId[8]      Asset ID per reward registry line
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
    4,                              // nInputNotes
    4,                              // nOutputNotes
    COMMITMENT_TREE_ZERO_LEAF(),    // zeroLeaf
    8,                              // nRewardLines
    2,                              // nPublicLines
    4                               // nRosterSlots
);
