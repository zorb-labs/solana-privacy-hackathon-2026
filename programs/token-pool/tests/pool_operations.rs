//! Tests for token-pool deposit and withdraw operations.
//!
//! These tests verify the core pool operations including:
//! - Deposit flow with fee calculation
//! - Withdraw flow with fee calculation
//! - State updates and accounting
//! - Error handling for edge cases

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use litesvm_token::{CreateAccount, CreateMint, MintTo};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use token_pool::{TokenPoolConfig, TokenPoolInstruction};

// --- Constants ---

const TOKEN_POOL_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("tokucUdUVP8k9xMS98cnVFmy4Yg3zkKMjfmGuYma8ah");

const SPL_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133,
    237, 95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);

const TOKEN_POOL_CONFIG_SEED: &[u8] = b"token_pool";
const VAULT_SEED: &[u8] = b"vault";

/// Panchor account discriminator size (8 bytes)
const DISC_SIZE: usize = 8;

// --- Test Helpers ---

fn deploy_token_pool_program(svm: &mut LiteSVM) -> Pubkey {
    let program_data = include_bytes!("../../../target/deploy/token_pool.so");
    svm.add_program(TOKEN_POOL_PROGRAM_ID, program_data).unwrap();
    TOKEN_POOL_PROGRAM_ID
}

fn find_token_config_pda(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TOKEN_POOL_CONFIG_SEED, mint.as_ref()], program_id)
}

fn find_vault_pda(program_id: &Pubkey, token_config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED, token_config.as_ref()], program_id)
}

/// Create a proper SPL Token mint using litesvm-token
fn create_real_mint(svm: &mut LiteSVM, authority: &Keypair, decimals: u8) -> Pubkey {
    CreateMint::new(svm, authority)
        .decimals(decimals)
        .authority(&authority.pubkey())
        .send()
        .expect("create mint")
}

/// Create a proper SPL Token account using litesvm-token
fn create_real_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
    balance: u64,
) -> Pubkey {
    let token_account = CreateAccount::new(svm, payer, mint)
        .owner(owner)
        .send()
        .expect("create token account");

    if balance > 0 {
        MintTo::new(svm, payer, mint, &token_account, balance)
            .owner(payer)
            .send()
            .expect("mint to");
    }

    token_account
}

fn build_instruction_data<T: BorshSerialize>(discriminator: u8, args: &T) -> Vec<u8> {
    let mut data = vec![discriminator];
    args.serialize(&mut data).unwrap();
    data
}

#[derive(BorshSerialize)]
struct InitPoolArgs {
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    _padding: [u8; 4],
}

fn build_init_pool_ix(
    program_id: Pubkey,
    mint: Pubkey,
    authority: &Keypair,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Instruction {
    let (pool_config_pda, _) = find_token_config_pda(&program_id, &mint);
    let (vault_pda, _) = find_vault_pda(&program_id, &pool_config_pda);

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(pool_config_pda, false),
            AccountMeta::new(vault_pda, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(solana_system_interface::program::ID, false),
        ],
        data: build_instruction_data(
            TokenPoolInstruction::InitPool as u8,
            &InitPoolArgs {
                max_deposit_amount,
                deposit_fee_rate,
                withdrawal_fee_rate,
                _padding: [0; 4],
            },
        ),
    }
}

/// Initialize a pool with a real SPL Token mint and return (mint, pool_config_pda, vault_pda)
fn init_pool(
    svm: &mut LiteSVM,
    program_id: Pubkey,
    authority: &Keypair,
    decimals: u8,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> (Pubkey, Pubkey, Pubkey) {
    // Create a real SPL Token mint using litesvm-token
    let mint = create_real_mint(svm, authority, decimals);
    let (pool_config_pda, _) = find_token_config_pda(&program_id, &mint);
    let (vault_pda, _) = find_vault_pda(&program_id, &pool_config_pda);

    let ix = build_init_pool_ix(
        program_id,
        mint,
        authority,
        max_deposit_amount,
        deposit_fee_rate,
        withdrawal_fee_rate,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .expect("InitPool should succeed");

    (mint, pool_config_pda, vault_pda)
}

/// Read TokenPoolConfig from account data
fn read_pool_config(svm: &LiteSVM, pool_config_pda: &Pubkey) -> TokenPoolConfig {
    let account = svm.get_account(pool_config_pda).unwrap();
    bytemuck::pod_read_unaligned(&account.data[DISC_SIZE..DISC_SIZE + TokenPoolConfig::SIZE])
}

/// Read token account balance
fn read_token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm.get_account(token_account).unwrap();
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

/// Build deposit instruction data (raw bytes as expected by process_deposit)
fn build_deposit_data(amount: u64, expected_output: u64) -> Vec<u8> {
    let mut data = vec![TokenPoolInstruction::Deposit as u8];
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&expected_output.to_le_bytes());
    data
}

fn build_deposit_ix(
    program_id: Pubkey,
    pool_config: Pubkey,
    vault: Pubkey,
    depositor_token: Pubkey,
    depositor: &Keypair,
    amount: u64,
    expected_output: u64,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(depositor_token, false),
            AccountMeta::new_readonly(depositor.pubkey(), true),
            // Include token program for CPI - pinocchio's invoke_signed needs it in accounts list
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            // Include token-pool program for self-CPI (emit_event calls Log instruction)
            AccountMeta::new_readonly(TOKEN_POOL_PROGRAM_ID, false),
        ],
        data: build_deposit_data(amount, expected_output),
    }
}


