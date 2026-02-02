//! Admin instruction helpers.

use borsh::BorshSerialize;
use litesvm::LiteSVM;
use shielded_pool::instructions::ShieldedPoolInstruction;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

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

/// Set pool paused state
pub fn set_pool_paused(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    global_config: &Pubkey,
    authority: &Keypair,
    is_paused: bool,
) -> Result<(), String> {
    #[derive(BorshSerialize)]
    struct SetPoolPausedArgs {
        is_paused: u8,
        _padding: [u8; 7],
    }

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*global_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(*program_id, false), // shielded_pool_program for CPI events
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::SetPoolPaused as u8,
            &SetPoolPausedArgs {
                is_paused: is_paused as u8,
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

/// Transfer authority to a new pending authority
pub fn transfer_authority(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    global_config: &Pubkey,
    authority: &Keypair,
    new_authority: &Pubkey,
) -> Result<(), String> {
    // TransferAuthority has no args - new_authority is passed as an account
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*global_config, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
            AccountMeta::new_readonly(*new_authority, false), // new_authority account
            AccountMeta::new_readonly(*program_id, false), // shielded_pool_program for CPI events
        ],
        data: build_instruction_data_no_args(ShieldedPoolInstruction::TransferAuthority as u8),
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

/// Accept authority transfer (called by new authority)
pub fn accept_authority(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    global_config: &Pubkey,
    new_authority: &Keypair,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*global_config, false),
            AccountMeta::new_readonly(new_authority.pubkey(), true),
            AccountMeta::new_readonly(*program_id, false), // shielded_pool_program for CPI events
        ],
        data: build_instruction_data_no_args(ShieldedPoolInstruction::AcceptAuthority as u8),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&new_authority.pubkey()),
        &[new_authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}
