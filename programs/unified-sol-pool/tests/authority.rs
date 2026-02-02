//! Unified SOL pool authority transfer tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Test successful authority transfer flow (transfer + accept).
#[test]
fn test_transfer_authority_success() {
    let mut svm = LiteSVM::new();
    let unified_sol_pool_id = deploy_unified_sol_pool_program(&mut svm);

    // Create authority
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL pool config
    let unified_sol_pool_config = init_unified_sol_pool_config(
        &mut svm,
        &unified_sol_pool_id,
        &authority,
        u64::MAX, // max_deposit_amount
        0,        // deposit_fee_rate
        0,        // withdrawal_fee_rate
        0,        // min_buffer_bps
        0,        // min_buffer_amount
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create new authority
    let new_authority = Keypair::new();
    svm.airdrop(&new_authority.pubkey(), 10_000_000_000)
        .unwrap();

    // Transfer authority
    let result = transfer_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &authority,
        &new_authority.pubkey(),
    );
    assert!(
        result.is_ok(),
        "transfer_authority failed: {:?}",
        result.err()
    );

    // Accept authority
    let result = accept_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &new_authority,
    );
    assert!(
        result.is_ok(),
        "accept_authority failed: {:?}",
        result.err()
    );

    // Verify new authority can perform admin actions (e.g., set_unified_sol_pool_config_active)
    let result = set_unified_sol_pool_config_active(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &new_authority,
        false,
    );
    assert!(
        result.is_ok(),
        "new authority should be able to set pool active: {:?}",
        result.err()
    );

    // Verify old authority can no longer perform admin actions
    let result = set_unified_sol_pool_config_active(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &authority,
        true,
    );
    assert!(
        result.is_err(),
        "old authority should not be able to set pool active"
    );
}

/// Test that unauthorized users cannot transfer authority.
#[test]
fn test_transfer_authority_unauthorized() {
    let mut svm = LiteSVM::new();
    let unified_sol_pool_id = deploy_unified_sol_pool_program(&mut svm);

    // Create authority
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL pool config
    let unified_sol_pool_config = init_unified_sol_pool_config(
        &mut svm,
        &unified_sol_pool_id,
        &authority,
        u64::MAX,
        0,
        0,
        0,
        0,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create an unauthorized user
    let unauthorized = Keypair::new();
    svm.airdrop(&unauthorized.pubkey(), 10_000_000_000).unwrap();

    let new_authority = Keypair::new();

    // Attempt transfer with unauthorized user
    let result = transfer_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &unauthorized,
        &new_authority.pubkey(),
    );
    assert!(
        result.is_err(),
        "transfer_authority should fail with wrong authority"
    );
}

/// Test that accept_authority fails with wrong pending authority.
#[test]
fn test_accept_authority_wrong_pending() {
    let mut svm = LiteSVM::new();
    let unified_sol_pool_id = deploy_unified_sol_pool_program(&mut svm);

    // Create authority
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL pool config
    let unified_sol_pool_config = init_unified_sol_pool_config(
        &mut svm,
        &unified_sol_pool_id,
        &authority,
        u64::MAX,
        0,
        0,
        0,
        0,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create two new authorities
    let new_authority = Keypair::new();
    svm.airdrop(&new_authority.pubkey(), 10_000_000_000)
        .unwrap();

    let wrong_authority = Keypair::new();
    svm.airdrop(&wrong_authority.pubkey(), 10_000_000_000)
        .unwrap();

    // Transfer to new_authority
    let result = transfer_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &authority,
        &new_authority.pubkey(),
    );
    assert!(
        result.is_ok(),
        "transfer_authority failed: {:?}",
        result.err()
    );

    // Try to accept with wrong_authority
    let result = accept_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &wrong_authority,
    );
    assert!(
        result.is_err(),
        "accept_authority should fail with wrong pending authority"
    );
}

/// Test that accept_authority fails when no pending authority is set.
#[test]
fn test_accept_authority_no_pending() {
    let mut svm = LiteSVM::new();
    let unified_sol_pool_id = deploy_unified_sol_pool_program(&mut svm);

    // Create authority
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL pool config
    let unified_sol_pool_config = init_unified_sol_pool_config(
        &mut svm,
        &unified_sol_pool_id,
        &authority,
        u64::MAX,
        0,
        0,
        0,
        0,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create a random user
    let random_user = Keypair::new();
    svm.airdrop(&random_user.pubkey(), 10_000_000_000).unwrap();

    // Try to accept without any pending authority
    let result = accept_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &random_user,
    );
    assert!(
        result.is_err(),
        "accept_authority should fail when no pending authority is set"
    );
}

/// Test that transfer can be overwritten before acceptance.
#[test]
fn test_transfer_authority_overwrite_pending() {
    let mut svm = LiteSVM::new();
    let unified_sol_pool_id = deploy_unified_sol_pool_program(&mut svm);

    // Create authority
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL pool config
    let unified_sol_pool_config = init_unified_sol_pool_config(
        &mut svm,
        &unified_sol_pool_id,
        &authority,
        u64::MAX,
        0,
        0,
        0,
        0,
    )
    .expect("init_unified_sol_pool_config should succeed");

    // Create two new authorities
    let new_authority_1 = Keypair::new();
    svm.airdrop(&new_authority_1.pubkey(), 10_000_000_000)
        .unwrap();

    let new_authority_2 = Keypair::new();
    svm.airdrop(&new_authority_2.pubkey(), 10_000_000_000)
        .unwrap();

    // Transfer to first authority
    let result = transfer_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &authority,
        &new_authority_1.pubkey(),
    );
    assert!(
        result.is_ok(),
        "first transfer_authority failed: {:?}",
        result.err()
    );

    // Overwrite with second authority
    let result = transfer_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &authority,
        &new_authority_2.pubkey(),
    );
    assert!(
        result.is_ok(),
        "second transfer_authority failed: {:?}",
        result.err()
    );

    // First authority should not be able to accept
    let result = accept_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &new_authority_1,
    );
    assert!(
        result.is_err(),
        "first authority should not be able to accept after overwrite"
    );

    // Second authority should be able to accept
    let result = accept_unified_sol_pool_authority(
        &mut svm,
        &unified_sol_pool_id,
        &unified_sol_pool_config,
        &new_authority_2,
    );
    assert!(
        result.is_ok(),
        "second authority should be able to accept: {:?}",
        result.err()
    );
}
