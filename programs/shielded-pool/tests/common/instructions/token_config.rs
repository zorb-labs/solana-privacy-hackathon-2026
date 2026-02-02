//! Token config instruction helpers.
//!
//! These helpers build instructions for the token-pool program.

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use token_pool::TokenPoolInstruction;

use crate::common::pda::{
    SPL_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, find_token_config_pda, find_vault_pda,
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

/// Initialize token pool for a new asset
/// Returns (pool_config, vault) PDAs
pub fn init_token_pool(
    svm: &mut LiteSVM,
    token_pool_program_id: &Pubkey,
    mint: &Pubkey,
    authority: &Keypair,
    max_deposit_amount: u64,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<(Pubkey, Pubkey), String> {
    #[derive(BorshSerialize)]
    struct InitPoolArgs {
        max_deposit_amount: u64,
        deposit_fee_rate: u16,
        withdrawal_fee_rate: u16,
    }

    let (pool_config, _) = find_token_config_pda(token_pool_program_id, mint);
    let (vault, _) = find_vault_pda(token_pool_program_id, &pool_config);

    let ix = Instruction {
        program_id: *token_pool_program_id,
        accounts: vec![
            AccountMeta::new_readonly(*mint, false),
            AccountMeta::new(pool_config, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            TokenPoolInstruction::InitPool as u8,
            &InitPoolArgs {
                max_deposit_amount,
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
        .map(|_| (pool_config, vault))
        .map_err(|e| format!("{:?}", e))
}

// NOTE: sweep_fees function removed - SweepFees instruction no longer exists in TokenPoolInstruction

/// Advance the reward epoch for a token pool
/// Permissionless instruction - uses generic payer
pub fn advance_epoch(
    svm: &mut LiteSVM,
    token_pool_program_id: &Pubkey,
    pool_config: &Pubkey,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *token_pool_program_id,
        accounts: vec![AccountMeta::new(*pool_config, false)],
        data: build_instruction_data_no_args(TokenPoolInstruction::FinalizeRewards as u8),
    };

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Set token pool active state
pub fn set_token_pool_active(
    svm: &mut LiteSVM,
    token_pool_program_id: &Pubkey,
    pool_config: &Pubkey,
    authority: &Keypair,
    is_active: bool,
) -> Result<(), String> {
    #[derive(BorshSerialize)]
    struct SetPoolActiveArgs {
        is_active: bool,
    }

    let ix = Instruction {
        program_id: *token_pool_program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            TokenPoolInstruction::SetPoolActive as u8,
            &SetPoolActiveArgs { is_active },
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

/// Set pool fee rates
pub fn set_fee_rates(
    svm: &mut LiteSVM,
    token_pool_program_id: &Pubkey,
    pool_config: &Pubkey,
    authority: &Keypair,
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<(), String> {
    #[derive(BorshSerialize)]
    struct SetFeeRatesArgs {
        deposit_fee_rate: u16,
        withdrawal_fee_rate: u16,
    }

    let ix = Instruction {
        program_id: *token_pool_program_id,
        accounts: vec![
            AccountMeta::new(*pool_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            TokenPoolInstruction::SetFeeRates as u8,
            &SetFeeRatesArgs {
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
