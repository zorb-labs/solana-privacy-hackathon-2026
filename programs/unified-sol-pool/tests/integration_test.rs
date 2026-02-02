//! Unified SOL Pool Integration Tests
//!
//! Tests the unified-sol-pool program state logic and edge cases.

use unified_sol_pool::{LstConfig, UnifiedSolPoolConfig, UnifiedSolPoolError, UNIFIED_SOL_ASSET_ID};
use zorb_pool_interface::{DepositParams, WithdrawParams};

/// Slot interval alias for readable test assertions.
/// Tests use this so they stay correct if UPDATE_SLOT_INTERVAL changes.
const INTERVAL: u64 = UnifiedSolPoolConfig::UPDATE_SLOT_INTERVAL;

// =============================================================================
// Test Helpers
// =============================================================================

/// Create a default UnifiedSolPoolConfig for testing
fn default_pool_config() -> UnifiedSolPoolConfig {
    UnifiedSolPoolConfig {
        asset_id: UNIFIED_SOL_ASSET_ID,
        authority: [0u8; 32],
        pending_authority: [0u8; 32],
        reward_epoch: 1, // Starts at 1, not 0
        _reserved1: [0u64; 7],
        total_virtual_sol: 0,
        reward_accumulator: 0,
        last_finalized_slot: 0,
        pending_deposit_fees: 0,
        pending_withdrawal_fees: 0,
        pending_appreciation: 0,
        finalized_balance: 0,
        pending_deposits: 0,
        pending_withdrawals: 0,
        deposit_fee_rate: 100,
        withdrawal_fee_rate: 100,
        min_buffer_bps: 2000,
        _pad1: [0u8; 2],
        min_buffer_amount: 1_000_000_000,
        is_active: 1,
        bump: 255,
        _pad2: [0u8; 14],
        total_deposited: 0,
        total_withdrawn: 0,
        total_rewards_distributed: 0,
        total_deposit_fees: 0,
        total_withdrawal_fees: 0,
        _reserved_stats: 0,
        total_appreciation: 0,
        max_deposit_amount: 0,
        deposit_count: 0,
        withdrawal_count: 0,
        lst_count: 0,
        _reserved: [0u8; 23],
    }
}

/// Create a default LstConfig for testing
fn default_lst_config() -> LstConfig {
    LstConfig {
        // Header
        pool_type: 0, // WSOL
        is_active: 1,
        bump: 255,
        _header_pad: [0u8; 5],
        // Common References
        lst_mint: [0u8; 32],
        lst_vault: [0u8; 32],
        // Exchange Rate State
        exchange_rate: 1_000_000_000, // 1:1 rate
        harvested_exchange_rate: 1_000_000_000,
        last_rate_update_slot: 0,
        last_harvest_epoch: 0,
        _pad_for_u128: 0,
        // Value Tracking
        total_virtual_sol: 0,
        vault_token_balance: 0,
        _value_pad: 0,
        // Statistics
        total_deposited: 0,
        total_withdrawn: 0,
        total_appreciation_harvested: 0,
        deposit_count: 0,
        withdrawal_count: 0,
        _stat_pad: 0,
        // Stake Pool Specific (zeroed for WSOL)
        stake_pool: [0u8; 32],
        stake_pool_program: [0u8; 32],
        previous_exchange_rate: 1_000_000_000,
        // Reserved
        _reserved: 0,
    }
}

// =============================================================================
// Basic Struct Tests
// =============================================================================

#[test]
fn test_unified_sol_pool_config_size() {
    assert!(
        core::mem::size_of::<UnifiedSolPoolConfig>() < 1024,
        "UnifiedSolPoolConfig too large"
    );
}

#[test]
fn test_lst_config_size() {
    assert!(
        core::mem::size_of::<LstConfig>() < 512,
        "LstConfig too large"
    );
}

#[test]
fn test_unified_sol_asset_id_is_one() {
    // Asset ID should be the number 1 in big-endian
    let mut expected = [0u8; 32];
    expected[31] = 1;
    assert_eq!(UNIFIED_SOL_ASSET_ID, expected);
}

