//! Tests for the indexed nullifier tree.

mod common;

use litesvm::LiteSVM;
use shielded_pool::{
    instructions::ShieldedPoolInstruction,
    state::{NULLIFIER_TREE_HEIGHT, NullifierIndexedTree},
};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;
use std::mem::size_of;

use common::{SYSTEM_PROGRAM_ID, derive_pdas};

// ============================================================================
// Tests
// ============================================================================

#[test]
#[ignore = "Requires rebuilding program with cargo build-sbf after struct changes"]
fn test_nullifier_tree_initialized_with_pool() {
    // Initialize the pool - this now creates the nullifier tree as well
    let mut svm = LiteSVM::new();
    let program_id = Pubkey::new_unique();
    let program_data = include_bytes!("../../../target/deploy/shielded_pool.so");
    let _ = svm.add_program(program_id, program_data);
    let authority = Keypair::new();
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();
    let (tree_pda, config_pda, receipt_pda, nullifier_tree_pda) = derive_pdas(&program_id);

    // Initialize instruction now creates all 4 accounts including nullifier tree
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
    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );
    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Initialize failed: {:?}", result.err());

    let tx_result = result.unwrap();
    println!("Initialize CU usage: {}", tx_result.compute_units_consumed);

    // Verify nullifier tree was initialized
    let tree_account_data = svm.get_account(&nullifier_tree_pda).unwrap();
    let tree_state: &NullifierIndexedTree = bytemuck::from_bytes(&tree_account_data.data);
    assert_eq!(tree_state.height, NULLIFIER_TREE_HEIGHT);
    assert_eq!(tree_state.next_index, 1); // Genesis leaf inserted
}

#[test]
fn test_nullifier_tree_account_size() {
    let size = size_of::<NullifierIndexedTree>();
    println!("NullifierIndexedTree size: {} bytes", size);
    assert!(size < 10240, "Account size should be under 10KB");
}
