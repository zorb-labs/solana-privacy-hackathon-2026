//! Structured account handling for execute_transact instruction.
//!
//! This module provides type-safe account extraction and validation for multi-asset
//! shielded transactions with DYNAMIC per-asset account counts based on pool type.
//!
//! # Pool Loading Requirements
//!
//! The `unique_asset_count` instruction data must account for ALL unique asset_ids
//! that appear in the transaction:
//!
//! 1. **Public Asset IDs** (N_PUBLIC_LINES = 2): All non-zero `publicAssetId[i]`
//!    where `ext_amounts[i] != 0` (active public flow slots)
//!
//! 2. **Reward Asset IDs** (N_REWARD_LINES = 8): All non-zero `rewardAssetId[i]`
//!    values in the proof (for reward accumulator validation)
//!
//! The union of these two sets determines which pools must be loaded. Multiple slots
//! may reference the same asset_id (e.g., multiple reward lines for the same pool),
//! but each unique asset_id only needs to be loaded once.
//!
//! ## Example
//!
//! A transaction with:
//! - `publicAssetId[0]` = USDC (asset_id: A), `ext_amounts[0]` = 100
//! - `publicAssetId[1]` = 0 (unused)
//! - `rewardAssetId[0..3]` = SOL (asset_id: B)
//! - `rewardAssetId[4..7]` = 0 (unused)
//!
//! Requires `unique_asset_count = 2` (pools for asset_id A and B)
//!
//! # Account Layout (Panchor Pattern)
//!
//! ## Fixed Accounts (15 accounts in panchor struct)
//! | Index | Account | W | S | Description |
//! |-------|---------|---|---|-------------|
//! | 0 | transact_session | W | - | Session PDA with proof data |
//! | 1 | commitment_tree | W | - | Commitment merkle tree |
//! | 2 | receipt_tree | W | - | Receipt merkle tree |
//! | 3 | nullifier_indexed_tree | W | - | Nullifier indexed merkle tree |
//! | 4 | epoch_root_pda | - | - | Epoch root PDA (optional) |
//! | 5 | global_config | - | - | Global pool config |
//! | 6-9 | nullifiers[N_INS] | W | - | Nullifier PDAs (4 slots) |
//! | 10 | depositor | - | S | Deposit authorizer (conditional) |
//! | 11 | relayer | - | S | Relayer (conditional signer) |
//! | 12 | token_program | - | - | SPL Token program |
//! | 13 | system_program | - | - | System program |
//! | 14 | payer | W | S | Rent payer |
//!
//! ## Dynamic Accounts (remaining_accounts, based on unique_asset_count)
//! Pool accounts loaded based on pool_type from PoolConfig:
//! - Token pool: 3 accounts (pool_config, token_pool_config, vault)
//! - UnifiedSol pool: 4 accounts (pool_config, unified_sol_pool_config, lst_config, vault)
//!
//! The asset_map is built from these accounts, keyed by asset_id for lookup.

use crate::{
    errors::ShieldedPoolError,
    state::{HubPoolType, PoolConfig},
};
use alloc::collections::BTreeMap;
use panchor::AccountLoader;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

// ============================================================================
// Reward Config and Slot Accounts
// ============================================================================
//
// These structures replace AssetMap with two separate structures:
// 1. RewardConfigMap - for reward accumulator validation (no vault needed)
// 2. SlotAccounts - for public slot CPI operations (includes vault + user tokens)
//
// This separation:
// - Reduces account loading for reward-only assets (2 accounts instead of 3-4)
// - Properly handles unified SOL with multiple LSTs (slot-indexed, not asset_id keyed)

// ============================================================================
// Reward Config (for accumulator validation - no vault needed)
// ============================================================================

