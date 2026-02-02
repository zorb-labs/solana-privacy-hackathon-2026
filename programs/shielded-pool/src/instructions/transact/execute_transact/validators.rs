//! Public amount and accumulator validation helpers for execute_transact.
//!
//! This module provides validation functions that verify the consistency between
//! ZK proof public inputs and on-chain pool state. These are security-critical
//! for ensuring the circuit's public_amount matches the intended deposit/withdrawal.
//!
//! # Security Considerations
//! - Public amount validation ensures ZK proof binds to correct ext_amount
//! - Accumulator validation ensures reward calculations use fresh on-chain state
//! - Exchange rate validation (unified SOL) prevents rate manipulation attacks

use crate::{
    errors::ShieldedPoolError,
    state::{LstConfig, TokenPoolConfig, UnifiedSolPoolConfig, find_token_pool_config_pda},
    utils::{self, check_public_amount_unified, validate_fee, validate_fee_unified},
};
use panchor::prelude::AccountLoader;
use pinocchio::program_error::ProgramError;
use pinocchio_contrib::AccountAssertions;
use zorb_pool_interface::{TOKEN_POOL_PROGRAM_ID, UNIFIED_SOL_POOL_PROGRAM_ID};

use super::accounts::RewardConfig;
use super::pool_config::PoolConfig;

// ============================================================================
// Public Amount Validators
// ============================================================================

/// Validate unified SOL public amount and fees.
///
/// For unified SOL pools:
/// - Public amount uses exchange rate: p = ext_amount × rate / precision (deposit) or inverse (withdrawal)
/// - Fee is calculated on shielded amount (s), not external amount (e)
/// - Per spec: Deposit: s = φ(e), Withdrawal: s = |p|
/// - Relayer fee is also deducted from public_amount for deposits
///
/// # Security
/// - Validates pool is active before accepting transactions
/// - Verifies exchange rate conversion matches ZK-bound public_amount
/// - Validates fees are within pool's configured bounds
#[inline(never)]
pub fn validate_unified_sol_public_amount(
    pool: &PoolConfig,
    ext_amount: i64,
    fee: u64,
    _relayer_fee: u64, // Unused - relayer_fee is a split of ext_amount, not deducted from public_amount
    public_amount: [u8; 32],
) -> Result<(), ProgramError> {
    // Check is_active first
    if !pool.is_active() {
        return Err(ShieldedPoolError::PoolPaused.into());
    }

    let exchange_rate = pool.deposit_exchange_rate();

    // Validate public_amount with exchange rate conversion
    // Note: relayer_fee is a split of ext_amount, not deducted from public_amount
    if !check_public_amount_unified(
        ext_amount,
        fee,
        public_amount,
        exchange_rate,
        LstConfig::RATE_PRECISION,
    ) {
        return Err(ShieldedPoolError::InvalidPublicAmountData.into());
    }

    // Validate fee on shielded amount
    validate_fee_unified(
        ext_amount,
        fee,
        public_amount,
        pool.deposit_fee_rate(),
        pool.withdrawal_fee_rate(),
        exchange_rate,
        LstConfig::RATE_PRECISION,
    )?;

    Ok(())
}

/// Validate token pool public amount and fees.
///
/// For token pools:
/// - Public amount is 1:1 with external amount: p = ext_amount - fee
/// - Fee is calculated on external amount
///
/// # Security
/// - Validates pool is active before accepting transactions
/// - Verifies 1:1 amount conversion matches ZK-bound public_amount
/// - Validates fees are within pool's configured bounds
#[inline(never)]
pub fn validate_token_public_amount(
    pool: &PoolConfig,
    ext_amount: i64,
    fee: u64,
    relayer_fee: u64,
    public_amount: [u8; 32],
) -> Result<(), ProgramError> {
    // Check is_active first
    if !pool.is_active() {
        return Err(ShieldedPoolError::PoolPaused.into());
    }

    // Skip validation for inactive slots
    if ext_amount == 0 && public_amount == [0u8; 32] {
        return Ok(());
    }

    // Validate public_amount (1:1, no exchange rate)
    if !utils::check_public_amount(ext_amount, fee, public_amount) {
        return Err(ShieldedPoolError::InvalidPublicAmountData.into());
    }

    // Validate fee on external amount
    validate_fee(
        ext_amount,
        fee,
        relayer_fee,
        pool.deposit_fee_rate(),
        pool.withdrawal_fee_rate(),
    )?;

    Ok(())
}

