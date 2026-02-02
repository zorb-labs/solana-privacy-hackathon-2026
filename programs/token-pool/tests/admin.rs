//! Token pool admin operation tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Test enabling and disabling a pool.
#[test]
fn test_set_pool_active() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Disable the pool (set active = false)
    let disable_result =
        set_token_pool_active(&mut svm, &program_id, &pool_config, &authority, false);
    assert!(disable_result.is_ok(), "set_pool_active(false) failed");

    // Enable the pool (set active = true)
    let enable_result =
        set_token_pool_active(&mut svm, &program_id, &pool_config, &authority, true);
    assert!(enable_result.is_ok(), "set_pool_active(true) failed");
}

/// Test updating fee rates.
#[test]
fn test_set_fee_rates() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool with zero fees
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Update fee rates to 1% deposit, 0.5% withdrawal
    let result = set_token_pool_fee_rates(&mut svm, &program_id, &pool_config, &authority, 100, 50);
    assert!(result.is_ok(), "set_fee_rates failed");
}

/// Test that unauthorized users cannot modify pool settings.
#[test]
fn test_unauthorized_admin_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let other_user = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool with authority
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Try to disable with different user - should fail
    let result = set_token_pool_active(&mut svm, &program_id, &pool_config, &other_user, false);
    assert!(
        result.is_err(),
        "set_pool_active should fail with wrong authority"
    );
}

/// Test that fees can be set to zero (no-fee mode).
#[test]
fn test_set_fee_rates_zero() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool with non-zero fees
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 100, 50)
        .expect("init_pool should succeed");

    // Set fees to zero
    let result = set_token_pool_fee_rates(&mut svm, &program_id, &pool_config, &authority, 0, 0);
    assert!(result.is_ok(), "set_fee_rates(0, 0) should succeed");
}

/// Test that fees can be set to maximum valid value (10000 bps = 100%).
#[test]
fn test_set_fee_rates_max_valid() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Set fees to maximum (100%)
    let result = set_token_pool_fee_rates(
        &mut svm,
        &program_id,
        &pool_config,
        &authority,
        10000,
        10000,
    );
    assert!(result.is_ok(), "set_fee_rates(10000, 10000) should succeed");
}

/// Test that deposit fee cannot exceed 10000 bps (100%).
#[test]
fn test_set_fee_rates_deposit_exceeds_100_percent() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Set deposit fee > 10000 bps - should fail (exceeds 100%)
    let result =
        set_token_pool_fee_rates(&mut svm, &program_id, &pool_config, &authority, 10001, 0);
    assert!(
        result.is_err(),
        "set_fee_rates should fail when deposit fee exceeds 100%"
    );
}

/// Test that withdrawal fee cannot exceed 10000 bps (100%).
#[test]
fn test_set_fee_rates_withdrawal_exceeds_100_percent() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Set withdrawal fee > 10000 bps - should fail (exceeds 100%)
    let result =
        set_token_pool_fee_rates(&mut svm, &program_id, &pool_config, &authority, 0, 10001);
    assert!(
        result.is_err(),
        "set_fee_rates should fail when withdrawal fee exceeds 100%"
    );
}

/// Test that enabling a disabled pool works.
#[test]
fn test_set_pool_enable_after_disable() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Disable the pool
    let result1 = set_token_pool_active(&mut svm, &program_id, &pool_config, &authority, false);
    assert!(result1.is_ok(), "disable should succeed");

    // Enable the pool
    let result2 = set_token_pool_active(&mut svm, &program_id, &pool_config, &authority, true);
    assert!(
        result2.is_ok(),
        "enable should succeed: {:?}",
        result2.err()
    );
}

/// Test that unauthorized users cannot set fee rates.
#[test]
fn test_set_fee_rates_unauthorized() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let other_user = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool with authority
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Try to set fees with different user - should fail
    let result =
        set_token_pool_fee_rates(&mut svm, &program_id, &pool_config, &other_user, 100, 50);
    assert!(
        result.is_err(),
        "set_fee_rates should fail with wrong authority"
    );
}