/// Pool type indicator for slot account loading.
///
/// Determines how many accounts to load for each public slot:
/// - `None (0)`: Inactive slot, 0 accounts
/// - `Token (1)`: 8 accounts (3 pool + 3 escrow + 2 user tokens)
/// - `UnifiedSol (2)`: 9 accounts (4 pool + 3 escrow + 2 user tokens)
///
/// # Account Layout Changes (v2 - Per-Slot Escrow)
///
/// All deposits now go through escrow accounts. The depositor_token field is replaced
/// with escrow accounts (escrow, escrow_vault_authority, escrow_token). This enables
/// relayer-assisted single-tx UX where:
/// 1. User creates escrow with tokens bound to a proof_hash
/// 2. Relayer executes transact with escrow accounts in each deposit slot
/// 3. Pool deposit CPI uses invoke_signed with escrow_vault_authority as signer
///
/// # IDL Representation
///
/// This enum maps to `ExecuteTransactData.slot_pool_type: [u8; 2]`.
/// Clients should use these discriminant values for type-safe slot configuration.
///
/// Note: panchor's `IdlType` derive doesn't support enums, so this type is
/// manually included in the IDL types section during generation.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlotPoolType {
    /// No pool accounts for this slot (inactive slot).
    None = 0,
    /// Token pool: 8 accounts (3 pool + 3 escrow + 2 user tokens).
    Token = 1,
    /// Unified SOL pool: 9 accounts (4 pool + 3 escrow + 2 user tokens).
    UnifiedSol = 2,
}

impl SlotPoolType {
    /// Convert from u8, returning None for invalid values.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(SlotPoolType::None),
            1 => Some(SlotPoolType::Token),
            2 => Some(SlotPoolType::UnifiedSol),
            _ => None,
        }
    }

    /// Number of accounts this pool type consumes.
    pub fn account_count(&self) -> usize {
        match self {
            SlotPoolType::None => 0,
            // pool_config, token_pool_config, vault + escrow, escrow_vault_authority, escrow_token + recipient_token, relayer_token + pool_program
            SlotPoolType::Token => 9,
            // pool_config, unified_sol_pool_config, lst_config, vault + escrow, escrow_vault_authority, escrow_token + recipient_token, relayer_token + pool_program
            SlotPoolType::UnifiedSol => 10,
        }
    }
}

/// Token reward config (1 account needed for validation).
/// Only what's needed for reward accumulator validation - no vault.
/// Note: pool_config is read to determine pool type but not stored.
#[derive(Clone, Copy)]
pub struct TokenRewardConfig<'a> {
    /// TokenPoolConfig (contains asset_id, is_active, reward_accumulator).
    pub token_pool_config: &'a AccountInfo,
}

/// Unified SOL reward config (1 account needed for validation).
/// Only what's needed for reward accumulator validation - no vault or lst_config.
/// Note: reward_accumulator lives in UnifiedSolPoolConfig, not per-LST.
/// Note: pool_config is read to determine pool type but not stored.
#[derive(Clone, Copy)]
pub struct UnifiedSolRewardConfig<'a> {
    /// UnifiedSolPoolConfig (contains asset_id, is_active, reward_accumulator).
    pub unified_sol_pool_config: &'a AccountInfo,
}

/// Enum for reward config based on pool type.
/// Keyed by asset_id in RewardConfigMap.
#[derive(Clone, Copy)]
pub enum RewardConfig<'a> {
    Token(TokenRewardConfig<'a>),
    UnifiedSol(UnifiedSolRewardConfig<'a>),
}


/// Reward config map: asset_id -> RewardConfig.
/// Used for N_REWARD_LINES accumulator validation.
pub type RewardConfigMap<'a> = BTreeMap<[u8; 32], RewardConfig<'a>>;

/// Result of building reward config map.
pub struct BuildRewardConfigResult<'a> {
    pub reward_config_map: RewardConfigMap<'a>,
    pub accounts_consumed: usize,
}

// ============================================================================
// Slot Accounts (for public slot CPI - includes vault + user tokens)
// ============================================================================

/// Token slot accounts (9 accounts).
/// Pool accounts + escrow accounts + user token accounts + pool program.
///
/// # Account Layout
/// ```text
/// INDEX   ACCOUNT              OWNER              PURPOSE
/// [0]     pool_config          shielded-pool      Hub routing
/// [1]     token_pool_config    token-pool         CPI signer
/// [2]     vault                SPL Token          Pool funds
/// [3]     escrow               shielded-pool      Escrow PDA (deposit binding)
/// [4]     escrow_vault_auth    (PDA)              Signs escrow transfers
/// [5]     escrow_token         SPL Token          Escrow vault (deposit source)
/// [6]     recipient_token      SPL Token          Withdrawal destination
/// [7]     relayer_token        SPL Token          Relayer fee destination
/// [8]     pool_program         (Executable)       Token pool program for CPI
/// ```
#[derive(Clone, Copy)]
pub struct TokenSlotAccounts<'a> {
    // Pool accounts (3)
    /// Hub's PoolConfig for routing.
    pub pool_config: &'a AccountInfo,
    /// TokenPoolConfig.
    pub token_pool_config: &'a AccountInfo,
    /// Vault token account.
    pub vault: &'a AccountInfo,
    // Escrow accounts (3)
    /// Escrow PDA: ["deposit_escrow", depositor, nonce].
    /// Binds deposit tokens to a specific proof_hash.
    pub escrow: &'a AccountInfo,
    /// Escrow vault authority PDA: ["escrow_vault_authority", escrow].
    /// Signs transfers from escrow_token.
    pub escrow_vault_authority: &'a AccountInfo,
    /// Escrow vault token account (ATA of escrow_vault_authority).
    /// Source for deposit transfers.
    pub escrow_token: &'a AccountInfo,
    // User token accounts (2)
    /// Recipient's token account.
    pub recipient_token: &'a AccountInfo,
    /// Relayer's token account.
    pub relayer_token: &'a AccountInfo,
    // Program account (1)
    /// Token pool program for CPI.
    pub pool_program: &'a AccountInfo,
}

