//! Program deployment and initialization helpers.

use litesvm::LiteSVM;
use shielded_pool::instructions::ShieldedPoolInstruction;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use super::pda::{
    SHIELDED_POOL_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_POOL_PROGRAM_ID,
    UNIFIED_SOL_POOL_PROGRAM_ID, derive_pdas,
};

/// Deploy the shielded pool program
pub fn deploy_program(svm: &mut LiteSVM) -> Pubkey {
    // Path relative to tests/common/ - works for both top-level and subdirectory tests
    let program_data = include_bytes!("../../../../target/deploy/shielded_pool.so");
    svm.add_program(SHIELDED_POOL_PROGRAM_ID, program_data);
    SHIELDED_POOL_PROGRAM_ID
}

/// Deploy the token pool program
pub fn deploy_token_pool_program(svm: &mut LiteSVM) -> Pubkey {
    let program_data = include_bytes!("../../../../target/deploy/token_pool.so");
    svm.add_program(TOKEN_POOL_PROGRAM_ID, program_data);
    TOKEN_POOL_PROGRAM_ID
}

/// Deploy the unified SOL pool program
pub fn deploy_unified_sol_pool_program(svm: &mut LiteSVM) -> Pubkey {
    let program_data = include_bytes!("../../../../target/deploy/unified_sol_pool.so");
    svm.add_program(UNIFIED_SOL_POOL_PROGRAM_ID, program_data);
    UNIFIED_SOL_POOL_PROGRAM_ID
}

/// Initialize shielded pool and return (tree_account, global_config, receipt_tree, nullifier_tree, authority)
pub fn initialize_shielded_pool(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
) -> (Pubkey, Pubkey, Pubkey, Pubkey, Keypair) {
    let authority = Keypair::new();

    // Airdrop to authority (enough for account creation)
    svm.airdrop(&authority.pubkey(), 10_000_000_000).unwrap();

    // Derive PDA addresses
    let (tree_pda, config_pda, receipt_pda, nullifier_tree_pda) = derive_pdas(program_id);

    // Initialize instruction - just discriminator byte, no data
    let init_ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(tree_pda, false),
            AccountMeta::new(config_pda, false),
            AccountMeta::new(receipt_pda, false),
            AccountMeta::new(nullifier_tree_pda, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(*program_id, false), // shielded_pool_program for event emission
        ],
        data: vec![ShieldedPoolInstruction::Initialize as u8],
    };

    // Create and send transaction
    let tx = Transaction::new_signed_with_payer(
        &[init_ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Initialize failed: {:?}", result.err());

    (
        tree_pda,
        config_pda,
        receipt_pda,
        nullifier_tree_pda,
        authority,
    )
}

/// Warp the slot forward
pub fn warp_to_slot(svm: &mut LiteSVM, slot: u64) {
    svm.warp_to_slot(slot);
}
