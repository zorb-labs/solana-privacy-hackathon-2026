//! Unified SOL pool epoch advancement tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Slot interval required between epoch advances
/// At 400ms/slot: 2700 slots ~ 18 minutes
const UPDATE_SLOT_INTERVAL: u64 = 2700;

/// Test successful unified epoch advancement.
#[test]
fn test_advance_unified_epoch() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create and initialize one LST config (WSOL for simplicity)
    let wsol_mint = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &wsol_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config should succeed");

    // Get the vault address
    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);

    // Harvest the LST first (required before epoch advance)
    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &authority,
    )
    .expect("harvest should succeed");

    // Warp forward past the update interval
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL + 10);

    // Advance epoch - pass all LST configs
    let result = advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config],
        &authority,
    );

    assert!(
        result.is_ok(),
        "advance_unified_epoch failed: {:?}",
        result.err()
    );
}

/// Test that epoch advancement requires all LSTs to be harvested.
///
/// This test validates that after an epoch advance, newly added LSTs
/// must be harvested before the next epoch can advance.
#[test]
fn test_advance_epoch_requires_harvest() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 30_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create and initialize first LST config
    let mint1 = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config1 = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &mint1,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config 1 should succeed");

    let (lst_vault1, _) = find_lst_vault_pda(&program_id, &lst_config1);

    // Harvest the first LST for epoch 1 (reward_epoch starts at 1, not 0)
    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config1,
        &lst_vault1,
        None,
        &authority,
    )
    .expect("harvest should succeed");

    // Warp forward and advance to epoch 2 (reward_epoch starts at 1)
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL + 10);
    advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config1],
        &authority,
    )
    .expect("first advance should succeed");

    // Now add a second LST (its last_harvest_epoch will be 0, but we're at epoch 2)
    let mint2 = create_mock_mint(&mut svm, 9);
    let lst_config2 = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &mint2,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config 2 should succeed");

    // DON'T harvest either LST for epoch 2
    // lst_config1: last_harvest_epoch = 1 (from harvest before first advance)
    // lst_config2: last_harvest_epoch = 0 (from initialization)
    // Current epoch is now 2, so both are "stale"

    // Warp forward again
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL * 2 + 20);

    // Try to advance epoch with both LSTs - should fail because neither is harvested for epoch 2
    let result = advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config1, lst_config2],
        &authority,
    );

    assert!(
        result.is_err(),
        "advance_unified_epoch should fail when LSTs not harvested"
    );
}

/// Test that epoch advancement fails when pool is paused.
#[test]
fn test_advance_epoch_pool_paused_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create and initialize one LST config
    let wsol_mint = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &wsol_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config should succeed");

    // Harvest the LST
    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);
    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &authority,
    )
    .expect("harvest should succeed");

    // Disable the unified config
    set_unified_sol_pool_config_active(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &authority,
        false,
    )
    .expect("disable should succeed");

    // Warp forward past the update interval
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL + 10);

    // Try to advance epoch - should fail because pool is disabled
    let result = advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config],
        &authority,
    );

    assert!(
        result.is_err(),
        "advance_unified_epoch should fail when pool is paused"
    );
}

/// Test that epoch advancement with wrong LST count fails.
#[test]
fn test_advance_epoch_wrong_lst_count_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 20_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create and initialize two LST configs
    let mint1 = create_mock_mint(&mut svm, 9);
    let mint2 = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config1 = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &mint1,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config 1 should succeed");

    let _lst_config2 = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &mint2,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config 2 should succeed");

    // Harvest the first LST
    let (lst_vault1, _) = find_lst_vault_pda(&program_id, &lst_config1);
    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config1,
        &lst_vault1,
        None,
        &authority,
    )
    .expect("harvest should succeed");

    // Warp forward
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL + 10);

    // Try to advance epoch with only one LST config (when 2 exist) - should fail
    let result = advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config1], // Missing lst_config2
        &authority,
    );

    assert!(
        result.is_err(),
        "advance_unified_epoch should fail with wrong LST count"
    );
}

/// Test that epoch advancement with multiple LSTs succeeds.
#[test]
fn test_advance_epoch_multiple_lsts_success() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 20_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create and initialize two LST configs
    let mint1 = create_mock_mint(&mut svm, 9);
    let mint2 = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config1 = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &mint1,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config 1 should succeed");

    let lst_config2 = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &mint2,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config 2 should succeed");

    // Harvest both LSTs
    let (lst_vault1, _) = find_lst_vault_pda(&program_id, &lst_config1);
    let (lst_vault2, _) = find_lst_vault_pda(&program_id, &lst_config2);

    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config1,
        &lst_vault1,
        None,
        &authority,
    )
    .expect("harvest 1 should succeed");

    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config2,
        &lst_vault2,
        None,
        &authority,
    )
    .expect("harvest 2 should succeed");

    // Warp forward
    warp_to_slot(&mut svm, UPDATE_SLOT_INTERVAL + 10);

    // Advance epoch with both LST configs
    let result = advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config1, lst_config2],
        &authority,
    );

    assert!(
        result.is_ok(),
        "advance_unified_epoch should succeed with multiple LSTs: {:?}",
        result.err()
    );
}

/// Test that epoch advancement too early fails.
#[test]
fn test_advance_epoch_too_early_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create and initialize one LST config
    let wsol_mint = create_mock_mint(&mut svm, 9);
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &wsol_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    )
    .expect("init_lst_config should succeed");

    // Harvest the LST
    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);
    harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &authority,
    )
    .expect("harvest should succeed");

    // DON'T warp forward - try to advance immediately

    // Try to advance epoch - should fail because not enough slots passed
    let result = advance_unified_epoch(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &[lst_config],
        &authority,
    );

    assert!(
        result.is_err(),
        "advance_unified_epoch should fail when called too early"
    );
}