#[test]
fn test_unified_sol_pool_config_is_active() {
    let mut config = default_pool_config();
    assert!(config.is_active());

    config.is_active = 0;
    assert!(!config.is_active());

    config.is_active = 255;
    assert!(config.is_active());
}

#[test]
fn test_lst_config_is_active() {
    let mut config = default_lst_config();
    assert!(config.is_active());

    config.is_active = 0;
    assert!(!config.is_active());
}

// =============================================================================
// current_balance() Tests
// =============================================================================

#[test]
fn test_current_balance_normal() {
    let mut config = default_pool_config();
    config.finalized_balance = 500;
    config.pending_deposits = 300;
    config.pending_withdrawals = 100;

    assert_eq!(config.current_balance().unwrap(), 700);
}

#[test]
fn test_current_balance_all_zeros() {
    let config = default_pool_config();
    assert_eq!(config.current_balance().unwrap(), 0);
}

#[test]
fn test_current_balance_exact_withdraw() {
    let mut config = default_pool_config();
    config.finalized_balance = 1000;
    config.pending_deposits = 500;
    config.pending_withdrawals = 1500;

    assert_eq!(config.current_balance().unwrap(), 0);
}

#[test]
fn test_current_balance_underflow_error() {
    let mut config = default_pool_config();
    config.finalized_balance = 1000;
    config.pending_deposits = 500;
    config.pending_withdrawals = 1501;

    let result = config.current_balance();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), UnifiedSolPoolError::ArithmeticOverflow));
}

#[test]
fn test_current_balance_addition_overflow() {
    let mut config = default_pool_config();
    config.finalized_balance = u128::MAX;
    config.pending_deposits = 1;

    let result = config.current_balance();
    assert!(result.is_err());
}

// =============================================================================
// record_deposit() Tests
// =============================================================================

#[test]
fn test_record_deposit_basic() {
    let mut config = default_pool_config();

    config.record_deposit(1000).unwrap();

    assert_eq!(config.pending_deposits, 1000);
    // Note: total_virtual_sol is NOT updated during deposit - it's calculated at finalization
    assert_eq!(config.total_deposited, 1000);
}

#[test]
fn test_record_deposit_multiple() {
    let mut config = default_pool_config();

    config.record_deposit(1000).unwrap();
    config.record_deposit(500).unwrap();

    assert_eq!(config.pending_deposits, 1500);
    // Note: total_virtual_sol is NOT updated during deposit - it's calculated at finalization
    assert_eq!(config.total_deposited, 1500);
}

#[test]
fn test_record_deposit_zero() {
    let mut config = default_pool_config();

    config.record_deposit(0).unwrap();

    assert_eq!(config.pending_deposits, 0);
    // Note: total_virtual_sol is NOT updated during deposit - it's calculated at finalization
}

#[test]
fn test_record_deposit_overflow_pending() {
    let mut config = default_pool_config();
    config.pending_deposits = u128::MAX;

    let result = config.record_deposit(1);
    assert!(result.is_err());
}

// Note: test_record_deposit_overflow_total_virtual_sol was removed because
// total_virtual_sol is no longer updated during deposit - it's calculated at finalization.

#[test]
fn test_record_deposit_overflow_total_deposited() {
    let mut config = default_pool_config();
    config.total_deposited = u128::MAX;

    let result = config.record_deposit(1);
    assert!(result.is_err());
}

// =============================================================================
// record_withdrawal() Tests
// =============================================================================

#[test]
fn test_record_withdrawal_basic() {
    let mut config = default_pool_config();

    config.record_withdrawal(300).unwrap();

    assert_eq!(config.pending_withdrawals, 300);
    // Note: total_virtual_sol is NOT updated during withdrawal - it's calculated at finalization
    assert_eq!(config.total_withdrawn, 300);
}