/// Unified SOL slot accounts (10 accounts).
/// Pool accounts + escrow accounts + user token accounts + pool program.
///
/// # Account Layout
/// ```text
/// INDEX   ACCOUNT                 OWNER              PURPOSE
/// [0]     pool_config             shielded-pool      Hub routing
/// [1]     unified_sol_pool_config unified-sol-pool   Pool config (singleton)
/// [2]     lst_config              unified-sol-pool   LST-specific (exchange rate)
/// [3]     vault                   SPL Token          LST vault
/// [4]     escrow                  shielded-pool      Escrow PDA (deposit binding)
/// [5]     escrow_vault_auth       (PDA)              Signs escrow transfers
/// [6]     escrow_token            SPL Token          Escrow vault (deposit source)
/// [7]     recipient_token         SPL Token          Withdrawal destination
/// [8]     relayer_token           SPL Token          Relayer fee destination
/// [9]     pool_program            (Executable)       Unified SOL pool program for CPI
/// ```
#[derive(Clone, Copy)]
pub struct UnifiedSolSlotAccounts<'a> {
    // Pool accounts (4)
    /// Hub's PoolConfig for routing.
    pub pool_config: &'a AccountInfo,
    /// UnifiedSolPoolConfig.
    pub unified_sol_pool_config: &'a AccountInfo,
    /// LstConfig (contains lst_mint, exchange_rate).
    pub lst_config: &'a AccountInfo,
    /// Vault token account (LST-specific).
    pub vault: &'a AccountInfo,
    // Escrow accounts (3)
    /// Escrow PDA: ["deposit_escrow", depositor, nonce].
    /// Binds deposit tokens to a specific proof_hash.
    pub escrow: &'a AccountInfo,
    /// Escrow vault authority PDA: ["escrow_vault_authority", escrow].
    /// Signs transfers from escrow_token.
    pub escrow_vault_authority: &'a AccountInfo,
    /// Escrow vault token account (ATA of escrow_vault_authority).
    /// Source for deposit transfers.
    pub escrow_token: &'a AccountInfo,
    // User token accounts (2)
    /// Recipient's token account.
    pub recipient_token: &'a AccountInfo,
    /// Relayer's token account.
    pub relayer_token: &'a AccountInfo,
    // Program account (1)
    /// Unified SOL pool program for CPI.
    pub pool_program: &'a AccountInfo,
}

/// Enum for slot accounts based on pool type.
/// Indexed by slot number (0, 1) in the public asset slots array.
#[derive(Clone, Copy)]
pub enum SlotAccounts<'a> {
    Token(TokenSlotAccounts<'a>),
    UnifiedSol(UnifiedSolSlotAccounts<'a>),
}

impl<'a> SlotAccounts<'a> {
    /// Get the hub pool config account.
    #[inline]
    pub fn pool_config(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.pool_config,
            SlotAccounts::UnifiedSol(u) => u.pool_config,
        }
    }

    /// Get the vault account.
    #[inline]
    pub fn vault(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.vault,
            SlotAccounts::UnifiedSol(u) => u.vault,
        }
    }

    /// Get the escrow PDA account.
    #[inline]
    pub fn escrow(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.escrow,
            SlotAccounts::UnifiedSol(u) => u.escrow,
        }
    }

    /// Get the escrow vault authority PDA (signs transfers from escrow).
    #[inline]
    pub fn escrow_vault_authority(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.escrow_vault_authority,
            SlotAccounts::UnifiedSol(u) => u.escrow_vault_authority,
        }
    }

    /// Get the escrow token account (source for deposits).
    #[inline]
    pub fn escrow_token(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.escrow_token,
            SlotAccounts::UnifiedSol(u) => u.escrow_token,
        }
    }

    /// Get the recipient token account.
    #[inline]
    pub fn recipient_token(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.recipient_token,
            SlotAccounts::UnifiedSol(u) => u.recipient_token,
        }
    }

    /// Get the relayer token account.
    #[inline]
    pub fn relayer_token(&self) -> &'a AccountInfo {
        match self {
            SlotAccounts::Token(t) => t.relayer_token,
            SlotAccounts::UnifiedSol(u) => u.relayer_token,
        }
    }
}

