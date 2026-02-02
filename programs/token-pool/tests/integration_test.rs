//! Token Pool Integration Tests
//!
//! Tests the token-pool program state logic and edge cases.

use token_pool::{TokenPoolConfig, TokenPoolError};
use zorb_pool_interface::{DepositParams, PoolInstruction, WithdrawParams};

/// Slot interval alias for readable test assertions.
/// Tests use this so they stay correct if UPDATE_SLOT_INTERVAL changes.
const INTERVAL: u64 = TokenPoolConfig::UPDATE_SLOT_INTERVAL;

// =============================================================================
// Test Helpers
// =============================================================================

/// Create a default TokenPoolConfig for testing
fn default_config() -> TokenPoolConfig {
    TokenPoolConfig {
        authority: [0u8; 32],
        pending_authority: [0u8; 32],
        mint: [0u8; 32],
        vault: [0u8; 32],
        asset_id: [0u8; 32],
        finalized_balance: 0,
        reward_accumulator: 0,
        pending_deposits: 0,
        pending_withdrawals: 0,
        pending_deposit_fees: 0,
        pending_withdrawal_fees: 0,
        pending_funded_rewards: 0,
        _pad_fees: 0,
        total_deposited: 0,
        total_withdrawn: 0,
        total_rewards_distributed: 0,
        total_deposit_fees: 0,
        total_withdrawal_fees: 0,
        total_funded_rewards: 0,
        _reserved_stats: 0,
        max_deposit_amount: u64::MAX,
        deposit_count: 0,
        withdrawal_count: 0,
        last_finalized_slot: 0,
        deposit_fee_rate: 100, // 1%
        withdrawal_fee_rate: 100,
        decimals: 9,
        is_active: 1,
        bump: 255,
        _padding: [0u8; 9],
    }
}

// =============================================================================
// Basic Struct Tests
// =============================================================================

#[test]
fn test_token_pool_config_size() {
    assert_eq!(
        core::mem::size_of::<TokenPoolConfig>(),
        TokenPoolConfig::SIZE,
        "TokenPoolConfig size mismatch"
    );
    assert!(TokenPoolConfig::SIZE < 1024);
}

#[test]
fn test_token_pool_config_is_active() {
    let mut config = default_config();
    assert!(config.is_active());

    config.is_active = 0;
    assert!(!config.is_active());

    // Edge: any non-zero value is active
    config.is_active = 42;
    assert!(config.is_active());
}

// =============================================================================
// current_balance() Tests
// =============================================================================

#[test]
fn test_current_balance_normal() {
    let mut config = default_config();
    config.finalized_balance = 500;
    config.pending_deposits = 300;
    config.pending_withdrawals = 100;

    // current_balance = 500 + 300 - 100 = 700
    assert_eq!(config.current_balance().unwrap(), 700);
}

#[test]
fn test_current_balance_all_zeros() {
    let config = default_config();
    assert_eq!(config.current_balance().unwrap(), 0);
}

#[test]
fn test_current_balance_no_withdrawals() {
    let mut config = default_config();
    config.finalized_balance = 1000;
    config.pending_deposits = 500;
    config.pending_withdrawals = 0;

    assert_eq!(config.current_balance().unwrap(), 1500);
}

#[test]
fn test_current_balance_exact_withdraw() {
    // Edge: withdrawals exactly equal finalized + deposits
    let mut config = default_config();
    config.finalized_balance = 1000;
    config.pending_deposits = 500;
    config.pending_withdrawals = 1500;

    assert_eq!(config.current_balance().unwrap(), 0);
}

#[test]
fn test_current_balance_underflow_returns_error() {
    // Edge: withdrawals exceed finalized + deposits
    let mut config = default_config();
    config.finalized_balance = 1000;
    config.pending_deposits = 500;
    config.pending_withdrawals = 1501; // one more than available

    let result = config.current_balance();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), TokenPoolError::ArithmeticOverflow));
}

#[test]
fn test_current_balance_addition_overflow() {
    // Edge: finalized_balance + pending_deposits overflows
    let mut config = default_config();
    config.finalized_balance = u128::MAX;
    config.pending_deposits = 1;
    config.pending_withdrawals = 0;

    let result = config.current_balance();
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), TokenPoolError::ArithmeticOverflow));
}