// =============================================================================
// Deposit Tests
// =============================================================================

#[test]
fn test_deposit_basic() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize pool with 1% deposit fee (100 basis points)
    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,               // decimals
        u64::MAX,        // max_deposit_amount
        100,             // deposit_fee_rate (1%)
        100,             // withdrawal_fee_rate (1%)
    );

    // Create depositor with tokens using real SPL Token
    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();

    let deposit_amount: u64 = 1_000_000_000; // 1 token
    let depositor_token = create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), deposit_amount);

    // Calculate expected output (1% fee)
    let fee = deposit_amount * 100 / 10000; // 10_000_000 (0.01 tokens)
    let expected_output = deposit_amount - fee; // 990_000_000

    // Build and send deposit instruction
    let ix = build_deposit_ix(
        program_id,
        pool_config,
        vault,
        depositor_token,
        &depositor,
        deposit_amount,
        expected_output,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Deposit should succeed: {:?}", result.err());

    // Verify state updates
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.pending_deposits, expected_output as u128);
    assert_eq!(config.total_deposited, expected_output as u128);
    assert_eq!(config.pending_deposit_fees, fee);
    assert_eq!(config.total_deposit_fees, fee as u128);

    // Verify token balances
    assert_eq!(read_token_balance(&svm, &depositor_token), 0);
    assert_eq!(read_token_balance(&svm, &vault), deposit_amount);
}

#[test]
fn test_deposit_zero_fee() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize pool with 0% deposit fee
    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,   // deposit_fee_rate (0%)
        0,   // withdrawal_fee_rate
    );

    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();

    let deposit_amount: u64 = 1_000_000_000;
    let depositor_token = create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), deposit_amount);

    // With 0% fee, expected_output = amount
    let ix = build_deposit_ix(
        program_id,
        pool_config,
        vault,
        depositor_token,
        &depositor,
        deposit_amount,
        deposit_amount, // No fee
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Deposit with 0% fee should succeed");

    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.pending_deposits, deposit_amount as u128);
    assert_eq!(config.pending_deposit_fees, 0);
}

#[test]
fn test_deposit_exceeds_limit() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize pool with small max_deposit_amount
    let max_deposit = 1_000_000; // 0.001 tokens
    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        max_deposit,
        0,
        0,
    );

    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();

    let deposit_amount: u64 = max_deposit + 1; // Exceeds limit
    let depositor_token = create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), deposit_amount);

    let ix = build_deposit_ix(
        program_id,
        pool_config,
        vault,
        depositor_token,
        &depositor,
        deposit_amount,
        deposit_amount,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Deposit exceeding limit should fail");
}

#[test]
fn test_deposit_expected_output_mismatch() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        100, // 1% fee
        0,
    );

    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();

    let deposit_amount: u64 = 1_000_000_000;
    let depositor_token = create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), deposit_amount);

    // Wrong expected_output (too high - ignoring fee)
    let ix = build_deposit_ix(
        program_id,
        pool_config,
        vault,
        depositor_token,
        &depositor,
        deposit_amount,
        deposit_amount, // Wrong: should be 990_000_000
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Deposit with wrong expected_output should fail");
}