#[test]
fn test_record_withdrawal_multiple() {
    let mut config = default_pool_config();

    config.record_withdrawal(300).unwrap();
    config.record_withdrawal(200).unwrap();

    assert_eq!(config.pending_withdrawals, 500);
    // Note: total_virtual_sol is NOT updated during withdrawal - it's calculated at finalization
    assert_eq!(config.total_withdrawn, 500);
}

#[test]
fn test_record_withdrawal_exact_balance() {
    let mut config = default_pool_config();

    config.record_withdrawal(1000).unwrap();

    assert_eq!(config.pending_withdrawals, 1000);
    assert_eq!(config.total_withdrawn, 1000);
    // Note: total_virtual_sol is NOT updated during withdrawal - it's calculated at finalization
}

// Note: test_record_withdrawal_underflow_total_virtual_sol was removed because
// total_virtual_sol is no longer updated during withdrawal - it's calculated at finalization.

#[test]
fn test_record_withdrawal_overflow_pending() {
    let mut config = default_pool_config();
    config.pending_withdrawals = u128::MAX;
    config.total_virtual_sol = 1;

    let result = config.record_withdrawal(1);
    assert!(result.is_err());
}

// =============================================================================
// calculate_required_buffer() Tests
// =============================================================================

#[test]
fn test_calculate_required_buffer_percentage_larger() {
    let mut config = default_pool_config();
    config.total_virtual_sol = 100_000_000_000_000; // 100,000 SOL
    config.min_buffer_bps = 2000; // 20%
    config.min_buffer_amount = 1_000_000_000; // 1 SOL minimum

    // 20% of 100,000 SOL = 20,000 SOL
    let buffer = config.calculate_required_buffer().unwrap();
    assert_eq!(buffer, 20_000_000_000_000);
}

#[test]
fn test_calculate_required_buffer_minimum_larger() {
    let mut config = default_pool_config();
    config.total_virtual_sol = 1_000_000_000; // 1 SOL
    config.min_buffer_bps = 2000; // 20%
    config.min_buffer_amount = 1_000_000_000; // 1 SOL minimum

    // 20% of 1 SOL = 0.2 SOL, but minimum is 1 SOL
    let buffer = config.calculate_required_buffer().unwrap();
    assert_eq!(buffer, 1_000_000_000);
}

#[test]
fn test_calculate_required_buffer_zero_balance() {
    let mut config = default_pool_config();
    config.total_virtual_sol = 0;
    config.min_buffer_bps = 2000;
    config.min_buffer_amount = 1_000_000_000;

    // 20% of 0 = 0, minimum is 1 SOL
    let buffer = config.calculate_required_buffer().unwrap();
    assert_eq!(buffer, 1_000_000_000);
}

#[test]
fn test_calculate_required_buffer_exact_equal() {
    let mut config = default_pool_config();
    config.total_virtual_sol = 5_000_000_000; // 5 SOL
    config.min_buffer_bps = 2000; // 20%
    config.min_buffer_amount = 1_000_000_000; // 1 SOL minimum

    // 20% of 5 SOL = 1 SOL = minimum
    let buffer = config.calculate_required_buffer().unwrap();
    assert_eq!(buffer, 1_000_000_000);
}

// =============================================================================
// add_appreciation() Tests
// =============================================================================

#[test]
fn test_add_appreciation_basic() {
    let mut config = default_pool_config();

    config.add_appreciation(1_000_000_000).unwrap();

    assert_eq!(config.pending_appreciation, 1_000_000_000);
    assert_eq!(config.total_appreciation, 1_000_000_000);
}

#[test]
fn test_add_appreciation_multiple() {
    let mut config = default_pool_config();

    config.add_appreciation(1_000_000_000).unwrap();
    config.add_appreciation(500_000_000).unwrap();

    assert_eq!(config.pending_appreciation, 1_500_000_000);
    assert_eq!(config.total_appreciation, 1_500_000_000);
}

#[test]
fn test_add_appreciation_overflow_pending() {
    let mut config = default_pool_config();
    config.pending_appreciation = u64::MAX;

    let result = config.add_appreciation(1);
    assert!(result.is_err());
}

