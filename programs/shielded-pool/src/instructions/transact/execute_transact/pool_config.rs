//! Pool configuration abstraction for execute_transact validation.
//!
//! This module provides a unified interface for accessing pool configuration
//! across different pool types (TokenPool vs UnifiedSolPool). It extracts
//! security-critical operations into a well-documented, auditable surface.

use crate::{
    errors::ShieldedPoolError,
    state::{LstConfig, TokenPoolConfig, UnifiedSolPoolConfig},
};
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

// ============================================================================
// POOL CONFIGURATION ENUM
// ============================================================================

/// Validated pool configuration for pre-validation phase.
///
/// This enum unifies handling of TokenConfig and UnifiedSol pools by holding
/// references to the appropriate config accounts for validation.
pub enum PoolConfig<'a> {
    /// Regular token pool config
    Token {
        config: pinocchio::account_info::Ref<'a, TokenPoolConfig>,
    },
    /// Unified SOL pool config (unified config + LST config)
    UnifiedSol {
        unified_config: pinocchio::account_info::Ref<'a, UnifiedSolPoolConfig>,
        lst_config: pinocchio::account_info::Ref<'a, LstConfig>,
    },
}

impl<'a> PoolConfig<'a> {
    /// Check if the pool is active.
    #[inline]
    pub fn is_active(&self) -> bool {
        match self {
            PoolConfig::Token { config, .. } => config.is_active != 0,
            PoolConfig::UnifiedSol {
                unified_config,
                lst_config,
                ..
            } => unified_config.is_active != 0 && lst_config.is_active != 0,
        }
    }

    /// Get deposit fee rate in basis points.
    #[inline]
    pub fn deposit_fee_rate(&self) -> u16 {
        match self {
            PoolConfig::Token { config, .. } => config.deposit_fee_rate,
            PoolConfig::UnifiedSol { unified_config, .. } => unified_config.deposit_fee_rate,
        }
    }

    /// Get withdrawal fee rate in basis points.
    #[inline]
    pub fn withdrawal_fee_rate(&self) -> u16 {
        match self {
            PoolConfig::Token { config, .. } => config.withdrawal_fee_rate,
            PoolConfig::UnifiedSol { unified_config, .. } => unified_config.withdrawal_fee_rate,
        }
    }

    /// Get the expected vault address from config.
    ///
    /// AUDIT: SECURITY-CRITICAL for unified SOL pools.
    /// Returns lst_config.lst_vault which is verified against passed vault account.
    /// See security chain documentation at L616-641.
    #[inline]
    pub fn expected_vault_address(&self) -> Pubkey {
        match self {
            PoolConfig::Token { config, .. } => config.vault,
            PoolConfig::UnifiedSol { lst_config, .. } => lst_config.lst_vault,
        }
    }

    /// Get the vault mint.
    ///
    /// AUDIT: SECURITY-CRITICAL for unified SOL pools.
    /// Returns lst_config.lst_mint which is used to verify token account mints.
    /// See security chain documentation at L616-641.
    #[inline]
    pub fn vault_mint(&self) -> Pubkey {
        match self {
            PoolConfig::Token { config, .. } => config.mint,
            PoolConfig::UnifiedSol { lst_config, .. } => lst_config.lst_mint,
        }
    }

    /// Get the exchange rate for deposits (LST → unified SOL conversion).
    /// Uses harvested_exchange_rate for unified SOL pools.
    ///
    /// AUDIT: This rate comes from lst_config. The security chain at L616-641 ensures
    /// this rate corresponds to the actual LST being deposited.
    #[inline]
    pub fn deposit_exchange_rate(&self) -> u64 {
        match self {
            PoolConfig::Token { .. } => LstConfig::RATE_PRECISION,
            PoolConfig::UnifiedSol { lst_config, .. } => lst_config.harvested_exchange_rate,
        }
    }

    /// Validate deposit constraints (pool-type specific).
    pub fn validate_deposit(
        &self,
        amount: u64,
        accumulator_epoch: u64,
    ) -> Result<(), ProgramError> {
        match self {
            PoolConfig::Token { config, .. } => {
                if amount > config.max_deposit_amount {
                    return Err(ShieldedPoolError::DepositLimitExceeded.into());
                }
                Ok(())
            }
            PoolConfig::UnifiedSol { lst_config, .. } => {
                // AUDIT: EPOCH MODEL - Verify LST was harvested in the PREVIOUS epoch
                // =====================================================================
                // The accumulator_epoch in the ZK proof corresponds to the CURRENT
                // reward_epoch (after finalization). The LST's last_harvest_epoch
                // should equal reward_epoch - 1 because:
                //
                // 1. harvest_lst_appreciation: sets last_harvest_epoch = reward_epoch
                // 2. finalize_unified_rewards: validates all LSTs harvested, then
                //    increments reward_epoch and freezes harvested_exchange_rate
                // 3. execute_transact: uses accumulator_epoch from the finalized state
                //
                // So: last_harvest_epoch (set during epoch N) == accumulator_epoch - 1
                //     where accumulator_epoch = N + 1 (after finalize incremented it)
                //
                // The checked_sub(1) also prevents deposits at epoch 0 (before any
                // finalization), which would underflow. This is intentional - epoch 0
                // is reserved as "uninitialized" (see init_unified_sol_pool_config.rs).
                //
                // See also:
                // - unified-sol-pool/init_unified_sol_pool_config.rs: reward_epoch = 1
                // - unified-sol-pool/init_lst_config.rs: last_harvest_epoch = 0
                // - unified-sol-pool/finalize_unified_rewards.rs: epoch increment
                // =====================================================================
                let expected = accumulator_epoch
                    .checked_sub(1)
                    .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
                if lst_config.last_harvest_epoch != expected {
                    return Err(ShieldedPoolError::StaleHarvestEpoch.into());
                }
                Ok(())
            }
        }
    }
}