// =============================================================================
// finalize_rewards() Slot Timing Tests
// =============================================================================

#[test]
fn test_finalize_rewards_not_ready_zero_elapsed() {
    let mut config = default_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 100;

    // Same slot as last finalization
    let result = config.finalize_rewards(1000);
    assert!(result.is_err());
    // RewardsNotReady error code
    assert_eq!(
        result.unwrap_err(),
        pinocchio::program_error::ProgramError::Custom(TokenPoolError::RewardsNotReady as u32)
    );
}

#[test]
fn test_finalize_rewards_not_ready_one_slot_short() {
    let mut config = default_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 100;

    // One slot short of the required interval
    let result = config.finalize_rewards(1000 + INTERVAL - 1);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        pinocchio::program_error::ProgramError::Custom(TokenPoolError::RewardsNotReady as u32)
    );
}

#[test]
fn test_finalize_rewards_ready_exactly_at_interval() {
    let mut config = default_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 1_000_000_000; // 1 token
    config.pending_deposit_fees = 1_000_000; // 0.001 tokens

    // Exactly INTERVAL slots elapsed
    let result = config.finalize_rewards(1000 + INTERVAL);
    assert!(result.is_ok());
    assert_eq!(config.last_finalized_slot, 1000 + INTERVAL);
}

#[test]
fn test_finalize_rewards_ready_past_interval() {
    let mut config = default_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 1_000_000_000;

    // Well past interval
    let result = config.finalize_rewards(1000 + INTERVAL * 3);
    assert!(result.is_ok());
    assert_eq!(config.last_finalized_slot, 1000 + INTERVAL * 3);
}

#[test]
fn test_finalize_rewards_slot_zero_initial() {
    // Edge: First finalization from slot 0
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000;

    // Exactly one interval from slot 0
    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());
}

#[test]
fn test_finalize_rewards_saturating_sub_on_clock_drift() {
    // Edge: What if current_slot < last_finalized_slot? (clock drift, shouldn't happen but defensive)
    let mut config = default_config();
    config.last_finalized_slot = 1000;
    config.finalized_balance = 100;

    // Current slot before last finalized (saturating_sub gives 0)
    let result = config.finalize_rewards(500);
    assert!(result.is_err()); // 0 < INTERVAL, so not ready
}

// =============================================================================
// finalize_rewards() Accumulator Math Tests
// =============================================================================

#[test]
fn test_finalize_rewards_basic_distribution() {
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000; // 1000 tokens (9 decimals)
    config.pending_deposit_fees = 10_000_000_000; // 10 tokens in fees

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // reward_delta = 10e9 * 1e18 / 1000e9 = 10e18 / 1000 = 10e15
    let expected_delta = 10_000_000_000u128 * TokenPoolConfig::ACCUMULATOR_PRECISION / 1_000_000_000_000;
    assert_eq!(config.reward_accumulator, expected_delta);

    // total_rewards_distributed should track total pending
    assert_eq!(config.total_rewards_distributed, 10_000_000_000);

    // Pending fields should be reset
    assert_eq!(config.pending_deposit_fees, 0);
    assert_eq!(config.pending_withdrawal_fees, 0);
    assert_eq!(config.pending_funded_rewards, 0);
}

#[test]
fn test_finalize_rewards_combines_all_reward_sources() {
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000; // 1000 tokens
    config.pending_deposit_fees = 1_000_000_000;  // 1 token
    config.pending_withdrawal_fees = 2_000_000_000; // 2 tokens
    config.pending_funded_rewards = 3_000_000_000;  // 3 tokens

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // Total pending = 1 + 2 + 3 = 6 tokens
    let total_pending = 6_000_000_000u128;
    let expected_delta = total_pending * TokenPoolConfig::ACCUMULATOR_PRECISION / 1_000_000_000_000;
    assert_eq!(config.reward_accumulator, expected_delta);
    assert_eq!(config.total_rewards_distributed, total_pending);
}

