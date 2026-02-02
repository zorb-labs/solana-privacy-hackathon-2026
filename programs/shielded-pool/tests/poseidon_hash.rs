mod common;

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use shielded_pool::instructions::ShieldedPoolInstruction;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

use common::SHIELDED_POOL_PROGRAM_ID;

/// Build instruction data with discriminator and Borsh-serialized args.
fn build_instruction_data<T: BorshSerialize>(discriminator: u8, args: &T) -> Vec<u8> {
    let mut data = vec![discriminator];
    args.serialize(&mut data).unwrap();
    data
}

#[test]
#[ignore = "Requires rebuilding program with cargo build-sbf after struct changes"]
fn test_poseidon_hash_compute_units() {
    let mut svm = LiteSVM::new();

    // Deploy program
    let program_id = SHIELDED_POOL_PROGRAM_ID;
    let program_data = include_bytes!("../../../target/deploy/shielded_pool.so");
    svm.add_program(program_id, program_data).unwrap();

    // Create payer
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    // Create 32-byte input
    let input: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];

    // Instruction args struct
    #[derive(BorshSerialize)]
    struct PoseidonHashArgs {
        input: [u8; 32],
    }

    // Poseidon hash instruction
    let poseidon_ix = Instruction {
        program_id,
        accounts: vec![AccountMeta::new_readonly(payer.pubkey(), true)],
        data: build_instruction_data(
            ShieldedPoolInstruction::PoseidonHash as u8,
            &PoseidonHashArgs { input },
        ),
    };

    // Create and send transaction
    let tx = Transaction::new_signed_with_payer(
        &[poseidon_ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);

    match &result {
        Ok(tx_metadata) => {
            println!("\n========================================");
            println!("Poseidon Hash Test Results");
            println!("========================================");
            println!("Input (32 bytes): {:?}", input);
            println!("Compute Units Used: {}", tx_metadata.compute_units_consumed);
            println!("Logs:");
            for log in &tx_metadata.logs {
                println!("  {}", log);
            }
            println!("========================================\n");
        }
        Err(e) => {
            println!("Transaction failed: {:?}", e);
        }
    }

    assert!(result.is_ok(), "Transaction should succeed");
}
