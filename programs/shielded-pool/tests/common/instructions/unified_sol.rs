//! Unified SOL instruction helpers.
//!
//! These helpers build instructions for the unified-sol-pool program.

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use unified_sol_pool::UnifiedSolPoolInstruction;

use crate::common::pda::{
    SPL_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, find_lst_config_pda, find_lst_vault_pda,
    find_unified_sol_pool_config_pda,
};

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

// Instruction args structs
#[derive(BorshSerialize)]
struct InitUnifiedSolPoolConfigArgs {
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    min_buffer_bps: u16,
    _padding: [u8; 2],
    min_buffer_amount: u64,
}

#[derive(BorshSerialize)]
struct InitLstConfigArgs {
    pool_type: u8,
}

#[derive(BorshSerialize)]
struct SetUnifiedSolPoolConfigFeeRatesArgs {
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
}

#[derive(BorshSerialize)]
struct SetUnifiedSolPoolConfigActiveArgs {
    is_active: bool,
}

#[derive(BorshSerialize)]
struct SetLstConfigActiveArgs {
    is_active: bool,
}

/// Initialize UnifiedSolPoolConfig
pub fn init_unified_sol_pool_config(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    authority: &Keypair,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    min_buffer_bps: u16,
    min_buffer_amount: u64,
) -> Pubkey {
    let (unified_sol_pool_config, _) =
        find_unified_sol_pool_config_pda(unified_sol_pool_program_id);

    let init_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(unified_sol_pool_config, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::InitUnifiedSolPoolConfig as u8,
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
        &[init_ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "InitUnifiedSolPoolConfig failed: {:?}",
        result.err()
    );

    unified_sol_pool_config
}

/// Initialize LstConfig
/// Returns (lst_config_pda, lst_vault_pda)
pub fn init_lst_config(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_mint: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_program: &Pubkey,
    authority: &Keypair,
    pool_type: u8,
) -> (Pubkey, Pubkey) {
    let (lst_config, _) = find_lst_config_pda(unified_sol_pool_program_id, lst_mint);
    let (lst_vault, _) = find_lst_vault_pda(unified_sol_pool_program_id, &lst_config);

    let init_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new(lst_config, false),
            AccountMeta::new(lst_vault, false),
            AccountMeta::new_readonly(*lst_mint, false),
            AccountMeta::new_readonly(*stake_pool, false),
            AccountMeta::new_readonly(*stake_pool_program, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::InitLstConfig as u8,
            &InitLstConfigArgs { pool_type },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "InitLstConfig failed: {:?}", result.err());

    (lst_config, lst_vault)
}

/// Call harvest_lst_appreciation instruction
pub fn harvest_lst_appreciation(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_config: &Pubkey,
    rate_data_account: &Pubkey,
    lst_vault: Option<&Pubkey>,
) -> Result<(), String> {
    let mut accounts = vec![
        AccountMeta::new(*unified_sol_pool_config, false),
        AccountMeta::new(*lst_config, false),
        AccountMeta::new_readonly(*rate_data_account, false),
    ];

    // For SPL stake pools, need to add lst_vault
    if let Some(vault) = lst_vault {
        accounts.push(AccountMeta::new_readonly(*vault, false));
    }

    let harvest_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts,
        data: build_instruction_data_no_args(
            UnifiedSolPoolInstruction::HarvestLstAppreciation as u8,
        ),
    };

    // Use a generic payer for permissionless instruction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[harvest_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Call advance_unified_epoch instruction with LST configs
pub fn advance_unified_epoch(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_configs: &[Pubkey],
) -> Result<(), String> {
    let mut accounts = vec![AccountMeta::new(*unified_sol_pool_config, false)];

    // Add all LST configs as read-only
    for lst_config in lst_configs {
        accounts.push(AccountMeta::new_readonly(*lst_config, false));
    }

    let advance_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts,
        data: build_instruction_data_no_args(
            UnifiedSolPoolInstruction::FinalizeUnifiedRewards as u8,
        ),
    };

    // Use a generic payer for permissionless instruction
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[advance_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Set unified SOL pool config fee rates
pub fn set_unified_sol_pool_config_fee_rates(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    authority: &Keypair,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::SetUnifiedSolPoolConfigFeeRates as u8,
            &SetUnifiedSolPoolConfigFeeRatesArgs {
                deposit_fee_rate,
                withdrawal_fee_rate,
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

/// Set unified SOL pool config active state
pub fn set_unified_sol_pool_config_active(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::SetUnifiedSolPoolConfigActive as u8,
            &SetUnifiedSolPoolConfigActiveArgs { is_active },
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

/// Set LST config active state
pub fn set_lst_config_active(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    lst_config: &Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(*lst_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::SetLstConfigActive as u8,
            &SetLstConfigActiveArgs { is_active },
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
// Raw instruction helpers for error testing
// ============================================================================

/// Call harvest_lst_appreciation with raw account list
pub fn harvest_lst_appreciation_raw(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    accounts: Vec<AccountMeta>,
) -> Result<(), String> {
    let harvest_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts,
        data: build_instruction_data_no_args(
            UnifiedSolPoolInstruction::HarvestLstAppreciation as u8,
        ),
    };

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[harvest_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Call advance_unified_epoch with raw account list
pub fn advance_unified_epoch_raw(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    accounts: Vec<AccountMeta>,
) -> Result<(), String> {
    let advance_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts,
        data: build_instruction_data_no_args(
            UnifiedSolPoolInstruction::FinalizeUnifiedRewards as u8,
        ),
    };

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[advance_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Try to init unified sol pool config with specific signer (for unauthorized tests)
pub fn init_unified_sol_pool_config_with_signer(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    signer: &Keypair,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    min_buffer_bps: u16,
    min_buffer_amount: u64,
) -> Result<Pubkey, String> {
    let (unified_sol_pool_config, _) =
        find_unified_sol_pool_config_pda(unified_sol_pool_program_id);

    let init_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(unified_sol_pool_config, false),
            AccountMeta::new(signer.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::InitUnifiedSolPoolConfig as u8,
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
        &[init_ix],
        Some(&signer.pubkey()),
        &[signer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| unified_sol_pool_config)
        .map_err(|e| format!("{:?}", e))
}

/// Try to init LST config with specific signer (for unauthorized tests)
pub fn init_lst_config_with_signer(
    svm: &mut LiteSVM,
    unified_sol_pool_program_id: &Pubkey,
    unified_sol_pool_config: &Pubkey,
    lst_mint: &Pubkey,
    stake_pool: &Pubkey,
    stake_pool_program: &Pubkey,
    signer: &Keypair,
    pool_type: u8,
) -> Result<(Pubkey, Pubkey), String> {
    let (lst_config, _) = find_lst_config_pda(unified_sol_pool_program_id, lst_mint);
    let (lst_vault, _) = find_lst_vault_pda(unified_sol_pool_program_id, &lst_config);

    let init_ix = Instruction {
        program_id: *unified_sol_pool_program_id,
        accounts: vec![
            AccountMeta::new(*unified_sol_pool_config, false),
            AccountMeta::new(lst_config, false),
            AccountMeta::new(lst_vault, false),
            AccountMeta::new_readonly(*lst_mint, false),
            AccountMeta::new_readonly(*stake_pool, false),
            AccountMeta::new_readonly(*stake_pool_program, false),
            AccountMeta::new(signer.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            UnifiedSolPoolInstruction::InitLstConfig as u8,
            &InitLstConfigArgs { pool_type },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&signer.pubkey()),
        &[signer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| (lst_config, lst_vault))
        .map_err(|e| format!("{:?}", e))
}