#[test]
fn test_finalize_rewards_updates_finalized_balance() {
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1000;
    config.pending_deposits = 300;
    config.pending_withdrawals = 100;

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // finalized_balance = 1000 + 300 - 100 = 1200
    assert_eq!(config.finalized_balance, 1200);
    assert_eq!(config.pending_deposits, 0);
    assert_eq!(config.pending_withdrawals, 0);
}

#[test]
fn test_finalize_rewards_accumulator_adds_to_existing() {
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000;
    config.reward_accumulator = 1_000_000_000_000_000_000; // Pre-existing accumulator
    config.pending_deposit_fees = 1_000_000_000;

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // Should add to existing accumulator, not replace
    let delta = 1_000_000_000u128 * TokenPoolConfig::ACCUMULATOR_PRECISION / 1_000_000_000_000;
    assert_eq!(config.reward_accumulator, 1_000_000_000_000_000_000 + delta);
}

// =============================================================================
// finalize_rewards() Edge Cases - Zero Values
// =============================================================================

#[test]
fn test_finalize_rewards_zero_pending_rewards() {
    // Edge: All pending reward fields are 0
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000;
    config.pending_deposits = 100_000_000_000;
    // All pending fees = 0

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // Accumulator should remain 0 (no rewards to distribute)
    // Actually, looking at the code: it will compute 0 * 1e18 / pool = 0
    assert_eq!(config.reward_accumulator, 0);

    // But finalized_balance should still update
    assert_eq!(config.finalized_balance, 1_100_000_000_000);
}

#[test]
fn test_finalize_rewards_zero_total_pool_preserves_rewards() {
    // Edge: When total_pool = 0, rewards are preserved (not distributed)
    // The slot advances but pending reward fields remain until depositors arrive
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 0;
    config.pending_deposits = 0;
    config.pending_withdrawals = 0;
    config.pending_deposit_fees = 1_000_000; // Has rewards but no deposits

    let result = config.finalize_rewards(INTERVAL);
    // Should succeed - slot advances, rewards preserved
    assert!(result.is_ok());

    // Slot should advance
    assert_eq!(config.last_finalized_slot, INTERVAL);

    // Rewards should be preserved (not reset)
    assert_eq!(config.pending_deposit_fees, 1_000_000);

    // Accumulator should remain 0 (no distribution happened)
    assert_eq!(config.reward_accumulator, 0);
}

#[test]
fn test_finalize_rewards_both_pool_and_rewards_zero() {
    // Edge: No deposits and no rewards - slot advances, nothing else changes
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 0;
    config.pending_deposits = 0;
    config.pending_withdrawals = 0;
    // All pending = 0

    let result = config.finalize_rewards(INTERVAL);
    // Should succeed - slot advances even with empty pool
    assert!(result.is_ok());

    // Slot should advance
    assert_eq!(config.last_finalized_slot, INTERVAL);

    // Everything else remains 0
    assert_eq!(config.finalized_balance, 0);
    assert_eq!(config.reward_accumulator, 0);
}

// =============================================================================
// finalize_rewards() Edge Cases - Overflow Protection
// =============================================================================

#[test]
fn test_finalize_rewards_large_rewards_no_overflow() {
    // Test with maximum reasonable values
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000_000_000; // Very large pool
    config.pending_deposit_fees = u64::MAX; // Maximum u64 fee

    let result = config.finalize_rewards(INTERVAL);
    // pending_rewards (u64::MAX) * ACCUMULATOR_PRECISION should fit in u128
    // u64::MAX * 1e18 ≈ 1.8e19 * 1e18 = 1.8e37, which fits in u128 (max ≈ 3.4e38)
    assert!(result.is_ok());
}

#[test]
fn test_finalize_rewards_accumulator_overflow() {
    // Edge: Accumulator overflow when adding delta to existing
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1; // Minimum pool = huge reward per unit
    config.reward_accumulator = u128::MAX - 1; // Near overflow
    config.pending_deposit_fees = 1;

    let result = config.finalize_rewards(INTERVAL);
    // delta = 1 * 1e18 / 1 = 1e18
    // accumulator = (u128::MAX - 1) + 1e18 would overflow
    assert!(result.is_err());
}

#[test]
fn test_finalize_rewards_total_rewards_distributed_overflow() {
    // Edge: total_rewards_distributed overflow
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000;
    config.total_rewards_distributed = u128::MAX - 100;
    config.pending_deposit_fees = 200; // Would overflow total_rewards_distributed

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_err());
}