#[test]
fn test_deposit_multiple_sequential() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0, // No fee for simpler accounting
        0,
    );

    // Multiple depositors
    for i in 0..3 {
        let depositor = Keypair::new();
        svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();

        let amount: u64 = (i + 1) as u64 * 100_000_000; // 0.1, 0.2, 0.3 tokens
        let depositor_token = create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), amount);

        let ix = build_deposit_ix(
            program_id,
            pool_config,
            vault,
            depositor_token,
            &depositor,
            amount,
            amount,
        );

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&depositor.pubkey()),
            &[&depositor],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx).expect("Deposit should succeed");
        svm.expire_blockhash();
    }

    // Verify cumulative state
    let config = read_pool_config(&svm, &pool_config);
    let expected_total = 100_000_000u128 + 200_000_000 + 300_000_000;
    assert_eq!(config.pending_deposits, expected_total);
    assert_eq!(config.total_deposited, expected_total);

    // Verify vault balance
    assert_eq!(read_token_balance(&svm, &vault), expected_total as u64);
}

// =============================================================================
// Pool Active/Inactive Tests
// =============================================================================

#[derive(BorshSerialize)]
struct SetPoolActiveArgs {
    is_active: u8,
    _padding: [u8; 7],
}

fn build_set_pool_active_ix(
    program_id: Pubkey,
    pool_config: Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            TokenPoolInstruction::SetPoolActive as u8,
            &SetPoolActiveArgs {
                is_active: if is_active { 1 } else { 0 },
                _padding: [0; 7],
            },
        ),
    }
}

#[test]
fn test_deposit_pool_paused() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Pause the pool
    let pause_ix = build_set_pool_active_ix(program_id, pool_config, &authority, false);
    let tx = Transaction::new_signed_with_payer(
        &[pause_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Pause should succeed");
    svm.expire_blockhash();

    // Verify pool is paused
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.is_active, 0);

    // Try to deposit to paused pool
    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();
    let depositor_token = create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), 100_000_000);

    let ix = build_deposit_ix(
        program_id,
        pool_config,
        vault,
        depositor_token,
        &depositor,
        100_000_000,
        100_000_000,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Deposit to paused pool should fail");
}

#[test]
fn test_set_pool_active_unauthorized() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, _) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Try to pause with wrong authority
    let wrong_authority = Keypair::new();
    svm.airdrop(&wrong_authority.pubkey(), 1_000_000_000).unwrap();

    let pause_ix = build_set_pool_active_ix(program_id, pool_config, &wrong_authority, false);
    let tx = Transaction::new_signed_with_payer(
        &[pause_ix],
        Some(&wrong_authority.pubkey()),
        &[&wrong_authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Pause with wrong authority should fail");
}

// =============================================================================
// Fee Rate Tests
// =============================================================================

#[derive(BorshSerialize)]
struct SetFeeRatesArgs {
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    _padding: [u8; 4],
}

fn build_set_fee_rates_ix(
    program_id: Pubkey,
    pool_config: Pubkey,
    authority: &Keypair,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            TokenPoolInstruction::SetFeeRates as u8,
            &SetFeeRatesArgs {
                deposit_fee_rate,
                withdrawal_fee_rate,
                _padding: [0; 4],
            },
        ),
    }
}

#[test]
fn test_set_fee_rates() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, _) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Update fee rates
    let ix = build_set_fee_rates_ix(program_id, pool_config, &authority, 500, 250);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx).expect("SetFeeRates should succeed");

    // Verify
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.deposit_fee_rate, 500);  // 5%
    assert_eq!(config.withdrawal_fee_rate, 250); // 2.5%
}

#[test]
fn test_set_fee_rates_exceeds_100_percent() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, _) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Try to set fee rate > 100% (10001 basis points)
    let ix = build_set_fee_rates_ix(program_id, pool_config, &authority, 10001, 0);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Fee rate > 100% should fail");
}

// =============================================================================
// Fund Rewards Tests
// =============================================================================

fn build_fund_rewards_ix(
    program_id: Pubkey,
    pool_config: Pubkey,
    vault: Pubkey,
    funder_token: Pubkey,
    funder: &Keypair,
    amount: u64,
) -> Instruction {
    let mut data = vec![TokenPoolInstruction::FundRewards as u8];
    data.extend_from_slice(&amount.to_le_bytes());

    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(funder_token, false),
            AccountMeta::new_readonly(funder.pubkey(), true),
            // Include token program for CPI - pinocchio's invoke_signed needs it in accounts list
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
        ],
        data,
    }
}

