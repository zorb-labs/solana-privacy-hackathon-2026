use panchor::prelude::*;
pub mod commitment_tree;
pub mod deposit_escrow;
pub mod global_config;
pub mod nullifier;
pub mod nullifier_epoch_root;
pub mod nullifier_tree;
pub mod pool_config;
pub mod pool_traits;
pub mod receipt_tree;
pub mod transact_session;

#[cfg(any(feature = "localnet", feature = "test-mode", test))]
use pinocchio::pubkey::Pubkey;

/// Account discriminators for the Shielded Pool program.
///
/// Each discriminator uniquely identifies an account type. The discriminator
/// is stored as the first 8 bytes of account data.
///
/// # Ranges (per discriminator-standard.md)
/// - **0-15**: Core accounts (global config, pools, primary state)
/// - **16-31**: User accounts (reserved for future use)
/// - **32-63**: Tree accounts (reserved for future use)
/// - **64-127**: Ephemeral accounts (reserved for future use)
#[account_type]
pub enum ShieldedPoolAccount {
    // =========================================================================
    // Core Accounts (0-15) - Global config, pools, primary state
    // =========================================================================
    /// Global configuration singleton
    GlobalConfig = 0,
    /// Commitment merkle tree
    CommitmentTree = 1,
    /// Receipt merkle tree
    ReceiptTree = 2,
    /// Token configuration (per mint)
    TokenConfig = 3,
    /// Nullifier account (per nullifier value)
    Nullifier = 4,
    // Reserved: 5-7
    /// Transact session (per user session)
    TransactSession = 8,
    // Reserved: 9
    /// Nullifier indexed tree
    NullifierIndexedTree = 10,
    // Reserved: 11
    /// Nullifier epoch root (per nullifier epoch)
    NullifierEpochRoot = 12,
    /// Unified SOL pool configuration
    UnifiedSolPoolConfig = 13,
    /// LST configuration (per LST mint)
    LstConfig = 14,
    /// Pool config (per asset_id) - hub's routing configuration
    PoolConfig = 15,
    // =========================================================================
    // User Accounts (16-31) - Reserved for future use
    // =========================================================================
    /// Deposit escrow for relayer-assisted deposits
    DepositEscrow = 16,

    // =========================================================================
    // Tree Accounts (32-63) - Reserved for future use
    // =========================================================================

    // =========================================================================
    // Ephemeral Accounts (64-127) - Reserved for future use
    // =========================================================================
}

// Re-export events from the events module for backwards compatibility
pub use crate::events::{
    CommitmentData, EventType, NEW_COMMITMENT_HEADER_SIZE, NewCommitmentEvent, NewCommitmentHeader,
    NewNullifierEvent, NewReceiptEvent, Receipt, RECEIPT_VERSION,
};

pub use commitment_tree::CommitmentMerkleTree;
pub use global_config::GlobalConfig;
pub use nullifier_epoch_root::{NullifierEpochRoot, MIN_PROVABLE_NULLIFIER_EPOCHS};
pub use nullifier::Nullifier;
pub use nullifier_tree::{
    CLEANUP_GRACE_EPOCHS, IndexedLeaf, MAX_NULLIFIER_VALUE, MIN_SLOTS_PER_NULLIFIER_EPOCH,
    NULLIFIER_TREE_HEIGHT, NullifierIndexedTree,
};

pub use pool_traits::RATE_PRECISION;

pub use pool_config::{PoolConfig, PoolType as HubPoolType};
pub use receipt_tree::{RECEIPT_TREE_HEIGHT, ReceiptMerkleTree};
pub use transact_session::{
    MAX_SESSION_DATA_LEN, SESSION_EXPIRY_SLOTS, TRANSACT_SESSION_HEADER_SIZE, TransactSession,
};

// Re-export types from pool programs
// These are the canonical types owned by the respective pool programs
pub use token_pool::{
    TokenPoolConfig, VAULT_SEED, find_token_pool_config_pda,
    find_vault_pda as find_vault_token_account,
};

pub use unified_sol_pool::{
    LST_CONFIG_SEED, LST_VAULT_SEED, LstConfig, UNIFIED_SOL_ASSET_ID, UNIFIED_SOL_POOL_CONFIG_SEED,
    UnifiedSolPoolConfig, find_lst_config_pda, find_lst_vault_pda,
    find_unified_sol_pool_config_pda,
};

pub use commitment_tree::{COMMITMENT_TREE_HEIGHT, ROOT_HISTORY_SIZE};
pub use deposit_escrow::DepositEscrow;

#[cfg(any(feature = "localnet", feature = "test-mode", test))]
pub const ADMIN_PUBKEY: Option<Pubkey> = None;