#[test]
fn test_add_appreciation_overflow_total() {
    let mut config = default_pool_config();
    config.total_appreciation = u128::MAX;

    let result = config.add_appreciation(1);
    assert!(result.is_err());
}

// =============================================================================
// finalize_rewards() Slot Timing Tests
// =============================================================================

#[test]
fn test_finalize_rewards_not_ready_returns_false() {
    let mut config = default_pool_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 100;

    // Not enough slots elapsed (one slot short of interval)
    let result = config.finalize_rewards(1000 + INTERVAL - 1);
    assert!(result.is_ok());
    assert!(!result.unwrap()); // Returns false when not ready
}

#[test]
fn test_finalize_rewards_ready_returns_true() {
    let mut config = default_pool_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 1_000_000_000;
    config.pending_deposit_fees = 1_000_000;

    // Exactly UPDATE_SLOT_INTERVAL slots
    let result = config.finalize_rewards(1000 + INTERVAL);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Returns true when ready and finalized
}

#[test]
fn test_finalize_rewards_past_interval() {
    let mut config = default_pool_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 1_000_000_000;

    let result = config.finalize_rewards(1000 + INTERVAL * 3);
    assert!(result.is_ok());
    assert!(result.unwrap());
    assert_eq!(config.last_finalized_slot, 1000 + INTERVAL * 3);
}

// =============================================================================
// finalize_rewards() Epoch Increment Tests
// =============================================================================

#[test]
fn test_finalize_rewards_increments_epoch() {
    let mut config = default_pool_config();
    config.reward_epoch = 1;
    config.finalized_balance = 1_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();

    assert_eq!(config.reward_epoch, 2);
}

#[test]
fn test_finalize_rewards_epoch_increments_multiple_times() {
    let mut config = default_pool_config();
    config.reward_epoch = 1;
    config.finalized_balance = 1_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();
    assert_eq!(config.reward_epoch, 2);

    config.finalize_rewards(INTERVAL * 2).unwrap();
    assert_eq!(config.reward_epoch, 3);

    config.finalize_rewards(INTERVAL * 3).unwrap();
    assert_eq!(config.reward_epoch, 4);
}

#[test]
fn test_finalize_rewards_epoch_overflow() {
    let mut config = default_pool_config();
    config.reward_epoch = u64::MAX;
    config.finalized_balance = 1_000_000_000;

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_err());
}

// =============================================================================
// finalize_rewards() Accumulator Math Tests
// =============================================================================

#[test]
fn test_finalize_rewards_basic_distribution() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000_000; // 1000 virtual SOL
    config.pending_deposit_fees = 10_000_000_000; // 10 virtual SOL in fees

    config.finalize_rewards(INTERVAL).unwrap();

    let expected = 10_000_000_000u128 * UnifiedSolPoolConfig::ACCUMULATOR_PRECISION / 1_000_000_000_000;
    assert_eq!(config.reward_accumulator, expected);
    assert_eq!(config.total_rewards_distributed, 10_000_000_000);
}

#[test]
fn test_finalize_rewards_combines_all_sources() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000_000;
    config.pending_deposit_fees = 1_000_000_000;
    config.pending_withdrawal_fees = 2_000_000_000;
    config.pending_appreciation = 3_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();

    // Total = 1 + 2 + 3 = 6 virtual SOL
    let expected = 6_000_000_000u128 * UnifiedSolPoolConfig::ACCUMULATOR_PRECISION / 1_000_000_000_000;
    assert_eq!(config.reward_accumulator, expected);
    assert_eq!(config.total_rewards_distributed, 6_000_000_000);
}

#[test]
fn test_finalize_rewards_resets_pending_fields() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000;
    config.pending_deposit_fees = 100;
    config.pending_withdrawal_fees = 200;
    config.pending_appreciation = 300;
    config.pending_deposits = 400;
    config.pending_withdrawals = 100;

    config.finalize_rewards(INTERVAL).unwrap();

    assert_eq!(config.pending_deposit_fees, 0);
    assert_eq!(config.pending_withdrawal_fees, 0);
    assert_eq!(config.pending_appreciation, 0);
    assert_eq!(config.pending_deposits, 0);
    assert_eq!(config.pending_withdrawals, 0);
}

