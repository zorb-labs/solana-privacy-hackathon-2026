mod common;

use litesvm::LiteSVM;
use shielded_pool::{
    instructions::ShieldedPoolInstruction,
    state::{CommitmentMerkleTree, GlobalConfig, NullifierIndexedTree, ReceiptMerkleTree},
};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::mem::size_of;

use common::{SHIELDED_POOL_PROGRAM_ID, SYSTEM_PROGRAM_ID, derive_pdas};

#[test]
fn test_admin_pubkey() {
    let address = solana_program::pubkey::Pubkey::from_str_const(
        "8oGzN9C427C3XQywVTHQYLxStB4oCah6SnMbb631ENEB",
    );
    println!("{}", address.to_string());
    println!("{:?}", address.to_bytes());
}

#[test]
#[ignore = "Requires rebuilding program with cargo build-sbf after struct changes"]
fn test_initialize() {
    let mut svm = LiteSVM::new();

    // Deploy program using actual program ID
    let program_id = SHIELDED_POOL_PROGRAM_ID;
    let program_data = include_bytes!("../../../target/deploy/shielded_pool.so");
    svm.add_program(program_id, program_data);

    // Authority (payer)
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Derive PDA addresses
    let (tree_pda, config_pda, receipt_pda, nullifier_tree_pda) = derive_pdas(&program_id);

    // Initialize instruction - just discriminator byte, no data
    let init_ix = Instruction {
        program_id,
        accounts: vec![
            AccountMeta::new(tree_pda, false),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(receipt_pda, false),
            AccountMeta::new(nullifier_tree_pda, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: vec![ShieldedPoolInstruction::Initialize as u8],
    };

    println!(
        "Initialize discriminator: {:?}",
        ShieldedPoolInstruction::Initialize as u8
    );

    // Create and send transaction
    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Initialize failed: {:?}", result.err());

    // Verify accounts were initialized
    // Note: On-chain accounts have 8-byte discriminator prefix + struct data
    let discriminator_size = 8;
    let commitment_tree_size = discriminator_size + size_of::<CommitmentMerkleTree>();
    let receipt_tree_size = discriminator_size + size_of::<ReceiptMerkleTree>();

    let tree_account_data = svm.get_account(&tree_pda).unwrap();
    assert_eq!(tree_account_data.owner, program_id);
    assert_eq!(tree_account_data.data.len(), commitment_tree_size);

    let config_data = svm.get_account(&config_pda).unwrap();
    assert_eq!(config_data.owner, program_id);

    let receipt_tree_data = svm.get_account(&receipt_pda).unwrap();
    assert_eq!(receipt_tree_data.owner, program_id);
    assert_eq!(receipt_tree_data.data.len(), receipt_tree_size);

    // Verify initial values (skip 8-byte discriminator prefix)
    let tree_state: &CommitmentMerkleTree =
        bytemuck::from_bytes(&tree_account_data.data[discriminator_size..]);
    assert_eq!(tree_state.authority, authority.pubkey().to_bytes());
    assert_eq!(tree_state.next_index, 0);
    assert_eq!(tree_state.height, 26);

    let config_state: &GlobalConfig = bytemuck::from_bytes(&config_data.data[discriminator_size..]);
    assert_eq!(config_state.authority, authority.pubkey().to_bytes());

    let receipt_tree_state: &ReceiptMerkleTree =
        bytemuck::from_bytes(&receipt_tree_data.data[discriminator_size..]);
    assert_eq!(receipt_tree_state.authority, authority.pubkey().to_bytes());
    assert_eq!(receipt_tree_state.next_index, 0);
    assert_eq!(receipt_tree_state.height, 26);

    // Verify nullifier tree was initialized
    let nullifier_tree_data = svm.get_account(&nullifier_tree_pda).unwrap();
    assert_eq!(nullifier_tree_data.owner, program_id);
    let nullifier_tree_state: &NullifierIndexedTree =
        bytemuck::from_bytes(&nullifier_tree_data.data[discriminator_size..]);
    assert_eq!(
        nullifier_tree_state.authority,
        authority.pubkey().to_bytes()
    );
    assert_eq!(nullifier_tree_state.next_index, 1); // Genesis leaf inserted
    assert_eq!(nullifier_tree_state.height, 26);
}