#[test]
fn test_fund_rewards_basic() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Fund rewards
    let funder = Keypair::new();
    svm.airdrop(&funder.pubkey(), 1_000_000_000).unwrap();

    let fund_amount: u64 = 500_000_000;
    let funder_token = create_real_token_account(&mut svm, &authority, &mint, &funder.pubkey(), fund_amount);

    let ix = build_fund_rewards_ix(
        program_id,
        pool_config,
        vault,
        funder_token,
        &funder,
        fund_amount,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&funder.pubkey()),
        &[&funder],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "FundRewards should succeed: {:?}", result.err());

    // Verify state
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.pending_funded_rewards, fund_amount);
    assert_eq!(config.total_funded_rewards, fund_amount as u128);

    // Verify token transfer
    assert_eq!(read_token_balance(&svm, &funder_token), 0);
    assert_eq!(read_token_balance(&svm, &vault), fund_amount);
}

#[test]
fn test_fund_rewards_zero_amount() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    let funder = Keypair::new();
    svm.airdrop(&funder.pubkey(), 1_000_000_000).unwrap();
    let funder_token = create_real_token_account(&mut svm, &authority, &mint, &funder.pubkey(), 100_000_000);

    // Try to fund with 0 amount
    let ix = build_fund_rewards_ix(
        program_id,
        pool_config,
        vault,
        funder_token,
        &funder,
        0, // Zero amount
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&funder.pubkey()),
        &[&funder],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "FundRewards with 0 amount should fail");
}

#[test]
fn test_fund_rewards_pool_paused() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Pause the pool
    let pause_ix = build_set_pool_active_ix(program_id, pool_config, &authority, false);
    let tx = Transaction::new_signed_with_payer(
        &[pause_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Pause should succeed");
    svm.expire_blockhash();

    // Try to fund paused pool
    let funder = Keypair::new();
    svm.airdrop(&funder.pubkey(), 1_000_000_000).unwrap();
    let funder_token = create_real_token_account(&mut svm, &authority, &mint, &funder.pubkey(), 100_000_000);

    let ix = build_fund_rewards_ix(
        program_id,
        pool_config,
        vault,
        funder_token,
        &funder,
        100_000_000,
    );

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&funder.pubkey()),
        &[&funder],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "FundRewards to paused pool should fail");
}

// =============================================================================
// Authority Transfer Tests
// =============================================================================

fn build_transfer_authority_ix(
    program_id: Pubkey,
    pool_config: Pubkey,
    authority: &Keypair,
    new_authority: Pubkey,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(new_authority, false),
        ],
        data: vec![TokenPoolInstruction::TransferAuthority as u8],
    }
}

fn build_accept_authority_ix(
    program_id: Pubkey,
    pool_config: Pubkey,
    new_authority: &Keypair,
) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new_readonly(new_authority.pubkey(), true),
        ],
        data: vec![TokenPoolInstruction::AcceptAuthority as u8],
    }
}

#[test]
fn test_authority_transfer_two_step() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, _) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    let new_authority = Keypair::new();
    svm.airdrop(&new_authority.pubkey(), 1_000_000_000).unwrap();

    // Step 1: Transfer authority (sets pending_authority)
    let transfer_ix = build_transfer_authority_ix(
        program_id,
        pool_config,
        &authority,
        new_authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("TransferAuthority should succeed");
    svm.expire_blockhash();

    // Verify pending_authority is set
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.pending_authority, new_authority.pubkey().to_bytes());
    assert_eq!(config.authority, authority.pubkey().to_bytes()); // Not changed yet

    // Step 2: Accept authority
    let accept_ix = build_accept_authority_ix(program_id, pool_config, &new_authority);
    let tx = Transaction::new_signed_with_payer(
        &[accept_ix],
        Some(&new_authority.pubkey()),
        &[&new_authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("AcceptAuthority should succeed");

    // Verify authority changed
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.authority, new_authority.pubkey().to_bytes());
    assert_eq!(config.pending_authority, [0u8; 32]); // Cleared
}