#[test]
fn test_finalize_rewards_updates_finalized_balance() {
    let mut config = default_pool_config();
    config.finalized_balance = 1000;
    config.pending_deposits = 300;
    config.pending_withdrawals = 100;

    config.finalize_rewards(INTERVAL).unwrap();

    assert_eq!(config.finalized_balance, 1200); // 1000 + 300 - 100
}

// =============================================================================
// finalize_rewards() Edge Cases - Zero Values
// =============================================================================

#[test]
fn test_finalize_rewards_zero_rewards_skips_accumulator_update() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000;
    // All pending reward fields = 0

    let original_accumulator = config.reward_accumulator;
    config.finalize_rewards(INTERVAL).unwrap();

    // Accumulator should not change if no rewards
    assert_eq!(config.reward_accumulator, original_accumulator);
    assert_eq!(config.total_rewards_distributed, 0);
}

#[test]
fn test_finalize_rewards_zero_pool_with_rewards_skips_update() {
    // Edge: if total_pool = 0, accumulator update is skipped (no one to distribute to)
    let mut config = default_pool_config();
    config.finalized_balance = 0;
    config.pending_deposits = 0;
    config.pending_withdrawals = 0;
    config.pending_deposit_fees = 1_000_000; // Has rewards

    // If total_pool = 0, reward accumulator update is skipped
    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());
    assert!(result.unwrap()); // Still returns true (finalization happened)

    // Accumulator should remain 0 (no finalized balance to distribute to)
    assert_eq!(config.reward_accumulator, 0);
}

#[test]
fn test_finalize_rewards_zero_both_pool_and_rewards() {
    let mut config = default_pool_config();
    config.finalized_balance = 0;
    // All pending = 0

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());
    assert!(result.unwrap());

    // No changes to accumulator
    assert_eq!(config.reward_accumulator, 0);
}

// =============================================================================
// finalize_rewards() Precision Tests
// =============================================================================

#[test]
fn test_finalize_rewards_precision_small_rewards_large_pool() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000_000_000_000_000; // 1 trillion SOL
    config.pending_deposit_fees = 1;

    config.finalize_rewards(INTERVAL).unwrap();

    // Very small rewards truncate to 0
    assert_eq!(config.reward_accumulator, 0);
    // But total_rewards_distributed still tracks it
    assert_eq!(config.total_rewards_distributed, 1);
}

#[test]
fn test_finalize_rewards_precision_preserved() {
    let mut config = default_pool_config();
    config.finalized_balance = 100_000_000_000; // 100 SOL
    config.pending_deposit_fees = 1_000_000_000; // 1 SOL (1%)

    config.finalize_rewards(INTERVAL).unwrap();

    let expected = 1_000_000_000u128 * UnifiedSolPoolConfig::ACCUMULATOR_PRECISION / 100_000_000_000;
    assert_eq!(config.reward_accumulator, expected);

    // User reward: 10 SOL * accumulator / 1e18 = 0.1 SOL
    let user_amount = 10_000_000_000u128;
    let user_reward = user_amount * config.reward_accumulator / UnifiedSolPoolConfig::ACCUMULATOR_PRECISION;
    assert_eq!(user_reward, 100_000_000);
}

// =============================================================================
// LstConfig Exchange Rate Tests
// =============================================================================

#[test]
fn test_calculate_virtual_sol_basic() {
    let mut config = default_lst_config();
    config.harvested_exchange_rate = 1_050_000_000; // 1.05x

    // 100 LST at 1.05x = 105 SOL
    let virtual_sol = config.calculate_virtual_sol(100_000_000_000);
    assert_eq!(virtual_sol, 105_000_000_000);
}

