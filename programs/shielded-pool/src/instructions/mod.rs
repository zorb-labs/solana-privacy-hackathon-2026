//! Instruction definitions for the shielded pool program.
//!
//! Uses panchor's `#[instructions]` macro for automatic dispatch.
//!
//! # Instruction Categories
//!
//! Instructions are organized by domain with discriminator ranges:
//! - **0-31**: Transact instructions (private transfers)
//! - **32-63**: Utility instructions (hashing, logging)
//! - **64-127**: Nullifier tree instructions
//! - **192-255**: Admin instructions

use panchor::prelude::*;

// Organized instruction modules by domain
pub mod admin;
pub mod deposit_escrow;
pub mod nullifier_tree;
pub mod transact;
pub mod util;

// Shared types
pub mod types;

pub use types::{
    N_INS, N_OUTS, N_PUBLIC_LINES, N_REWARD_LINES, NullifierBatchInsertData,
    NullifierBatchInsertHeader, NullifierBatchInsertProof, NullifierNonMembershipProofData,
    TransactParams, TransactProofData,
};

// Re-export accounts and data structs
pub use admin::{
    AcceptAuthorityAccounts, InitializeAccounts, RegisterTokenPoolAccounts,
    RegisterUnifiedSolPoolAccounts, SetPoolConfigActiveAccounts, SetPoolConfigActiveData,
    SetPoolPausedAccounts, SetPoolPausedData, TransferAuthorityAccounts,
};
pub use deposit_escrow::{
    CloseDepositEscrowAccounts, CloseDepositEscrowData, InitDepositEscrowAccounts,
    InitDepositEscrowData,
};
pub use nullifier_tree::{
    AdvanceEarliestProvableEpochAccounts, AdvanceEarliestProvableEpochData,
    AdvanceNullifierEpochAccounts, CloseInsertedNullifierAccounts, CloseNullifierEpochRootAccounts,
    NullifierBatchInsertAccounts,
};
pub use transact::{
    CloseTransactSessionAccounts, ExecuteTransactAccounts, ExecuteTransactData,
    InitTransactSessionAccounts, InitTransactSessionData, SlotPoolType,
    UploadTransactChunkAccounts,
};
pub use util::{
    LogAccounts, PoseidonHashAccounts, PoseidonHashData, TestGroth16Accounts, TestGroth16Data,
};

// Re-export handler functions for #[instructions] macro
// The macro expects process_* functions to be in scope
pub use admin::{
    process_accept_authority, process_initialize, process_register_token_pool,
    process_register_unified_sol_pool, process_set_pool_config_active, process_set_pool_paused,
    process_transfer_authority,
};
pub use deposit_escrow::{process_close_deposit_escrow, process_init_deposit_escrow};
pub use nullifier_tree::{
    process_advance_earliest_provable_epoch, process_advance_nullifier_epoch,
    process_close_inserted_nullifier, process_close_nullifier_epoch_root,
    process_nullifier_batch_insert,
};
pub use transact::{
    process_close_transact_session, process_execute_transact, process_init_transact_session,
    process_upload_transact_chunk,
};
pub use util::{process_log, process_poseidon_hash, process_test_groth16};

/// Shielded pool instruction set.
///
/// Discriminators are organized by domain:
/// - Transact: 0-31
/// - Utility: 32-63
/// - Nullifier Tree: 64-127
/// - Admin: 192-255
#[instructions]
pub enum ShieldedPoolInstruction {
    // =========================================================================
    // Transact Instructions (0-31) - Private transfers
    // =========================================================================
    /// Initialize a transact session account for chunked proof upload.
    /// Creates a temporary account to store proof data across multiple transactions.
    #[handler(data, accounts = InitTransactSessionAccounts)]
    InitTransactSession = 0,

    /// Upload a chunk of transaction data to the session account.
    /// Proof data is uploaded in chunks due to transaction size limits.
    #[handler(raw_data, accounts = UploadTransactChunkAccounts)]
    UploadTransactChunk = 1,

    /// Execute shielded transaction using uploaded proof data.
    /// Verifies the ZK proof, updates merkle trees, and transfers tokens.
    /// Uses wrapper accounts struct for panchor compatibility.
    #[handler(data, accounts = ExecuteTransactAccounts)]
    ExecuteTransact = 2,