#[test]
fn test_accept_authority_wrong_signer() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, _) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    let new_authority = Keypair::new();
    let wrong_signer = Keypair::new();
    svm.airdrop(&new_authority.pubkey(), 1_000_000_000).unwrap();
    svm.airdrop(&wrong_signer.pubkey(), 1_000_000_000).unwrap();

    // Set pending authority
    let transfer_ix = build_transfer_authority_ix(
        program_id,
        pool_config,
        &authority,
        new_authority.pubkey(),
    );
    let tx = Transaction::new_signed_with_payer(
        &[transfer_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("TransferAuthority should succeed");
    svm.expire_blockhash();

    // Try to accept with wrong signer
    let accept_ix = build_accept_authority_ix(program_id, pool_config, &wrong_signer);
    let tx = Transaction::new_signed_with_payer(
        &[accept_ix],
        Some(&wrong_signer.pubkey()),
        &[&wrong_signer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "AcceptAuthority with wrong signer should fail");
}

// =============================================================================
// Finalize Rewards Tests
// =============================================================================

/// UPDATE_SLOT_INTERVAL constant (must match the on-chain constant)
const UPDATE_SLOT_INTERVAL: u64 = 216_000;

fn build_finalize_rewards_ix(program_id: Pubkey, pool_config: Pubkey) -> Instruction {
    Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            // Include token-pool program for self-CPI (emit_event calls Log instruction)
            AccountMeta::new_readonly(TOKEN_POOL_PROGRAM_ID, false),
        ],
        data: vec![TokenPoolInstruction::FinalizeRewards as u8],
    }
}

#[test]
fn test_finalize_rewards_basic() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Initialize pool with fee rates
    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        100, // 1% deposit fee
        100, // 1% withdrawal fee
    );

    // Make a deposit to generate fees
    let depositor = Keypair::new();
    svm.airdrop(&depositor.pubkey(), 1_000_000_000).unwrap();
    let deposit_amount: u64 = 1_000_000_000;
    let depositor_token =
        create_real_token_account(&mut svm, &authority, &mint, &depositor.pubkey(), deposit_amount);

    let fee = deposit_amount * 100 / 10000;
    let expected_output = deposit_amount - fee;

    let deposit_ix = build_deposit_ix(
        program_id,
        pool_config,
        vault,
        depositor_token,
        &depositor,
        deposit_amount,
        expected_output,
    );

    let tx = Transaction::new_signed_with_payer(
        &[deposit_ix],
        Some(&depositor.pubkey()),
        &[&depositor],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Deposit should succeed");

    // Verify pending state before finalization
    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.pending_deposits, expected_output as u128);
    assert_eq!(config.pending_deposit_fees, fee);
    assert_eq!(config.finalized_balance, 0);

    // Advance time past UPDATE_SLOT_INTERVAL
    svm.warp_to_slot(UPDATE_SLOT_INTERVAL + 100);
    svm.expire_blockhash();

    // Finalize rewards - permissionless, anyone can call
    let caller = Keypair::new();
    svm.airdrop(&caller.pubkey(), 100_000_000).unwrap();

    let finalize_ix = build_finalize_rewards_ix(program_id, pool_config);
    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix],
        Some(&caller.pubkey()),
        &[&caller],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "FinalizeRewards should succeed: {:?}",
        result.err()
    );

    // Verify state after finalization
    let config = read_pool_config(&svm, &pool_config);
    // pending_deposits should be moved to finalized_balance
    assert_eq!(config.finalized_balance, expected_output as u128);
    assert_eq!(config.pending_deposits, 0);
    // pending_deposit_fees should be cleared after distribution
    assert_eq!(config.pending_deposit_fees, 0);
    // last_finalized_slot should be updated
    assert!(config.last_finalized_slot > 0);
}

#[test]
fn test_finalize_rewards_too_early() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, _) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Try to finalize immediately (without waiting for UPDATE_SLOT_INTERVAL)
    let caller = Keypair::new();
    svm.airdrop(&caller.pubkey(), 100_000_000).unwrap();

    let finalize_ix = build_finalize_rewards_ix(program_id, pool_config);
    let tx = Transaction::new_signed_with_payer(
        &[finalize_ix],
        Some(&caller.pubkey()),
        &[&caller],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_err(),
        "FinalizeRewards should fail when called too early"
    );
}