#[test]
fn test_calculate_virtual_sol_one_to_one() {
    let mut config = default_lst_config();
    config.harvested_exchange_rate = 1_000_000_000; // 1:1

    let virtual_sol = config.calculate_virtual_sol(100_000_000_000);
    assert_eq!(virtual_sol, 100_000_000_000);
}

#[test]
fn test_calculate_virtual_sol_discount_rate() {
    let mut config = default_lst_config();
    config.harvested_exchange_rate = 950_000_000; // 0.95x

    // 100 LST at 0.95x = 95 SOL
    let virtual_sol = config.calculate_virtual_sol(100_000_000_000);
    assert_eq!(virtual_sol, 95_000_000_000);
}

#[test]
fn test_calculate_virtual_sol_zero_balance() {
    let config = default_lst_config();
    let virtual_sol = config.calculate_virtual_sol(0);
    assert_eq!(virtual_sol, 0);
}

#[test]
fn test_calculate_lst_tokens_basic() {
    let mut config = default_lst_config();
    config.harvested_exchange_rate = 1_050_000_000; // 1.05x

    // 105 SOL at 1.05x = 100 LST
    let lst_tokens = config.calculate_lst_tokens(105_000_000_000);
    assert_eq!(lst_tokens, 100_000_000_000);
}

#[test]
fn test_calculate_lst_tokens_one_to_one() {
    let mut config = default_lst_config();
    config.harvested_exchange_rate = 1_000_000_000;

    let lst_tokens = config.calculate_lst_tokens(100_000_000_000);
    assert_eq!(lst_tokens, 100_000_000_000);
}

#[test]
fn test_calculate_lst_tokens_zero() {
    let config = default_lst_config();
    let lst_tokens = config.calculate_lst_tokens(0);
    assert_eq!(lst_tokens, 0);
}

#[test]
fn test_virtual_sol_round_trip() {
    let mut config = default_lst_config();
    config.harvested_exchange_rate = 1_050_000_000;

    let lst_amount = 100_000_000_000u64;
    let virtual_sol = config.calculate_virtual_sol(lst_amount);
    let recovered = config.calculate_lst_tokens(virtual_sol as u64);

    // Should recover original amount (exact round-trip)
    assert_eq!(recovered, lst_amount);
}

// =============================================================================
// LstConfig Rate Validation Tests
// =============================================================================

#[test]
fn test_validate_rate_change_first_update() {
    let mut config = default_lst_config();
    config.exchange_rate = 0; // First update

    // Should accept any rate on first update
    assert!(config.validate_rate_change(1_000_000_000).is_ok());
    assert!(config.validate_rate_change(2_000_000_000).is_ok());
}

#[test]
fn test_validate_rate_change_within_bounds() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000; // 1x

    // 0.5% change = 5_000_000 (max allowed)
    let max_change = config.exchange_rate / 200;
    assert_eq!(max_change, 5_000_000);

    // Should accept rate at boundary
    assert!(config.validate_rate_change(1_005_000_000).is_ok()); // +0.5%
    assert!(config.validate_rate_change(995_000_000).is_ok());   // -0.5%
}

#[test]
fn test_validate_rate_change_exceeds_bounds_up() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;

    // More than 0.5% increase
    let result = config.validate_rate_change(1_005_000_001);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), UnifiedSolPoolError::InvalidExchangeRate));
}

#[test]
fn test_validate_rate_change_exceeds_bounds_down() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;

    // More than 0.5% decrease
    let result = config.validate_rate_change(994_999_999);
    assert!(result.is_err());
}

#[test]
fn test_validate_rate_change_zero_change() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;

    // Same rate is valid
    assert!(config.validate_rate_change(1_000_000_000).is_ok());
}

// =============================================================================
// LstConfig update_exchange_rate() Tests
// =============================================================================

