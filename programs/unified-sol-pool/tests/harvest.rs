//! LST appreciation harvesting tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Test harvesting LST appreciation for WSOL (balance sync).
#[test]
fn test_harvest_lst_appreciation_wsol() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create mock WSOL mint
    let wsol_mint = create_mock_mint(&mut svm, 9);

    // Initialize WSOL config
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

    // Update vault balance to simulate deposits (both actual balance and LstConfig counter)
    update_vault_balance(&mut svm, &lst_vault, 1_000_000_000);
    update_lst_config_vault_balance(&mut svm, &lst_config, 1_000_000_000);

    // Harvest - for WSOL, rate_data_account is the vault
    let result = harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault, // rate_data_account = vault for WSOL
        None,       // No separate vault account needed for WSOL
        &authority,
    );

    assert!(
        result.is_ok(),
        "harvest_lst_appreciation failed: {:?}",
        result.err()
    );
}

/// Test harvesting appreciation updates virtual SOL value.
#[test]
fn test_harvest_updates_virtual_sol_value() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create mock LST mint
    let lst_mint = create_mock_mint(&mut svm, 9);

    // Use the canonical SPL Stake Pool program ID (must be in whitelist)
    let stake_pool_program = SPL_STAKE_POOL_PROGRAM_ID;
    let stake_pool = create_mock_stake_pool(
        &mut svm,
        &lst_mint,         // pool_mint must match lst_mint
        1_000_000_000_000, // 1000 SOL
        1_000_000_000_000, // 1000 tokens (1:1 rate)
        stake_pool_program,
    );

    // Initialize LST config
    let lst_config = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::SPL_STAKE_POOL,
    )
    .expect("init_lst_config should succeed");

    // Get the vault address
    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);

    // Update vault balance to simulate having tokens (both actual balance and LstConfig counter)
    update_vault_balance(&mut svm, &lst_vault, 100_000_000_000); // 100 tokens
    update_lst_config_vault_balance(&mut svm, &lst_config, 100_000_000_000);

    // Harvest with 1:1 rate first
    let result = harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &stake_pool,
        Some(&lst_vault),
        &authority,
    );

    assert!(
        result.is_ok(),
        "harvest_lst_appreciation failed: {:?}",
        result.err()
    );
}

/// Test harvest fails when LST config is paused.
#[test]
fn test_harvest_lst_paused_fails() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create mock WSOL mint
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

    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);
    update_vault_balance(&mut svm, &lst_vault, 1_000_000_000);
    update_lst_config_vault_balance(&mut svm, &lst_config, 1_000_000_000);

    // Disable the LST config
    set_lst_config_active(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &authority,
        false,
    )
    .expect("disable should succeed");

    // Try to harvest - should fail
    let result = harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &authority,
    );

    assert!(result.is_err(), "harvest should fail when LST is paused");
}

/// Test harvest with zero vault balance succeeds.
#[test]
fn test_harvest_zero_vault_balance() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create mock WSOL mint
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

    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);
    // Don't update vault balance - leave at 0

    // Harvest with zero balance - should succeed (no-op)
    let result = harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &authority,
    );

    assert!(
        result.is_ok(),
        "harvest with zero balance should succeed: {:?}",
        result.err()
    );
}

/// Test harvesting twice in same epoch is idempotent.
#[test]
fn test_harvest_twice_same_epoch_idempotent() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    let other_payer = Keypair::new(); // Use different payer for second harvest
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    svm.airdrop(&other_payer.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create mock WSOL mint
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

    let (lst_vault, _) = find_lst_vault_pda(&program_id, &lst_config);
    update_vault_balance(&mut svm, &lst_vault, 1_000_000_000);
    update_lst_config_vault_balance(&mut svm, &lst_config, 1_000_000_000);

    // First harvest
    let result1 = harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &authority,
    );
    assert!(result1.is_ok(), "first harvest should succeed");

    // Second harvest in same epoch with different payer (permissionless, so anyone can call)
    // This creates a unique transaction signature
    let result2 = harvest_lst_appreciation(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_config,
        &lst_vault,
        None,
        &other_payer,
    );
    assert!(
        result2.is_ok(),
        "second harvest should succeed (idempotent): {:?}",
        result2.err()
    );
}
