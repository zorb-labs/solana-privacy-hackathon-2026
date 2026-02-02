//! Shielded pool authority transfer tests.
//!
//! Tests for TransferAuthority and AcceptAuthority instructions.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;

// ============================================================================
// Authority Transfer Tests
// ============================================================================

/// Test successful authority transfer flow (transfer + accept).
#[test]
fn test_transfer_authority_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, global_config, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create new authority
    let new_authority = Keypair::new();
    svm.airdrop(&new_authority.pubkey(), 10_000_000_000)
        .unwrap();

    // Transfer authority
    let result = transfer_authority(
        &mut svm,
        &program_id,
        &global_config,
        &authority,
        &new_authority.pubkey(),
    );
    assert!(
        result.is_ok(),
        "transfer_authority failed: {:?}",
        result.err()
    );

    // Accept authority
    let result = accept_authority(&mut svm, &program_id, &global_config, &new_authority);
    assert!(
        result.is_ok(),
        "accept_authority failed: {:?}",
        result.err()
    );

    // Verify new authority can perform admin actions (e.g., set_pool_paused)
    let result = set_pool_paused(
        &mut svm,
        &program_id,
        &global_config,
        &new_authority,
        true,
    );
    assert!(
        result.is_ok(),
        "new authority should be able to pause pool: {:?}",
        result.err()
    );

    // Verify old authority can no longer perform admin actions
    let result = set_pool_paused(
        &mut svm,
        &program_id,
        &global_config,
        &authority,
        false,
    );
    assert!(
        result.is_err(),
        "old authority should not be able to unpause pool"
    );
}

/// Test that unauthorized users cannot transfer authority.
#[test]
fn test_transfer_authority_unauthorized() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, global_config, _, _, _authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create an unauthorized user
    let unauthorized = Keypair::new();
    svm.airdrop(&unauthorized.pubkey(), 10_000_000_000).unwrap();

    let new_authority = Keypair::new();

    // Attempt transfer with unauthorized user
    let result = transfer_authority(
        &mut svm,
        &program_id,
        &global_config,
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
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, global_config, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create two new authorities
    let new_authority = Keypair::new();
    svm.airdrop(&new_authority.pubkey(), 10_000_000_000)
        .unwrap();

    let wrong_authority = Keypair::new();
    svm.airdrop(&wrong_authority.pubkey(), 10_000_000_000)
        .unwrap();

    // Transfer to new_authority
    let result = transfer_authority(
        &mut svm,
        &program_id,
        &global_config,
        &authority,
        &new_authority.pubkey(),
    );
    assert!(
        result.is_ok(),
        "transfer_authority failed: {:?}",
        result.err()
    );

    // Try to accept with wrong_authority
    let result = accept_authority(
        &mut svm,
        &program_id,
        &global_config,
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
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, global_config, _, _, _authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create a random user
    let random_user = Keypair::new();
    svm.airdrop(&random_user.pubkey(), 10_000_000_000).unwrap();

    // Try to accept without any pending authority
    let result = accept_authority(&mut svm, &program_id, &global_config, &random_user);
    assert!(
        result.is_err(),
        "accept_authority should fail when no pending authority is set"
    );
}

/// Test that transfer can be overwritten before acceptance.
#[test]
fn test_transfer_authority_overwrite_pending() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, global_config, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create two new authorities
    let new_authority_1 = Keypair::new();
    svm.airdrop(&new_authority_1.pubkey(), 10_000_000_000)
        .unwrap();

    let new_authority_2 = Keypair::new();
    svm.airdrop(&new_authority_2.pubkey(), 10_000_000_000)
        .unwrap();

    // Transfer to first authority
    let result = transfer_authority(
        &mut svm,
        &program_id,
        &global_config,
        &authority,
        &new_authority_1.pubkey(),
    );
    assert!(
        result.is_ok(),
        "first transfer_authority failed: {:?}",
        result.err()
    );

    // Overwrite with second authority
    let result = transfer_authority(
        &mut svm,
        &program_id,
        &global_config,
        &authority,
        &new_authority_2.pubkey(),
    );
    assert!(
        result.is_ok(),
        "second transfer_authority failed: {:?}",
        result.err()
    );

    // First authority should not be able to accept
    let result = accept_authority(
        &mut svm,
        &program_id,
        &global_config,
        &new_authority_1,
    );
    assert!(
        result.is_err(),
        "first authority should not be able to accept after overwrite"
    );

    // Second authority should be able to accept
    let result = accept_authority(
        &mut svm,
        &program_id,
        &global_config,
        &new_authority_2,
    );
    assert!(
        result.is_ok(),
        "second authority should be able to accept: {:?}",
        result.err()
    );
}
