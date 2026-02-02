//! Token pool epoch advancement tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Slot interval required between epoch advances (from token-pool state.rs)
/// At 400ms/slot: 2700 slots = 18 minutes
const UPDATE_SLOT_INTERVAL: u64 = 2700;

/// Test successful epoch advancement after sufficient slots.
#[test]
fn test_advance_epoch_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Warp forward past the update interval
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL + 10);

    // Advance epoch should succeed
    let result = advance_token_epoch(&mut svm, &program_id, &pool_config, &authority);
    assert!(result.is_ok(), "advance_epoch failed: {:?}", result.err());
}

/// Test that epoch advancement fails if not enough slots have passed.
#[test]
fn test_advance_epoch_too_early() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let mint = create_mock_mint(&mut svm, 9);

    // Initialize pool
    let pool_config = init_token_pool(&mut svm, &program_id, &mint, &authority, u64::MAX, 0, 0)
        .expect("init_pool should succeed");

    // Try to advance immediately (not enough slots elapsed)
    let result = advance_token_epoch(&mut svm, &program_id, &pool_config, &authority);
    assert!(
        result.is_err(),
        "advance_epoch should fail when called too early"
    );
}
