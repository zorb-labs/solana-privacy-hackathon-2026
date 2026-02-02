//! Unified SOL pool admin operation tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

// ============================================================================
// SetUnifiedConfigFeeRates Tests
// ============================================================================

/// Test successful fee rate update.
#[test]
fn test_set_unified_fee_rates_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified config with initial fees
    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,             // max_deposit_amount (0 = no limit)
        100,           // deposit_fee_rate (1%)
        50,            // withdrawal_fee_rate (0.5%)
        2000,          // min_buffer_bps (20%)
        1_000_000_000, // min_buffer_amount
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Update fee rates
    let result = set_unified_sol_pool_config_fee_rates(
        &mut svm,
        &program_id,
        &unified_config,
        &authority,
        200, // new deposit_fee_rate (2%)
        100, // new withdrawal_fee_rate (1%)
    );
    assert!(
        result.is_ok(),
        "set_unified_fee_rates failed: {:?}",
        result.err()
    );
}

/// Test that unauthorized users cannot update fee rates.
#[test]
fn test_set_unified_fee_rates_unauthorized() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    let other_user = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified config
    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Try to update fee rates with wrong authority
    let result = set_unified_sol_pool_config_fee_rates(
        &mut svm,
        &program_id,
        &unified_config,
        &other_user,
        200,
        100,
    );
    assert!(
        result.is_err(),
        "set_unified_fee_rates should fail with wrong authority"
    );
}

/// Test that deposit fee exceeding 10000 bps fails.
#[test]
fn test_set_unified_fee_rates_deposit_too_high() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Try to set deposit fee > 10000 bps
    let result = set_unified_sol_pool_config_fee_rates(
        &mut svm,
        &program_id,
        &unified_config,
        &authority,
        10001, // > 100%
        50,
    );
    assert!(
        result.is_err(),
        "set_unified_fee_rates with deposit > 10000 bps should fail"
    );
}

/// Test that withdrawal fee exceeding 10000 bps fails.
#[test]
fn test_set_unified_fee_rates_withdrawal_too_high() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Try to set withdrawal fee > 10000 bps
    let result = set_unified_sol_pool_config_fee_rates(
        &mut svm,
        &program_id,
        &unified_config,
        &authority,
        100,
        10001, // > 100%
    );
    assert!(
        result.is_err(),
        "set_unified_fee_rates with withdrawal > 10000 bps should fail"
    );
}

// ============================================================================
// SetUnifiedSolConfigActive Tests
// ============================================================================

/// Test successful enable/disable of unified config.
#[test]
fn test_set_unified_config_active_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Disable
    let result = set_unified_sol_pool_config_active(
        &mut svm,
        &program_id,
        &unified_config,
        &authority,
        false,
    );
    assert!(result.is_ok(), "disable should succeed");

    // Enable
    let result = set_unified_sol_pool_config_active(
        &mut svm,
        &program_id,
        &unified_config,
        &authority,
        true,
    );
    assert!(result.is_ok(), "enable should succeed");
}

/// Test that unauthorized users cannot change unified config active state.
#[test]
fn test_set_unified_config_active_unauthorized() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    let other_user = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Try to disable with wrong authority
    let result = set_unified_sol_pool_config_active(
        &mut svm,
        &program_id,
        &unified_config,
        &other_user,
        false,
    );
    assert!(result.is_err(), "disable should fail with wrong authority");
}

// ============================================================================
// SetLstConfigActive Tests
// ============================================================================

/// Test successful enable/disable of LST config.
#[test]
fn test_set_lst_config_active_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified config
    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create mock WSOL mint
    let wsol_mint = create_mock_mint(&mut svm, 9);

    // For WSOL, stake_pool is just a placeholder (not used)
    let stake_pool_placeholder = wsol_mint;

    // Initialize LST config for WSOL
    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_config,
        &wsol_mint,
        &stake_pool_placeholder,
        &SPL_TOKEN_PROGRAM_ID,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config should succeed");

    // Disable LST config
    let result = set_lst_config_active(
        &mut svm,
        &program_id,
        &unified_config,
        &lst_config,
        &authority,
        false,
    );
    assert!(result.is_ok(), "disable lst_config should succeed");

    // Enable LST config
    let result = set_lst_config_active(
        &mut svm,
        &program_id,
        &unified_config,
        &lst_config,
        &authority,
        true,
    );
    assert!(result.is_ok(), "enable lst_config should succeed");
}

/// Test that unauthorized users cannot change LST config active state.
#[test]
fn test_set_lst_config_active_unauthorized() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    let other_user = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified config
    let unified_config = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,
        100,
        50,
        2000,
        1_000_000_000,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create mock WSOL mint
    let wsol_mint = create_mock_mint(&mut svm, 9);
    let stake_pool_placeholder = wsol_mint;

    // Initialize LST config for WSOL
    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_config,
        &wsol_mint,
        &stake_pool_placeholder,
        &SPL_TOKEN_PROGRAM_ID,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config should succeed");

    // Try to disable LST config with wrong authority
    let result = set_lst_config_active(
        &mut svm,
        &program_id,
        &unified_config,
        &lst_config,
        &other_user,
        false,
    );
    assert!(
        result.is_err(),
        "disable lst_config should fail with wrong authority"
    );
}
