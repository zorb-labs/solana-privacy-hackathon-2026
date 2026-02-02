//! Unified SOL pool initialization tests.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;

/// Test successful unified SOL config initialization.
#[test]
fn test_init_unified_sol_pool_config() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config
    let result = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,             // max_deposit_amount (0 = no limit)
        100,           // deposit_fee_rate (1%)
        50,            // withdrawal_fee_rate (0.5%)
        2000,          // min_buffer_bps (20%)
        1_000_000_000, // min_buffer_amount (1 SOL)
    );

    assert!(
        result.is_ok(),
        "init_unified_sol_pool_config failed: {:?}",
        result.err()
    );

    let config = result.unwrap();

    // Verify the config account exists and is owned by the program
    let account = svm.get_account(&config).expect("config should exist");
    assert_eq!(
        account.owner, program_id,
        "config should be owned by program"
    );
}

/// Test initializing a WSOL LST config.
#[test]
fn test_init_lst_config_wsol() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config first
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create a mock WSOL mint
    let wsol_mint = create_mock_mint(&mut svm, 9);

    // For WSOL, stake_pool and stake_pool_program can be any accounts
    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    // Initialize LST config for WSOL
    let result = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &wsol_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    );

    assert!(
        result.is_ok(),
        "init_lst_config for WSOL failed: {:?}",
        result.err()
    );

    let lst_config = result.unwrap();

    // Verify the LST config account exists
    let account = svm
        .get_account(&lst_config)
        .expect("lst_config should exist");
    assert_eq!(
        account.owner, program_id,
        "lst_config should be owned by program"
    );

    // Verify the vault was created
    let (vault, _) = find_lst_vault_pda(&program_id, &lst_config);
    let vault_account = svm.get_account(&vault).expect("vault should exist");
    assert_eq!(
        vault_account.owner, SPL_TOKEN_PROGRAM_ID,
        "vault should be owned by token program"
    );
}

/// Test initializing an SPL stake pool LST config.
#[test]
fn test_init_lst_config_spl_stake_pool() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config first
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create a mock LST mint (e.g., vSOL)
    let lst_mint = create_mock_mint(&mut svm, 9);

    // Use the canonical SPL Stake Pool program ID (must be in whitelist)
    let stake_pool_program = SPL_STAKE_POOL_PROGRAM_ID;

    // Create a mock stake pool account with 1:1 initial exchange rate
    let stake_pool = create_mock_stake_pool(
        &mut svm,
        &lst_mint,
        1_000_000_000_000, // 1000 SOL total lamports
        1_000_000_000_000, // 1000 tokens supply (1:1 rate)
        stake_pool_program,
    );

    // Initialize LST config for SPL stake pool
    let result = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &lst_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::SPL_STAKE_POOL,
    );

    assert!(
        result.is_ok(),
        "init_lst_config for SPL stake pool failed: {:?}",
        result.err()
    );
}

/// Test unified SOL pool config initialization fails with fee rate over 100%.
#[test]
fn test_init_unified_sol_pool_config_fee_too_high() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Try with deposit_fee_rate > 10000 bps (over 100%)
    let result = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,     // max_deposit_amount
        10001, // deposit_fee_rate > 100% - should fail
        0,     // withdrawal_fee_rate
        0,     // min_buffer_bps
        0,     // min_buffer_amount
    );
    assert!(
        result.is_err(),
        "init_unified_sol_pool_config should fail with deposit fee > 100%"
    );

    // Also test withdrawal_fee_rate > 10000 bps
    let result2 = init_unified_sol_pool_config(
        &mut svm,
        &program_id,
        &authority,
        0,     // max_deposit_amount
        0,     // deposit_fee_rate
        10001, // withdrawal_fee_rate > 100% - should fail
        0,     // min_buffer_bps
        0,     // min_buffer_amount
    );
    assert!(
        result2.is_err(),
        "init_unified_sol_pool_config should fail with withdrawal fee > 100%"
    );
}

/// Test LST config initialization fails with invalid mint (not owned by token program).
#[test]
fn test_init_lst_config_invalid_mint() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_unified_sol_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize unified SOL config first
    let unified_sol_config =
        init_unified_sol_pool_config(&mut svm, &program_id, &authority, 0, 0, 0, 0, 0)
            .expect("init_unified_sol_pool_config should succeed");

    // Create an "invalid" mint - account not owned by SPL Token program
    let invalid_mint = create_invalid_mint(&mut svm, 9);

    let stake_pool = Pubkey::new_unique();
    let stake_pool_program = Pubkey::new_unique();

    // Try to initialize LST config with invalid mint
    let result = init_lst_config(
        &mut svm,
        &program_id,
        &unified_sol_config,
        &invalid_mint,
        &stake_pool,
        &stake_pool_program,
        &authority,
        pool_types::WSOL,
    );

    assert!(
        result.is_err(),
        "init_lst_config should fail with invalid mint"
    );
}