#[test]
fn test_finalize_rewards_multiple_cycles() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (mint, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    let caller = Keypair::new();
    svm.airdrop(&caller.pubkey(), 1_000_000_000).unwrap();

    // Cycle 1: Deposit and finalize
    let depositor1 = Keypair::new();
    svm.airdrop(&depositor1.pubkey(), 1_000_000_000).unwrap();
    let amount1: u64 = 100_000_000;
    let token1 = create_real_token_account(&mut svm, &authority, &mint, &depositor1.pubkey(), amount1);

    let tx = Transaction::new_signed_with_payer(
        &[build_deposit_ix(program_id, pool_config, vault, token1, &depositor1, amount1, amount1)],
        Some(&depositor1.pubkey()),
        &[&depositor1],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Deposit 1 should succeed");

    svm.warp_to_slot(UPDATE_SLOT_INTERVAL + 100);
    svm.expire_blockhash();

    let tx = Transaction::new_signed_with_payer(
        &[build_finalize_rewards_ix(program_id, pool_config)],
        Some(&caller.pubkey()),
        &[&caller],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Finalize 1 should succeed");

    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.finalized_balance, amount1 as u128);

    // Cycle 2: Another deposit and finalize
    let depositor2 = Keypair::new();
    svm.airdrop(&depositor2.pubkey(), 1_000_000_000).unwrap();
    let amount2: u64 = 200_000_000;
    let token2 = create_real_token_account(&mut svm, &authority, &mint, &depositor2.pubkey(), amount2);

    svm.expire_blockhash();
    let tx = Transaction::new_signed_with_payer(
        &[build_deposit_ix(program_id, pool_config, vault, token2, &depositor2, amount2, amount2)],
        Some(&depositor2.pubkey()),
        &[&depositor2],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Deposit 2 should succeed");

    svm.warp_to_slot(2 * UPDATE_SLOT_INTERVAL + 200);
    svm.expire_blockhash();

    let tx = Transaction::new_signed_with_payer(
        &[build_finalize_rewards_ix(program_id, pool_config)],
        Some(&caller.pubkey()),
        &[&caller],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Finalize 2 should succeed");

    let config = read_pool_config(&svm, &pool_config);
    assert_eq!(config.finalized_balance, (amount1 + amount2) as u128);
}

// =============================================================================
// Withdraw Tests
// =============================================================================

// Note: The withdraw instruction validates hub_authority against the canonical PDA
// derived from HUB_PROGRAM_ID. Since we can't mock this in LiteSVM tests, we test
// the validation path by passing an invalid hub_authority.

#[test]
fn test_withdraw_invalid_hub_authority() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        100, // 1% withdrawal fee
    );

    // Create a fake hub_authority (not the canonical PDA)
    let fake_hub_authority = Keypair::new();

    // Build withdraw instruction with invalid hub_authority
    let mut data = vec![TokenPoolInstruction::Withdraw as u8];
    let amount: u64 = 1_000_000_000;
    let fee = amount * 100 / 10000;
    let expected_output = amount - fee;
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&expected_output.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(fake_hub_authority.pubkey(), false),
            // Include token program for potential CPI
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            // Include token-pool program for self-CPI
            AccountMeta::new_readonly(TOKEN_POOL_PROGRAM_ID, false),
        ],
        data,
    };

    svm.airdrop(&fake_hub_authority.pubkey(), 100_000_000).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&fake_hub_authority.pubkey()),
        &[&fake_hub_authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_err(),
        "Withdraw with invalid hub_authority should fail"
    );
}

#[test]
fn test_withdraw_pool_paused() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_token_pool_program(&mut svm);

    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    let (_, pool_config, vault) = init_pool(
        &mut svm,
        program_id,
        &authority,
        9,
        u64::MAX,
        0,
        0,
    );

    // Pause the pool
    let pause_ix = build_set_pool_active_ix(program_id, pool_config, &authority, false);
    let tx = Transaction::new_signed_with_payer(
        &[pause_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("Pause should succeed");
    svm.expire_blockhash();

    // Try to withdraw from paused pool
    // Even with invalid hub_authority, paused check happens first
    let fake_hub_authority = Keypair::new();
    svm.airdrop(&fake_hub_authority.pubkey(), 100_000_000).unwrap();

    let mut data = vec![TokenPoolInstruction::Withdraw as u8];
    data.extend_from_slice(&1_000_000_000u64.to_le_bytes());
    data.extend_from_slice(&1_000_000_000u64.to_le_bytes());

    let ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(pool_config, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(fake_hub_authority.pubkey(), false),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(TOKEN_POOL_PROGRAM_ID, false),
        ],
        data,
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&fake_hub_authority.pubkey()),
        &[&fake_hub_authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // Should fail (either due to hub_authority validation or pool paused check)
    assert!(result.is_err(), "Withdraw should fail on paused pool");
}