// =============================================================================
// finalize_rewards() Precision Tests
// =============================================================================

#[test]
fn test_finalize_rewards_precision_small_rewards_large_pool() {
    // Edge: Very small rewards relative to large pool
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000_000_000_000; // 1 trillion tokens
    config.pending_deposit_fees = 1; // 1 lamport

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // delta = 1 * 1e18 / 1e21 = 1e18 / 1e21 = 0 (truncated)
    // This is expected - very small rewards relative to pool truncate to 0
    assert_eq!(config.reward_accumulator, 0);
}

#[test]
fn test_finalize_rewards_precision_preserved() {
    // Test that accumulator maintains precision for reward calculation
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 100_000_000_000; // 100 tokens
    config.pending_deposit_fees = 1_000_000_000; // 1 token (1% rewards)

    let result = config.finalize_rewards(INTERVAL);
    assert!(result.is_ok());

    // delta = 1e9 * 1e18 / 100e9 = 1e27 / 1e11 = 1e16
    let expected = 1_000_000_000u128 * 1_000_000_000_000_000_000 / 100_000_000_000;
    assert_eq!(config.reward_accumulator, expected);

    // User reward calculation: user_amount * delta / 1e18
    // For user with 10 tokens: 10e9 * 1e16 / 1e18 = 1e8 = 0.1 tokens
    let user_amount = 10_000_000_000u128;
    let user_reward = user_amount * config.reward_accumulator / TokenPoolConfig::ACCUMULATOR_PRECISION;
    assert_eq!(user_reward, 100_000_000); // 0.1 tokens
}

// =============================================================================
// Constants Tests
// =============================================================================

#[test]
fn test_accumulator_precision_constant() {
    assert_eq!(TokenPoolConfig::ACCUMULATOR_PRECISION, 1_000_000_000_000_000_000);
}

#[test]
fn test_update_slot_interval_constant() {
    // Sanity check: interval should be reasonable (10-30 minutes at 400ms/slot)
    assert!(TokenPoolConfig::UPDATE_SLOT_INTERVAL >= 1500); // ~10 min minimum
    assert!(TokenPoolConfig::UPDATE_SLOT_INTERVAL <= 9000); // ~60 min maximum
    assert_eq!(INTERVAL, TokenPoolConfig::UPDATE_SLOT_INTERVAL);
}

// =============================================================================
// Serialization Tests
// =============================================================================

#[test]
fn test_deposit_params_serialization() {
    let params = DepositParams {
        amount: 1_000_000_000,
        expected_output: 990_000_000, // 1% fee
    };

    let data = zorb_pool_interface::build_deposit_instruction_data(&params);
    assert_eq!(data[0], PoolInstruction::Deposit as u8);

    let parsed = zorb_pool_interface::parse_deposit_params(&data).unwrap();
    assert_eq!(parsed.amount, params.amount);
    assert_eq!(parsed.expected_output, params.expected_output);
}

#[test]
fn test_deposit_params_edge_max_values() {
    let params = DepositParams {
        amount: u64::MAX,
        expected_output: u64::MAX - 1,
    };

    let data = zorb_pool_interface::build_deposit_instruction_data(&params);
    let parsed = zorb_pool_interface::parse_deposit_params(&data).unwrap();
    assert_eq!(parsed.amount, u64::MAX);
    assert_eq!(parsed.expected_output, u64::MAX - 1);
}

#[test]
fn test_deposit_params_edge_zero() {
    let params = DepositParams {
        amount: 0,
        expected_output: 0,
    };

    let data = zorb_pool_interface::build_deposit_instruction_data(&params);
    let parsed = zorb_pool_interface::parse_deposit_params(&data).unwrap();
    assert_eq!(parsed.amount, 0);
    assert_eq!(parsed.expected_output, 0);
}

#[test]
fn test_withdraw_params_serialization() {
    let params = WithdrawParams {
        amount: 1_000_000_000,
        expected_output: 990_000_000,
    };

    let data = zorb_pool_interface::build_withdraw_instruction_data(&params);
    assert_eq!(data[0], PoolInstruction::Withdraw as u8);

    let parsed = zorb_pool_interface::parse_withdraw_params(&data).unwrap();
    assert_eq!(parsed.amount, params.amount);
    assert_eq!(parsed.expected_output, params.expected_output);
}