#[test]
fn test_update_exchange_rate_appreciation() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;

    // Rate increase from 1.00 to 1.01 with 100 LST in vault
    // appreciation = vault_balance * (new_rate - old_rate) / 1e9
    //              = 100e9 * (1.01e9 - 1e9) / 1e9 = 1e9 = 1 SOL
    let appreciation = config.update_exchange_rate(100_000_000_000, 1_010_000_000, 1000).unwrap();

    assert_eq!(appreciation, 1_000_000_000);
    assert_eq!(config.exchange_rate, 1_010_000_000);
    assert_eq!(config.previous_exchange_rate, 1_000_000_000);
    assert_eq!(config.last_rate_update_slot, 1000);
    assert_eq!(config.total_appreciation_harvested, 1_000_000_000);
}

#[test]
fn test_update_exchange_rate_depreciation() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;

    // Rate decrease - no appreciation
    let appreciation = config.update_exchange_rate(100_000_000_000, 990_000_000, 1000).unwrap();

    assert_eq!(appreciation, 0);
    assert_eq!(config.exchange_rate, 990_000_000);
}

#[test]
fn test_update_exchange_rate_no_change() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;

    let appreciation = config.update_exchange_rate(100_000_000_000, 1_000_000_000, 1000).unwrap();

    assert_eq!(appreciation, 0);
}

#[test]
fn test_update_exchange_rate_first_update() {
    let mut config = default_lst_config();
    config.exchange_rate = 0;

    // First update from rate=0 to rate=1e9 with 100 LST in vault
    // appreciation = vault_balance * (new_rate - old_rate) / 1e9
    //              = 100e9 * (1e9 - 0) / 1e9 = 100e9 = 100 SOL
    let appreciation = config.update_exchange_rate(100_000_000_000, 1_000_000_000, 1000).unwrap();

    assert_eq!(appreciation, 100_000_000_000);
}

#[test]
fn test_update_exchange_rate_overflow_appreciation() {
    let mut config = default_lst_config();
    config.exchange_rate = 1_000_000_000;
    config.total_appreciation_harvested = u64::MAX;

    // Would overflow total_appreciation_harvested
    let result = config.update_exchange_rate(100_000_000_000, 1_010_000_000, 1000);
    assert!(result.is_err());
}

// =============================================================================
// Reward Epoch Semantics Tests (AUDIT CRITICAL)
// =============================================================================

#[test]
fn test_epoch_starts_at_one() {
    // New pools should have reward_epoch = 1
    let config = default_pool_config();
    assert_eq!(config.reward_epoch, 1);
}

#[test]
fn test_lst_harvest_epoch_starts_at_zero() {
    // New LSTs should have last_harvest_epoch = 0 (never harvested)
    let config = default_lst_config();
    assert_eq!(config.last_harvest_epoch, 0);
}

#[test]
fn test_epoch_semantics_prevents_uninitialized_finalization() {
    // The epoch model exists to prevent finalizing with unharvested LSTs.
    // If reward_epoch started at 0, a new LST (last_harvest_epoch=0) would
    // pass the finalize check (0 == 0) without being harvested.
    //
    // With reward_epoch starting at 1:
    // - New LST: last_harvest_epoch = 0
    // - Finalize check: 0 == 1 fails (must harvest first)

    let pool = default_pool_config();
    let lst = default_lst_config();

    // Simulated finalize check: last_harvest_epoch == reward_epoch
    let can_finalize = lst.last_harvest_epoch == pool.reward_epoch;
    assert!(!can_finalize, "New LST should not pass finalize check");
}

// =============================================================================
// Constants Tests
// =============================================================================

#[test]
fn test_accumulator_precision_constant() {
    assert_eq!(UnifiedSolPoolConfig::ACCUMULATOR_PRECISION, 1_000_000_000_000_000_000);
}

#[test]
fn test_update_slot_interval_constant() {
    // Sanity check: interval should be reasonable (10-30 minutes at 400ms/slot)
    assert!(UnifiedSolPoolConfig::UPDATE_SLOT_INTERVAL >= 1500); // ~10 min minimum
    assert!(UnifiedSolPoolConfig::UPDATE_SLOT_INTERVAL <= 9000); // ~60 min maximum
    assert_eq!(INTERVAL, UnifiedSolPoolConfig::UPDATE_SLOT_INTERVAL);
}

