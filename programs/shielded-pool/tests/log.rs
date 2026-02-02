use borsh::BorshSerialize;
use litesvm::LiteSVM;
use shielded_pool::instructions::ShieldedPoolInstruction;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

mod common;
use common::{deploy_program, initialize_shielded_pool};

/// Build instruction data with discriminator and Borsh-serialized args.
fn build_instruction_data<T: BorshSerialize>(discriminator: u8, args: &T) -> Vec<u8> {
    let mut data = vec![discriminator];
    args.serialize(&mut data).unwrap();
    data
}

/// Log instruction args
#[derive(BorshSerialize)]
struct LogArgs {
    data: Vec<u8>,
}

#[test]
fn test_log_requires_signer() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize the pool to get a valid PDA
    let (_, config_pda, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Try to call log with config_pda as non-signer (should fail)
    let log_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(config_pda, false), // Not a signer!
            AccountMeta::new_readonly(program_id, false),
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::Log as u8,
            &LogArgs {
                data: vec![1, 2, 3, 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[log_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Log should fail without signer");
}

#[test]
fn test_log_requires_program_owned_account() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize the pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create a random account not owned by the program
    let random_account = Keypair::new();
    svm.airdrop(&random_account.pubkey(), 1_000_000_000)
        .unwrap();

    // Try to call log with an account not owned by the program
    let log_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(random_account.pubkey(), true), // Signer but wrong owner
            AccountMeta::new_readonly(program_id, false),
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::Log as u8,
            &LogArgs {
                data: vec![1, 2, 3, 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[log_ix],
        Some(&authority.pubkey()),
        &[&authority, &random_account],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_err(),
        "Log should fail with non-program-owned account"
    );
}

#[test]
fn test_log_requires_correct_program_account() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize the pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create a wrong program ID
    let wrong_program_id = Pubkey::new_unique();

    // Try to call log with wrong program account and a regular signer
    // The authority is a regular keypair, not a PDA, so it will fail owner check
    let log_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(authority.pubkey(), true), // Regular signer (wrong owner)
            AccountMeta::new_readonly(wrong_program_id, false),  // Wrong program!
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::Log as u8,
            &LogArgs {
                data: vec![1, 2, 3, 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[log_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // Should fail because either the program account is wrong or the authority isn't program-owned
    assert!(
        result.is_err(),
        "Log should fail with wrong program account or non-program-owned authority"
    );
}

#[test]
fn test_log_missing_accounts() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize the pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Try to call log with no accounts
    let log_ix = Instruction {
        program_id,
        accounts: vec![],
        data: build_instruction_data(
            ShieldedPoolInstruction::Log as u8,
            &LogArgs {
                data: vec![1, 2, 3, 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[log_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_err(), "Log should fail with missing accounts");
}

#[test]
fn test_log_missing_program_account() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize the pool
    let (_, _config_pda, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Create a random account that can sign (but is not program-owned)
    let random_signer = Keypair::new();
    svm.airdrop(&random_signer.pubkey(), 1_000_000_000).unwrap();

    // Try to call log with only one account (missing program account)
    // Use a random signer instead of PDA since PDAs can't sign transactions externally
    let log_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new_readonly(random_signer.pubkey(), true),
            // Missing shielded_pool_program account
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::Log as u8,
            &LogArgs {
                data: vec![1, 2, 3, 4],
            },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[log_ix],
        Some(&authority.pubkey()),
        &[&authority, &random_signer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // Should fail - either because of missing program account or wrong owner
    assert!(
        result.is_err(),
        "Log should fail with missing program account"
    );
}

// Note: A successful log test would require CPI from another instruction
// because the authority must be a PDA that can only sign via invoke_signed.
// The log instruction is designed to be called internally by other instructions
// (like execute_transact) and not directly by external callers.
//
// The successful case is implicitly tested through execute_transact tests
// which emit events via emit_cpi_log.
