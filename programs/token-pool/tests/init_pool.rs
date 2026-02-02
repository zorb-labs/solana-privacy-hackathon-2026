//! Token pool initialization tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Test successful pool initialization.
#[test]
fn test_init_pool_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Create a mock mint with 9 decimals
    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let result = init_token_pool(
        &mut svm,
        &program_id,
        &mint,
        &authority,
        1_000_000_000, // max_deposit_amount
        100,           // deposit_fee_rate (1%)
        50,            // withdrawal_fee_rate (0.5%)
    );

    assert!(result.is_ok(), "init_pool failed: {:?}", result.err());
    let pool_config = result.unwrap();

    // Verify the pool config account exists and is owned by the program
    let account = svm
        .get_account(&pool_config)
        .expect("pool_config should exist");
    assert_eq!(
        account.owner, program_id,
        "pool_config should be owned by program"
    );

    // Verify vault was created
    let (vault, _) = find_vault_pda(&program_id, &pool_config);
    let vault_account = svm.get_account(&vault).expect("vault should exist");
    assert_eq!(
        vault_account.owner, SPL_TOKEN_PROGRAM_ID,
        "vault should be owned by token program"
    );
}

/// Test pool initialization with different token decimals.
#[test]
fn test_init_pool_different_decimals() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 20_000_000_000).unwrap();

    // Create mints with different decimals
    let mint_6 = create_mock_mint(&mut svm, 6); // USDC-like
    let mint_9 = create_mock_mint(&mut svm, 9); // SOL-like

    // Initialize pool for 6-decimal mint
    let result_6 = init_token_pool(&mut svm, &program_id, &mint_6, &authority, u64::MAX, 0, 0);
    assert!(result_6.is_ok(), "init_pool for 6 decimals failed");

    // Initialize pool for 9-decimal mint
    let result_9 = init_token_pool(&mut svm, &program_id, &mint_9, &authority, u64::MAX, 0, 0);
    assert!(result_9.is_ok(), "init_pool for 9 decimals failed");
}

/// Test pool initialization with fee rates.
#[test]
fn test_init_pool_with_fees() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize with 1% deposit fee and 0.5% withdrawal fee
    let result = init_token_pool(
        &mut svm,
        &program_id,
        &mint,
        &authority,
        1_000_000_000_000, // 1000 tokens max deposit
        100,               // 1% deposit fee
        50,                // 0.5% withdrawal fee
    );

    assert!(result.is_ok(), "init_pool with fees failed");
}

/// Test pool initialization fails with invalid mint (not owned by token program).
#[test]
fn test_init_pool_invalid_mint() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Create an "invalid" mint - account not owned by SPL Token program
    let invalid_mint = create_invalid_mint(&mut svm, 9);

    let result = init_token_pool(
        &mut svm,
        &program_id,
        &invalid_mint,
        &authority,
        u64::MAX,
        0,
        0,
    );

    assert!(result.is_err(), "init_pool should fail with invalid mint");
}

/// Test pool initialization fails with fee rate over 100% (10000 basis points).
#[test]
fn test_init_pool_fee_too_high() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Try with deposit_fee_rate > 10000 bps (over 100%)
    let result = init_token_pool(
        &mut svm,
        &program_id,
        &mint,
        &authority,
        u64::MAX,
        10001, // 100.01% - should fail
        0,
    );
    assert!(
        result.is_err(),
        "init_pool should fail with deposit fee > 100%"
    );

    // Also test withdrawal_fee_rate > 10000 bps
    let mint2 = create_mock_mint(&mut svm, 9);
    let result2 = init_token_pool(
        &mut svm,
        &program_id,
        &mint2,
        &authority,
        u64::MAX,
        0,
        10001, // 100.01% - should fail
    );
    assert!(
        result2.is_err(),
        "init_pool should fail with withdrawal fee > 100%"
    );
}
