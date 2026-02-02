//! Token pool instruction helpers.

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;

use super::pda::{SPL_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, find_token_pool_config_pda, find_vault_pda};

/// Token pool instruction discriminators
pub mod discriminators {
    pub const DEPOSIT: u8 = 0;
    pub const WITHDRAW: u8 = 1;
    pub const INIT_POOL: u8 = 64;
    pub const SET_POOL_ACTIVE: u8 = 65;
    pub const SET_FEE_RATES: u8 = 66;
    pub const ADVANCE_EPOCH: u8 = 67;
    pub const FUND_REWARDS: u8 = 68;
    pub const TRANSFER_AUTHORITY: u8 = 192;
    pub const ACCEPT_AUTHORITY: u8 = 193;
}

/// Build instruction data with discriminator and Borsh-serialized args.
fn build_instruction_data<T: BorshSerialize>(discriminator: u8, args: &T) -> Vec<u8> {
    let mut data = vec![discriminator];
    args.serialize(&mut data).unwrap();
    data
}

/// Build instruction data with just the discriminator (no args).
fn build_instruction_data_no_args(discriminator: u8) -> Vec<u8> {
    vec![discriminator]
}

// ============================================================================
// InitPool
// ============================================================================

/// Args for InitPool instruction (matches InitPoolData)
#[derive(BorshSerialize)]
struct InitPoolArgs {
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    _padding: [u8; 4],
}

/// Initialize a new token pool.
///
/// Returns the pool_config PDA on success.
pub fn init_token_pool(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    mint: &Pubkey,
    authority: &Keypair,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<Pubkey, String> {
    let (pool_config, _) = find_token_pool_config_pda(program_id, mint);
    let (vault, _) = find_vault_pda(program_id, &pool_config);

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(pool_config, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            discriminators::INIT_POOL,
            &InitPoolArgs {
                max_deposit_amount,
                deposit_fee_rate,
                withdrawal_fee_rate,
                _padding: [0; 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| pool_config)
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// SetPoolActive
// ============================================================================

/// Args for SetPoolActive instruction
#[derive(BorshSerialize)]
struct SetPoolActiveArgs {
    is_active: u8,
    _padding: [u8; 7],
}

/// Set the active state for a token pool.
pub fn set_token_pool_active(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    pool_config: &Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            discriminators::SET_POOL_ACTIVE,
            &SetPoolActiveArgs {
                is_active: is_active as u8,
                _padding: [0; 7],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// SetFeeRates
// ============================================================================

/// Args for SetFeeRates instruction
#[derive(BorshSerialize)]
struct SetFeeRatesArgs {
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    _padding: [u8; 4],
}

/// Update fee rates for a token pool.
pub fn set_token_pool_fee_rates(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    pool_config: &Pubkey,
    authority: &Keypair,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            discriminators::SET_FEE_RATES,
            &SetFeeRatesArgs {
                deposit_fee_rate,
                withdrawal_fee_rate,
                _padding: [0; 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// AdvanceEpoch
// ============================================================================

/// Advance the reward accumulator epoch.
///
/// This is permissionless - anyone can call it.
pub fn advance_token_epoch(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    pool_config: &Pubkey,
    payer: &Keypair,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(*program_id, false), // token_pool_program for CPI events
        ],
        data: build_instruction_data_no_args(discriminators::ADVANCE_EPOCH),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// FundRewards
// ============================================================================

/// Args for FundRewards instruction
#[derive(BorshSerialize)]
struct FundRewardsArgs {
    amount: u64,
}

/// Fund the reward pool with external tokens.
///
/// This is permissionless - anyone can fund rewards via token transfer.
pub fn fund_token_rewards(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    pool_config: &Pubkey,
    vault: &Pubkey,
    funder_token: &Pubkey,
    funder: &Keypair,
    amount: u64,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new(*vault, false),
            AccountMeta::new(*funder_token, false),
            AccountMeta::new_readonly(funder.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
        ],
        data: build_instruction_data(discriminators::FUND_REWARDS, &FundRewardsArgs { amount }),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&funder.pubkey()),
        &[funder],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// Authority Transfer Instructions
// ============================================================================

/// Initiate authority transfer by setting pending_authority.
pub fn transfer_token_pool_authority(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    pool_config: &Pubkey,
    authority: &Keypair,
    new_authority: &Pubkey,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(*new_authority, false),
        ],
        data: build_instruction_data_no_args(discriminators::TRANSFER_AUTHORITY),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Accept pending authority transfer.
pub fn accept_token_pool_authority(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    pool_config: &Pubkey,
    pending_authority: &Keypair,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(pending_authority.pubkey(), true),
        ],
        data: build_instruction_data_no_args(discriminators::ACCEPT_AUTHORITY),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&pending_authority.pubkey()),
        &[pending_authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}