// ============================================================================
// Builder Functions for New Structures
// ============================================================================

/// Build reward config map from remaining_accounts.
///
/// # Purpose
///
/// Loads pool configuration accounts needed for reward accumulator validation (V4).
/// Each reward config is 2 accounts regardless of pool type - NO vault needed since
/// we only read the `reward_accumulator` field for validation.
///
/// # Account Layout Per Entry
///
/// ```text
/// Token Pool (2 accounts):
///   [0] pool_config         - Hub's PoolConfig (shielded-pool program)
///   [1] token_pool_config   - TokenPoolConfig (token-pool program)
///
/// Unified SOL Pool (2 accounts):
///   [0] pool_config              - Hub's PoolConfig (shielded-pool program)
///   [1] unified_sol_pool_config  - UnifiedSolPoolConfig (unified-sol-pool program)
/// ```
///
/// # Validation Summary
///
/// | Check | Where | Description |
/// |-------|-------|-------------|
/// | PoolConfig owner | HERE | Must be shielded-pool program |
/// | PoolConfig discriminator | HERE | Valid PoolConfig account |
/// | pool_type valid | HERE | Must be Token(0) or UnifiedSol(1) |
/// | TokenPoolConfig owner | DEFERRED | validate_token_accumulator_v2 |
/// | TokenPoolConfig PDA | DEFERRED | validate_token_accumulator_v2 |
/// | TokenPoolConfig.asset_id | DEFERRED | Must match proof's rewardAssetId |
/// | TokenPoolConfig.is_active | DEFERRED | Pool must be active |
/// | TokenPoolConfig.reward_accumulator | DEFERRED | Must match proof value |
/// | UnifiedSolPoolConfig owner | DEFERRED | validate_unified_sol_accumulator_v2 |
/// | UnifiedSolPoolConfig.asset_id | DEFERRED | Must match proof's rewardAssetId |
/// | UnifiedSolPoolConfig.is_active | DEFERRED | Pool must be active |
/// | UnifiedSolPoolConfig.reward_accumulator | DEFERRED | Must match proof value |
///
/// # Security Notes
///
/// - PoolConfig PDA verification is implicit: if owner is shielded-pool and data deserializes
///   correctly, attacker cannot forge (they can't write to our program's accounts)
/// - Pool-specific config validation is DEFERRED to validation functions because we need
///   the proof's rewardAssetId to verify the correct config was provided
/// - Duplicate asset_ids use first occurrence (client must not send duplicates)
pub fn build_reward_config_map<'a>(
    _program_id: &Pubkey,
    remaining: &'a [AccountInfo],
    unique_reward_config_count: usize,
) -> Result<BuildRewardConfigResult<'a>, ProgramError> {
    let mut reward_config_map: RewardConfigMap<'a> = BTreeMap::new();
    let mut idx = 0;

    for _i in 0..unique_reward_config_count {
        // ====================================================================
        // ACCOUNT COUNT CHECK
        // Each reward config group requires exactly 2 accounts
        // ====================================================================
        if idx + 2 > remaining.len() {
            return Err(ShieldedPoolError::MissingAccounts.into());
        }

        // ====================================================================
        // ACCOUNT [0]: pool_config (Hub's PoolConfig)
        // ====================================================================
        // Type:    PoolConfig (defined in shielded-pool/src/state/pool_config.rs)
        // Owner:   shielded-pool program (crate::ID)
        // PDA:     ["pool_config", asset_id] - verified implicitly by owner check
        // Purpose: Routes to correct pool program, stores asset_id and pool_type
        //
        // VALIDATION HERE:
        //   ✓ Owner is shielded-pool program (AccountLoader::new checks this)
        //   ✓ Data deserializes as valid PoolConfig (discriminator check)
        //   ✓ pool_type is valid enum variant (Token=0 or UnifiedSol=1)
        //
        // SECURITY: Attacker cannot provide fake PoolConfig because:
        //   - They cannot create accounts owned by shielded-pool program
        //   - If owner check passes, data integrity is guaranteed
        // ====================================================================
        let pool_config_account = &remaining[idx];

        let loader = AccountLoader::<PoolConfig>::new(pool_config_account)
            .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?;
        let config = loader.load()
            .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?;

        let asset_id = config.asset_id;
        let pool_type = HubPoolType::from_u8(config.pool_type)
            .ok_or(ShieldedPoolError::InvalidPoolConfig)?;

        // ====================================================================
        // ACCOUNT [1]: pool_specific_config (TokenPoolConfig or UnifiedSolPoolConfig)
        // ====================================================================
        // Type depends on pool_type from PoolConfig above.
        //
        // FOR TOKEN POOL:
        //   Type:    TokenPoolConfig (defined in token-pool program)
        //   Owner:   token-pool program (TOKEN_POOL_PROGRAM_ID)
        //   PDA:     ["token_pool_config", mint]
        //   Fields:  asset_id, mint, is_active, reward_accumulator, ...
        //
        // FOR UNIFIED SOL POOL:
        //   Type:    UnifiedSolPoolConfig (defined in unified-sol-pool program)
        //   Owner:   unified-sol-pool program (UNIFIED_SOL_POOL_PROGRAM_ID)
        //   PDA:     ["unified_sol_pool_config"]
        //   Fields:  asset_id, is_active, reward_accumulator, ...
        //
        // VALIDATION DEFERRED to validate_*_accumulator_v2 functions:
        //   □ Owner is correct pool program
        //   □ PDA derivation is correct (for token pool: from mint)
        //   □ asset_id matches proof's rewardAssetId[i]
        //   □ is_active == 1 (pool not paused)
        //   □ reward_accumulator matches proof's rewardAcc[i]
        //
        // WHY DEFERRED: We don't have the proof data here. Validation functions
        // receive both the config and the proof values to cross-check.
        // ====================================================================
        let pool_specific_config = &remaining[idx + 1];
        idx += 2;

        // Build the reward config enum based on pool type
        // Note: pool_config_account is used above for routing but not stored
        let reward_config = match pool_type {
            HubPoolType::Token => {
                // Token pool: pool_specific_config is TokenPoolConfig
                // Owner/PDA validation deferred to validate_token_accumulator_v2
                RewardConfig::Token(TokenRewardConfig {
                    token_pool_config: pool_specific_config,
                })
            }
            HubPoolType::UnifiedSol => {
                // Unified SOL pool: pool_specific_config is UnifiedSolPoolConfig
                // Owner/PDA validation deferred to validate_unified_sol_accumulator_v2
                //
                // NOTE: All LSTs share the SAME UnifiedSolPoolConfig and thus the
                // same reward_accumulator. Multiple LSTs with different lst_configs
                // will have the same asset_id key and use this single entry.
                RewardConfig::UnifiedSol(UnifiedSolRewardConfig {
                    unified_sol_pool_config: pool_specific_config,
                })
            }
        };

        // ====================================================================
        // MAP INSERTION
        // ====================================================================
        // Key:   asset_id (from PoolConfig)
        // Value: RewardConfig enum (Token or UnifiedSol variant)
        //
        // DUPLICATE HANDLING:
        //   - First occurrence wins (or_insert)
        //   - Client should NOT send duplicate asset_ids
        //   - If duplicate, we log warning but don't fail (idempotent)
        //
        // UNIFIED SOL NOTE:
        //   All LSTs share the same asset_id (unified SOL virtual asset).
        //   Only ONE entry needed per unique asset_id for reward validation,
        //   since reward_accumulator is in UnifiedSolPoolConfig (shared).
        // ====================================================================
        reward_config_map.entry(asset_id).or_insert(reward_config);
    }

    Ok(BuildRewardConfigResult {
        reward_config_map,
        accounts_consumed: idx,
    })
}


/// Require reward config for asset_id, returning error if not found.
#[inline]
pub fn require_reward_config<'a, 'b>(
    reward_config_map: &'b RewardConfigMap<'a>,
    asset_id: &[u8; 32],
) -> Result<&'b RewardConfig<'a>, ProgramError> {
    reward_config_map
        .get(asset_id)
        .ok_or_else(|| ShieldedPoolError::InvalidAssetId.into())
}