#[test]
fn test_rate_precision_constant() {
    assert_eq!(LstConfig::RATE_PRECISION, 1_000_000_000);
}

#[test]
fn test_max_rate_change_bps_constant() {
    assert_eq!(LstConfig::MAX_RATE_CHANGE_BPS, 50); // 0.5%
}

// =============================================================================
// Serialization Tests
// =============================================================================

#[test]
fn test_deposit_params_for_unified_sol() {
    let params = DepositParams {
        amount: 100_000_000_000,          // 100 LST tokens
        expected_output: 103_950_000_000, // ~104 virtual SOL (accounting for rate and fee)
    };

    let data = zorb_pool_interface::build_deposit_instruction_data(&params);
    let parsed = zorb_pool_interface::parse_deposit_params(&data).unwrap();

    assert_eq!(parsed.amount, 100_000_000_000);
    assert_eq!(parsed.expected_output, 103_950_000_000);
}

#[test]
fn test_withdraw_params_for_unified_sol() {
    let params = WithdrawParams {
        amount: 106_050_000_000,          // virtual SOL spent
        expected_output: 100_000_000_000, // LST tokens to receive
    };

    let data = zorb_pool_interface::build_withdraw_instruction_data(&params);
    let parsed = zorb_pool_interface::parse_withdraw_params(&data).unwrap();

    assert_eq!(parsed.amount, 106_050_000_000);
    assert_eq!(parsed.expected_output, 100_000_000_000);
}

// =============================================================================
// Multi-Finalization Scenario Tests
// =============================================================================

#[test]
fn test_multiple_finalization_with_appreciation() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000_000;

    // Cycle 1: Deposit fees + appreciation
    config.pending_deposit_fees = 1_000_000_000;
    config.pending_appreciation = 5_000_000_000;
    config.finalize_rewards(INTERVAL).unwrap();

    let acc1 = config.reward_accumulator;
    assert_eq!(config.reward_epoch, 2);

    // Cycle 2: More appreciation
    config.pending_appreciation = 7_000_000_000;
    config.finalize_rewards(INTERVAL * 2).unwrap();

    let acc2 = config.reward_accumulator;
    assert_eq!(config.reward_epoch, 3);

    // Accumulator monotonically increases
    assert!(acc2 > acc1);

    // Total rewards = 1 + 5 + 7 = 13
    assert_eq!(config.total_rewards_distributed, 13_000_000_000);
}

#[test]
fn test_finalization_with_deposits_and_withdrawals() {
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000_000;

    // First cycle
    config.pending_deposits = 500_000_000_000;
    config.pending_withdrawals = 200_000_000_000;
    config.pending_appreciation = 10_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();

    // finalized_balance = 1000 + 500 - 200 = 1300
    assert_eq!(config.finalized_balance, 1_300_000_000_000);
}

// =============================================================================
// Documentation Example Verification
// =============================================================================

#[test]
fn test_docstring_example() {
    // Verify the example from UnifiedSolPoolConfig docstring
    let mut config = default_pool_config();
    config.finalized_balance = 1_000_000_000_000; // 1000e9 virtual SOL

    // User A deposits 500 vSOL
    config.pending_deposits = 500_000_000_000;
    // User B withdraws 200
    config.pending_withdrawals = 200_000_000_000;
    // LST appreciation + fees: 50e9
    config.pending_appreciation = 50_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();

    // total_pool = 1000e9 + 500e9 - 200e9 = 1300e9
    assert_eq!(config.finalized_balance, 1_300_000_000_000);

    // reward_delta = 50e9 * 1e18 / 1300e9 ≈ 38,461,538,461,538,461
    let expected_delta = 50_000_000_000u128 * UnifiedSolPoolConfig::ACCUMULATOR_PRECISION / 1_300_000_000_000;
    assert_eq!(config.reward_accumulator, expected_delta);
    assert!(config.reward_accumulator > 38_000_000_000_000_000);
    assert!(config.reward_accumulator < 39_000_000_000_000_000);
}