#[test]
fn test_pool_instruction_discriminator() {
    assert_eq!(PoolInstruction::Deposit as u8, 0);
    assert_eq!(PoolInstruction::Withdraw as u8, 1);
    assert_eq!(PoolInstruction::GetInfo as u8, 2);
}

// =============================================================================
// Multi-Finalization Scenario Tests
// =============================================================================

#[test]
fn test_multiple_finalization_cycles() {
    let mut config = default_config();
    config.finalized_balance = 1_000_000_000_000; // 1000 tokens

    // Cycle 1: Deposit fees
    config.pending_deposit_fees = 1_000_000_000;
    config.finalize_rewards(INTERVAL).unwrap();
    let acc1 = config.reward_accumulator;

    // Cycle 2: Withdrawal fees (slot advances by INTERVAL)
    config.pending_withdrawal_fees = 2_000_000_000;
    config.finalize_rewards(INTERVAL * 2).unwrap();
    let acc2 = config.reward_accumulator;

    // Cycle 3: Funded rewards
    config.pending_funded_rewards = 3_000_000_000;
    config.finalize_rewards(INTERVAL * 3).unwrap();
    let acc3 = config.reward_accumulator;

    // Accumulator should monotonically increase
    assert!(acc1 < acc2);
    assert!(acc2 < acc3);

    // total_rewards_distributed = 1 + 2 + 3 = 6 tokens
    assert_eq!(config.total_rewards_distributed, 6_000_000_000);
}

#[test]
fn test_finalization_with_deposits_between() {
    let mut config = default_config();
    config.finalized_balance = 1_000_000_000_000; // 1000 tokens

    // Finalize once
    config.pending_deposit_fees = 10_000_000_000;
    config.finalize_rewards(INTERVAL).unwrap();

    // Simulate deposits between finalizations
    config.pending_deposits = 500_000_000_000; // 500 more tokens deposited
    config.pending_deposit_fees = 5_000_000_000; // Fees from new deposits

    // Finalize again
    config.finalize_rewards(INTERVAL * 2).unwrap();

    // finalized_balance should now include the 500 deposited
    assert_eq!(config.finalized_balance, 1_500_000_000_000);
}

#[test]
fn test_finalization_with_withdrawals_between() {
    let mut config = default_config();
    config.finalized_balance = 1_000_000_000_000; // 1000 tokens

    // Simulate withdrawal between finalizations
    config.pending_withdrawals = 200_000_000_000; // 200 tokens withdrawn
    config.pending_withdrawal_fees = 2_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();

    // finalized_balance should reflect the withdrawal
    assert_eq!(config.finalized_balance, 800_000_000_000);
}

// =============================================================================
// Documentation Example Verification
// =============================================================================

#[test]
fn test_docstring_example() {
    // Verify the example from TokenPoolConfig docstring
    let mut config = default_config();
    config.last_finalized_slot = 0;
    config.finalized_balance = 1_000_000_000_000; // 1000e9

    // User A deposits 500
    config.pending_deposits = 500_000_000_000;
    // User B withdraws 200
    config.pending_withdrawals = 200_000_000_000;
    // Fees collected: 50 tokens (using pending_deposit_fees for simplicity)
    config.pending_deposit_fees = 50_000_000_000;

    config.finalize_rewards(INTERVAL).unwrap();

    // total_pool = 1000e9 + 500e9 - 200e9 = 1300e9
    assert_eq!(config.finalized_balance, 1_300_000_000_000);

    // reward_delta = 50e9 * 1e18 / 1300e9 ≈ 38,461,538,461,538,461
    let expected_delta = 50_000_000_000u128 * 1_000_000_000_000_000_000 / 1_300_000_000_000;
    assert_eq!(config.reward_accumulator, expected_delta);
    // Verify it's approximately the value in the docs
    assert!(config.reward_accumulator > 38_000_000_000_000_000);
    assert!(config.reward_accumulator < 39_000_000_000_000_000);
}