    /// Close a transact session account and reclaim rent.
    #[handler(accounts = CloseTransactSessionAccounts)]
    CloseTransactSession = 3,

    // =========================================================================
    // Utility Instructions (32-63)
    // =========================================================================
    /// Compute Poseidon hash (utility instruction for testing/verification).
    #[handler(data, accounts = PoseidonHashAccounts)]
    PoseidonHash = 32,

    /// Log event data via CPI self-invocation.
    /// Used internally to emit structured events.
    #[handler(raw_data, accounts = LogAccounts)]
    Log = 33,

    /// Test Groth16 proof verification (utility instruction).
    /// Verifies a proof against the nullifier batch insertion VK (batch size 4).
    /// Useful for testing that Groth16 verification works correctly on-chain.
    #[handler(data, accounts = TestGroth16Accounts)]
    TestGroth16 = 34,

    // =========================================================================
    // Nullifier Tree Instructions (64-127)
    // Discriminators 64 and 67 were removed (InitNullifierTree, SingleInsertNullifier)
    // =========================================================================
    /// Advance the nullifier tree epoch for batch finalization.
    #[handler(accounts = AdvanceNullifierEpochAccounts)]
    AdvanceNullifierEpoch = 65,

    /// Close a nullifier PDA after insertion is finalized.
    #[handler(accounts = CloseInsertedNullifierAccounts)]
    CloseInsertedNullifier = 66,

    /// Insert a batch of nullifiers using a ZK proof.
    #[handler(raw_data, accounts = NullifierBatchInsertAccounts)]
    NullifierBatchInsert = 68,

    /// Advance the earliest provable epoch to prune old roots.
    #[handler(data, accounts = AdvanceEarliestProvableEpochAccounts)]
    AdvanceEarliestProvableEpoch = 69,

    /// Close a NullifierEpochRoot PDA after nullifier epoch is no longer provable.
    #[handler(accounts = CloseNullifierEpochRootAccounts)]
    CloseNullifierEpochRoot = 70,

    // Discriminators 71-72 reserved (batch close instructions removed)

    // =========================================================================
    // Escrow Instructions (128-159) - Relayer-assisted deposits
    // =========================================================================
    /// Initialize a deposit escrow for relayer-assisted deposits.
    /// Creates escrow account, vault ATA, and transfers tokens from depositor.
    #[handler(data, accounts = InitDepositEscrowAccounts)]
    InitDepositEscrow = 128,

    /// Close a deposit escrow and reclaim tokens after expiry.
    /// Returns tokens and rent to the original depositor.
    #[handler(data, accounts = CloseDepositEscrowAccounts)]
    CloseDepositEscrow = 129,

    // =========================================================================
    // Admin Instructions (192-255) - Pool initialization and configuration
    // =========================================================================
    /// Initialize a new shielded pool with merkle tree and global config.
    #[handler(accounts = InitializeAccounts)]
    Initialize = 192,

    /// Set the paused state for the pool.
    #[handler(data, accounts = SetPoolPausedAccounts)]
    SetPoolPaused = 193,

    /// Register a token pool with the hub.
    /// Creates PoolConfigAccount linking to an existing TokenPoolConfig.
    #[handler(accounts = RegisterTokenPoolAccounts)]
    RegisterTokenPool = 194,

    /// Register the unified SOL pool with the hub.
    /// Creates PoolConfigAccount linking to an existing UnifiedSolPoolConfig.
    #[handler(accounts = RegisterUnifiedSolPoolAccounts)]
    RegisterUnifiedSolPool = 195,

    /// Set the active state for a pool config.
    /// Enables or disables pool routing for an asset.
    #[handler(data, accounts = SetPoolConfigActiveAccounts)]
    SetPoolConfigActive = 196,

    /// Initiate two-step authority transfer by setting pending_authority.
    /// The new authority must call AcceptAuthority to complete.
    #[handler(accounts = TransferAuthorityAccounts)]
    TransferAuthority = 197,

    /// Complete two-step authority transfer by accepting pending authority role.
    /// Must be called by the pending_authority address.
    #[handler(accounts = AcceptAuthorityAccounts)]
    AcceptAuthority = 198,
}
