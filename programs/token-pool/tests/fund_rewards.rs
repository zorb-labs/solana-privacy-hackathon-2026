//! Fund rewards instruction tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Test successful fund rewards.
#[test]
fn test_fund_rewards_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let funder = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&funder.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Get vault PDA
    let (vault, _) = find_vault_pda(&program_id, &pool_config);

    // Create funder token account with tokens
    let funder_token = create_mock_token_account(&mut svm, &mint, &funder.pubkey(), 1_000_000_000);

    // Fund rewards
    let result = fund_token_rewards(
        &mut svm,
        &program_id,
        &pool_config,
        &vault,
        &funder_token,
        &funder,
        500_000_000, // Fund 0.5 tokens as rewards
    );
    assert!(result.is_ok(), "fund_rewards failed: {:?}", result.err());
}

/// Test that fund_rewards updates pending_rewards.
#[test]
fn test_fund_rewards_updates_pending() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let funder = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&funder.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    let (vault, _) = find_vault_pda(&program_id, &pool_config);

    // Create funder token account with tokens
    let funder_token = create_mock_token_account(&mut svm, &mint, &funder.pubkey(), 1_000_000_000);

    // Fund rewards twice
    let amount1 = 100_000_000;
    let amount2 = 200_000_000;

    fund_token_rewards(
        &mut svm,
        &program_id,
        &pool_config,
        &vault,
        &funder_token,
        &funder,
        amount1,
    )
    .expect("first fund_rewards should succeed");

    fund_token_rewards(
        &mut svm,
        &program_id,
        &pool_config,
        &vault,
        &funder_token,
        &funder,
        amount2,
    )
    .expect("second fund_rewards should succeed");

    // Verify vault balance increased
    let vault_balance = get_token_balance(&svm, &vault);
    assert_eq!(
        vault_balance,
        amount1 + amount2,
        "vault should contain funded rewards"
    );

    // Verify pending_rewards was updated in pool_config
    let pending_rewards = get_token_config_pending_funded_rewards(&svm, &pool_config);
    assert_eq!(
        pending_rewards,
        amount1 + amount2,
        "pending_rewards should match funded amount"
    );
}

/// Test that fund_rewards with zero amount fails.
#[test]
fn test_fund_rewards_zero_amount_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let funder = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&funder.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    let (vault, _) = find_vault_pda(&program_id, &pool_config);

    // Create funder token account
    let funder_token = create_mock_token_account(&mut svm, &mint, &funder.pubkey(), 1_000_000_000);

    // Try to fund with zero amount - should fail
    let result = fund_token_rewards(
        &mut svm,
        &program_id,
        &pool_config,
        &vault,
        &funder_token,
        &funder,
        0,
    );
    assert!(result.is_err(), "fund_rewards with zero amount should fail");
}

/// Test that fund_rewards fails when pool is paused.
#[test]
fn test_fund_rewards_pool_paused_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let funder = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&funder.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    let (vault, _) = find_vault_pda(&program_id, &pool_config);

    // Disable the pool
    set_token_pool_active(&mut svm, &program_id, &pool_config, &authority, false)
        .expect("disable should succeed");

    // Create funder token account
    let funder_token = create_mock_token_account(&mut svm, &mint, &funder.pubkey(), 1_000_000_000);

    // Try to fund rewards - should fail because pool is disabled
    let result = fund_token_rewards(
        &mut svm,
        &program_id,
        &pool_config,
        &vault,
        &funder_token,
        &funder,
        500_000_000,
    );
    assert!(
        result.is_err(),
        "fund_rewards should fail when pool is paused"
    );
}

/// Test that anyone can fund rewards (permissionless).
#[test]
fn test_fund_rewards_permissionless() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    let random_funder = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&random_funder.pubkey(), 10_000_000_000)
        .unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool with authority
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    let (vault, _) = find_vault_pda(&program_id, &pool_config);

    // Random user (not authority) creates token account and funds rewards
    let funder_token =
        create_mock_token_account(&mut svm, &mint, &random_funder.pubkey(), 1_000_000_000);

    // Fund rewards from random user - should succeed (permissionless)
    let result = fund_token_rewards(
        &mut svm,
        &program_id,
        &pool_config,
        &vault,
        &funder_token,
        &random_funder,
        500_000_000,
    );
    assert!(
        result.is_ok(),
        "fund_rewards should succeed from any user: {:?}",
        result.err()
    );
}
