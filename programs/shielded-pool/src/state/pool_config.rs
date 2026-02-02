//! Hub's PoolConfig account for routing pool operations.
//!
//! This is a lightweight routing configuration that the hub uses to:
//! 1. Identify the pool type (Token or UnifiedSol)
//! 2. Get the pool program ID to CPI to
//! 3. Match asset_ids from ZK proofs
//!
//! The hub reads the actual pool state (fee rates, exchange rates) from
//! pool program accounts via cross-program account reading.

use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

use crate::state::ShieldedPoolAccount;

/// Pool type discriminator for routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PoolType {
    /// Standard SPL token pool (1:1 exchange rate)
    Token = 0,
    /// Unified SOL pool with LST support (exchange rate conversion)
    UnifiedSol = 1,
}

impl PoolType {
    /// Convert from u8 to PoolType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PoolType::Token),
            1 => Some(PoolType::UnifiedSol),
            _ => None,
        }
    }

    /// Returns the number of config accounts required for this pool type.
    ///
    /// - Token pool: 1 (TokenPoolConfig)
    /// - UnifiedSol pool: 2 (UnifiedSolPoolConfig + LstConfig)
    pub const fn config_account_count(&self) -> usize {
        match self {
            PoolType::Token => 1,
            PoolType::UnifiedSol => 2,
        }
    }

    /// Returns the total number of accounts per asset slot for this pool type.
    ///
    /// Layout for Token pool (6 accounts):
    /// - hub_pool_config, token_pool_config, vault, depositor_token, recipient_token, relayer_token
    ///
    /// Layout for UnifiedSol pool (7 accounts):
    /// - hub_pool_config, unified_sol_pool_config, lst_config, vault, depositor_token, recipient_token, relayer_token
    pub const fn accounts_per_asset(&self) -> usize {
        match self {
            PoolType::Token => 6,
            PoolType::UnifiedSol => 7,
        }
    }
}

/// Hub's PoolConfig account for routing pool operations.
///
/// This is a PDA owned by the hub program that maps an asset_id to a pool
/// program. The hub uses this to:
/// 1. Determine which pool program to CPI to
/// 2. Validate that pool program accounts are owned by the expected program
/// 3. Route deposits/withdrawals to the correct pool
///
/// # PDA Seeds
/// `["pool_config", asset_id]`
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][72-byte struct data]`
///
/// Total on-chain size: 80 bytes
#[account(ShieldedPoolAccount::PoolConfig)]
#[repr(C)]
pub struct PoolConfig {
    // === Routing (64 bytes) ===
    /// Pool program ID to CPI to (and expected owner of pool config accounts)
    pub pool_program: Pubkey,
    /// Asset ID for matching proof.public_asset_ids
    pub asset_id: [u8; 32],

    // === Metadata (8 bytes) ===
    /// Pool type (0 = Token, 1 = UnifiedSol)
    pub pool_type: u8,
    /// Whether the pool is active (0 = inactive, 1 = active)
    pub is_active: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 5],
}

impl PoolConfig {
    /// Account size in bytes (including 8-byte discriminator)
    pub const SIZE: usize = 8 + core::mem::size_of::<Self>();

    /// Returns true if the pool is active.
    ///
    /// Note: Named `active()` instead of `is_active()` to avoid shadowing
    /// the `is_active` field.
    #[inline]
    pub fn active(&self) -> bool {
        self.is_active != 0
    }

    /// Get the pool type
    #[inline]
    pub fn pool_type(&self) -> Option<PoolType> {
        PoolType::from_u8(self.pool_type)
    }

    /// Get the pool program ID
    #[inline]
    pub fn pool_program(&self) -> &Pubkey {
        &self.pool_program
    }

    /// Get the asset ID
    #[inline]
    pub fn asset_id(&self) -> &[u8; 32] {
        &self.asset_id
    }

    /// Returns the number of config accounts required for this pool type.
    #[inline]
    pub fn config_account_count(&self) -> usize {
        self.pool_type()
            .map(|pt| pt.config_account_count())
            .unwrap_or(1)
    }

    /// Returns the total number of accounts per asset slot for this pool type.
    #[inline]
    pub fn accounts_per_asset(&self) -> usize {
        self.pool_type()
            .map(|pt| pt.accounts_per_asset())
            .unwrap_or(6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_type_from_u8() {
        assert_eq!(PoolType::from_u8(0), Some(PoolType::Token));
        assert_eq!(PoolType::from_u8(1), Some(PoolType::UnifiedSol));
        assert_eq!(PoolType::from_u8(2), None);
    }

    #[test]
    fn test_pool_config_size() {
        // 8 (discriminator) + 1 + 1 + 1 + 5 (padding) + 32 + 32 = 80
        assert_eq!(PoolConfig::SIZE, 80);
    }

    #[test]
    fn test_accounts_per_asset() {
        assert_eq!(PoolType::Token.accounts_per_asset(), 6);
        assert_eq!(PoolType::UnifiedSol.accounts_per_asset(), 7);
    }

    #[test]
    fn test_config_account_count() {
        assert_eq!(PoolType::Token.config_account_count(), 1);
        assert_eq!(PoolType::UnifiedSol.config_account_count(), 2);
    }
}