// ============================================================================
// Accumulator Helpers
// ============================================================================

/// Convert u128 accumulator to 32-byte big-endian representation.
/// The u128 is placed in the lower 16 bytes (bytes 16-31).
#[inline]
fn accumulator_to_bytes(accumulator: u128) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[16..].copy_from_slice(&accumulator.to_be_bytes());
    bytes
}

// ============================================================================
// Accumulator Validators
// ============================================================================

/// Validate token pool reward accumulator using RewardConfig (v2).
/// Uses the new separated reward config structure.
///
/// # Security
/// - Verifies config account is owned by token-pool program
/// - Validates PDA derivation to prevent spoofed configs
/// - Ensures asset_id matches expected value
/// - Verifies pool is active
/// - Confirms accumulator matches on-chain state (prevents stale reward attacks)
#[inline(never)]
pub fn validate_token_accumulator(
    reward_config: &RewardConfig,
    in_asset_id: &[u8; 32],
    in_accumulator: &[u8; 32],
) -> Result<(), ProgramError> {
    let config_account = match reward_config {
        RewardConfig::Token(t) => t.token_pool_config,
        RewardConfig::UnifiedSol(_) => {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }
    };

    // TokenPoolConfig is owned by the token-pool program
    config_account.assert_owner(&TOKEN_POOL_PROGRAM_ID)?;

    AccountLoader::<TokenPoolConfig>::new(config_account)?.try_inspect(|token_config| {
        // PDA verification
        let (expected_pda, _) = find_token_pool_config_pda(&token_config.mint);
        config_account.assert_key(&expected_pda)?;

        // Verify asset_id matches
        if token_config.asset_id != *in_asset_id {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }

        // Verify pool is active
        if token_config.is_active == 0 {
            return Err(ShieldedPoolError::PoolPaused.into());
        }

        // Verify accumulator matches on-chain state
        if accumulator_to_bytes(token_config.reward_accumulator) != *in_accumulator {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }
        Ok(())
    })
}

/// Validate unified SOL reward accumulator using RewardConfig (v2).
/// Uses the new separated reward config structure.
///
/// # Security
/// - Verifies config account is owned by unified-sol-pool program
/// - Ensures asset_id matches expected value
/// - Verifies pool is active
/// - Confirms accumulator matches on-chain state (prevents stale reward attacks)
#[inline(never)]
pub fn validate_unified_sol_accumulator(
    reward_config: &RewardConfig,
    in_asset_id: &[u8; 32],
    in_accumulator: &[u8; 32],
) -> Result<(), ProgramError> {
    let config_account = match reward_config {
        RewardConfig::UnifiedSol(u) => u.unified_sol_pool_config,
        RewardConfig::Token(_) => {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }
    };

    // UnifiedSolPoolConfig is owned by the unified-sol-pool program
    config_account.assert_owner(&UNIFIED_SOL_POOL_PROGRAM_ID)?;

    AccountLoader::<UnifiedSolPoolConfig>::new(config_account)?.try_inspect(|unified_config| {
        // Verify asset_id matches
        if unified_config.asset_id != *in_asset_id {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }

        // Verify pool is active
        if unified_config.is_active == 0 {
            return Err(ShieldedPoolError::PoolPaused.into());
        }

        // Verify accumulator matches on-chain state
        if accumulator_to_bytes(unified_config.reward_accumulator) != *in_accumulator {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }
        Ok(())
    })
}
