//! Unified SOL pool instruction helpers.

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;

use super::pda::{
    SPL_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, find_lst_config_pda, find_lst_vault_pda,
    find_unified_sol_pool_config_pda,
};

/// Unified SOL pool instruction discriminators
pub mod discriminators {
    pub const INIT_UNIFIED_SOL_POOL_CONFIG: u8 = 64;
    pub const INIT_LST_CONFIG: u8 = 65;
    pub const SET_UNIFIED_SOL_POOL_CONFIG_ACTIVE: u8 = 66;
    pub const SET_LST_CONFIG_ACTIVE: u8 = 67;
    pub const SET_UNIFIED_SOL_POOL_CONFIG_FEE_RATES: u8 = 68;
    pub const FINALIZE_UNIFIED_REWARDS: u8 = 69;
    pub const HARVEST_LST_APPRECIATION: u8 = 70;
    pub const TRANSFER_AUTHORITY: u8 = 192;
    pub const ACCEPT_AUTHORITY: u8 = 193;
}

/// Pool type enum values
pub mod pool_types {
    pub const WSOL: u8 = 0;
    pub const SPL_STAKE_POOL: u8 = 1;
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
// InitUnifiedSolPoolConfig
// ============================================================================

/// Args for InitUnifiedSolPoolConfig instruction
#[derive(BorshSerialize)]
struct InitUnifiedSolPoolConfigArgs {
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    min_buffer_bps: u16,
    _padding: [u8; 2],
    min_buffer_amount: u64,
}

/// Initialize the unified SOL pool configuration.
///
/// Returns the unified_sol_pool_config PDA on success.
pub fn init_unified_sol_pool_config(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    authority: &Keypair,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    min_buffer_bps: u16,
    min_buffer_amount: u64,
) -> Result<Pubkey, String> {
    let (unified_sol_pool_config, _) = find_unified_sol_pool_config_pda(program_id);

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(unified_sol_pool_config, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            discriminators::INIT_UNIFIED_SOL_POOL_CONFIG,
            &InitUnifiedSolPoolConfigArgs {
                max_deposit_amount,
                deposit_fee_rate,
                withdrawal_fee_rate,
                min_buffer_bps,
                _padding: [0; 2],
                min_buffer_amount,
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
        .map(|_| unified_sol_pool_config)
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// InitLstConfig
// ============================================================================

/// Args for InitLstConfig instruction
#[derive(BorshSerialize)]
struct InitLstConfigArgs {
    pool_type: u8,
    _padding: [u8; 7],
}

/// Initialize LST configuration for a specific LST.
///
/// Returns the lst_config PDA on success.
pub fn init_lst_config(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_mint: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_program: &Pubkey,
    authority: &Keypair,
    pool_type: u8,
) -> Result<Pubkey, String> {
    let (lst_config, _) = find_lst_config_pda(program_id, lst_mint);
    let (lst_vault, _) = find_lst_vault_pda(program_id, &lst_config);

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new(lst_config, false),
            AccountMeta::new_readonly(*lst_mint, false),
            AccountMeta::new(lst_vault, false),
            AccountMeta::new_readonly(*stake_pool, false),
            AccountMeta::new_readonly(*stake_pool_program, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            discriminators::INIT_LST_CONFIG,
            &InitLstConfigArgs {
                pool_type,
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
        .map(|_| lst_config)
        .map_err(|e| format!("{:?}", e))
}

// ============================================================================
// SetUnifiedSolPoolConfigActive
// ============================================================================

/// Args for SetUnifiedSolPoolConfigActive instruction
#[derive(BorshSerialize)]
struct SetUnifiedSolPoolConfigActiveArgs {
    is_active: u8,
    _padding: [u8; 7],
}

/// Set the active state for the unified SOL pool config.
pub fn set_unified_sol_pool_config_active(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            discriminators::SET_UNIFIED_SOL_POOL_CONFIG_ACTIVE,
            &SetUnifiedSolPoolConfigActiveArgs {
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
// SetLstConfigActive
// ============================================================================

/// Args for SetLstConfigActive instruction
#[derive(BorshSerialize)]
struct SetLstConfigActiveArgs {
    is_active: u8,
    _padding: [u8; 7],
}

/// Set the active state for an LST config.
pub fn set_lst_config_active(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_config: &Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new_readonly(*unified_sol_pool_config, false),
            AccountMeta::new(*lst_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            discriminators::SET_LST_CONFIG_ACTIVE,
            &SetLstConfigActiveArgs {
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
// FinalizeUnifiedRewards (AdvanceUnifiedEpoch)
// ============================================================================

/// Advance the unified SOL reward epoch.
///
/// This is permissionless. Requires all registered LST configs to be passed
/// and all must have been harvested in the current epoch.
pub fn advance_unified_epoch(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_configs: &[Pubkey],
    payer: &Keypair,
) -> Result<(), String> {
    // FinalizeUnifiedRewardsAccounts has:
    // 1. unified_sol_pool_config (mutable)
    // 2. unified_sol_program (for self-CPI event emission)
    // Then LST configs are passed via remaining_accounts
    let mut accounts = vec![
        AccountMeta::new(*unified_sol_pool_config, false),
        AccountMeta::new_readonly(*program_id, false), // unified_sol_program
    ];

    // Add all LST configs as remaining accounts
    for lst_config in lst_configs {
        accounts.push(AccountMeta::new(*lst_config, false));
    }

    let ix = Instruction {
        program_id: *program_id,
        accounts,
        data: build_instruction_data_no_args(discriminators::FINALIZE_UNIFIED_REWARDS),
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
// HarvestLstAppreciation
// ============================================================================

/// Harvest LST appreciation for a specific LST.
///
/// This is permissionless. For WSOL, rate_data_account is the lst_vault.
/// For SplStakePool, rate_data_account is the stake_pool and lst_vault must
/// be passed as a remaining account.
pub fn harvest_lst_appreciation(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_config: &Pubkey,
    rate_data_account: &Pubkey,
    lst_vault: Option<&Pubkey>,
    payer: &Keypair,
) -> Result<(), String> {
    let mut accounts = vec![
        AccountMeta::new(*unified_sol_pool_config, false),
        AccountMeta::new(*lst_config, false),
        AccountMeta::new_readonly(*rate_data_account, false),
        AccountMeta::new_readonly(*program_id, false), // unified_sol_program for self-CPI events
    ];

    // Add lst_vault as remaining account for SplStakePool type
    if let Some(vault) = lst_vault {
        accounts.push(AccountMeta::new_readonly(*vault, false));
    }

    let ix = Instruction {
        program_id: *program_id,
        accounts,
        data: build_instruction_data_no_args(discriminators::HARVEST_LST_APPRECIATION),
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
// SetUnifiedSolPoolConfigFeeRates
// ============================================================================

/// Args for SetUnifiedSolPoolConfigFeeRates instruction
#[derive(BorshSerialize)]
struct SetUnifiedSolPoolConfigFeeRatesArgs {
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    _padding: [u8; 4],
}

/// Set the fee rates for unified SOL pool config.
pub fn set_unified_sol_pool_config_fee_rates(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    authority: &Keypair,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            discriminators::SET_UNIFIED_SOL_POOL_CONFIG_FEE_RATES,
            &SetUnifiedSolPoolConfigFeeRatesArgs {
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
// Authority Transfer Instructions
// ============================================================================

/// Initiate authority transfer by setting pending_authority.
pub fn transfer_unified_sol_pool_authority(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    authority: &Keypair,
    new_authority: &Pubkey,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
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
pub fn accept_unified_sol_pool_authority(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    pending_authority: &Keypair,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
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
