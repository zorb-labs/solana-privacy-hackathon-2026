//! Transact instruction helpers.

use borsh::BorshSerialize;
use bytemuck;
use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use shielded_pool::instructions::{ShieldedPoolInstruction, TransactParams, TransactProofData};
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

use crate::common::pda::SYSTEM_PROGRAM_ID;

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
struct InitTransactSessionArgs {
    nonce: u64,
    data_len: u32,
    _padding: [u8; 4],
}

#[derive(BorshSerialize)]
struct UploadTransactChunkArgs {
    offset: u32,
    data: Vec<u8>,
}

/// Compute Budget program ID
pub const COMPUTE_BUDGET_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("ComputeBudget111111111111111111111111111111");

/// Create a SetComputeUnitLimit instruction
fn set_compute_unit_limit(units: u32) -> Instruction {
    // Instruction data: discriminator (2) + u32 units
    let mut data = vec![2u8]; // SetComputeUnitLimit discriminator
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_id: COMPUTE_BUDGET_PROGRAM_ID,
        accounts: vec![],
        data,
    }
}

// Re-export constants from shielded_pool
pub use shielded_pool::instructions::types::{N_INS, N_OUTS, N_PUBLIC_LINES, N_REWARD_LINES};

/// Unified SOL asset ID constant (matches src/utils.rs)
pub const UNIFIED_SOL_ASSET_ID: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

/// Compute the transact_params_hash for a proof.
/// This must match the on-chain `calculate_transact_params_hash` function exactly.
///
/// Uses SHA256 of the raw Pod bytes of TransactParams for canonical hashing.
/// The encrypted_output_hashes field in TransactParams must already contain
/// the SHA256 hashes of the encrypted outputs.
pub fn compute_transact_params_hash(params: &TransactParams) -> [u8; 32] {
    // Hash the entire TransactParams struct as raw Pod bytes
    let params_bytes = bytemuck::bytes_of(params);
    let mut hasher = Sha256::new();
    hasher.update(params_bytes);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);

    // The on-chain code compares:
    //   Fr::from_le_bytes_mod_order(&calculated_hash) != Fr::from_be_bytes_mod_order(&proof.transact_params_hash)
    // So we need to reverse the hash bytes when storing in the proof,
    // so that be(reversed_hash) == le(calculated_hash)
    hash.reverse();
    hash
}

/// Compute SHA256 hash of encrypted output data.
/// Returns hashes suitable for the encrypted_output_hashes field in TransactParams.
pub fn compute_encrypted_output_hashes(encrypted_outputs: &[&[u8]; N_OUTS]) -> [[u8; 32]; N_OUTS] {
    let mut hashes = [[0u8; 32]; N_OUTS];
    for i in 0..N_OUTS {
        let mut hasher = Sha256::new();
        hasher.update(encrypted_outputs[i]);
        let result = hasher.finalize();
        hashes[i].copy_from_slice(&result);
    }
    hashes
}

/// Compute the public_amount field element for a proof.
/// For deposits (ext_amount > 0): public_amount = ext_amount - fee
/// For withdrawals (ext_amount < 0): public_amount = -(|ext_amount| + fee + relayer_fee)
/// For transfers (ext_amount == 0): public_amount = -(fee + relayer_fee)
pub fn compute_public_amount(ext_amount: i64, fee: u64, relayer_fee: u64) -> [u8; 32] {
    if ext_amount > 0 {
        // Deposit: public_amount = ext_amount - fee
        let amount = (ext_amount as u64).checked_sub(fee).unwrap();
        amount_to_field_element(amount as i64)
    } else if ext_amount < 0 {
        // Withdrawal: public_amount = -(|ext_amount| + fee + relayer_fee)
        let abs_ext = (-ext_amount) as u64;
        let total = abs_ext.checked_add(fee).unwrap().checked_add(relayer_fee).unwrap();
        amount_to_field_element(-(total as i64))
    } else {
        // Transfer: public_amount = -(fee + relayer_fee)
        let total = fee.checked_add(relayer_fee).unwrap();
        if total == 0 {
            [0u8; 32]
        } else {
            amount_to_field_element(-(total as i64))
        }
    }
}

/// Convert i64 amount to BN254 field element bytes (big-endian).
/// Positive amounts are stored directly.
/// Negative amounts use modular negation: p - |amount|
/// where p is the BN254 scalar field modulus.
///
/// Returns big-endian bytes to match on-chain Fr::from_be_bytes_mod_order.
pub fn amount_to_field_element(amount: i64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    if amount >= 0 {
        // Store as big-endian: most significant bytes first
        // For positive amounts, the value goes in the last 8 bytes (big-endian)
        bytes[24..32].copy_from_slice(&(amount as u64).to_be_bytes());
    } else {
        // BN254 scalar field modulus (r) in little-endian
        // r = 21888242871839275222246405745257275088548364400416034343698204186575808495617
        let r_le: [u8; 32] = [
            0x01, 0x00, 0x00, 0xf0, 0x93, 0xf5, 0xe1, 0x43, 0x91, 0x70, 0xb9, 0x79, 0x48, 0xe8,
            0x33, 0x28, 0x5d, 0x58, 0x81, 0x81, 0xb6, 0x45, 0x50, 0xb8, 0x29, 0xa0, 0x31, 0xe1,
            0x72, 0x4e, 0x64, 0x30,
        ];

        // Compute r - |amount| using big integer arithmetic (little-endian)
        let abs_amount = (-amount) as u64;
        let mut result_le = [0u8; 32];
        let mut borrow: u128 = 0;
        for i in 0..32 {
            let r_byte = r_le[i] as u128;
            let amt_byte = if i < 8 {
                ((abs_amount >> (i * 8)) & 0xff) as u128
            } else {
                0
            };
            let diff = r_byte.wrapping_sub(amt_byte).wrapping_sub(borrow);
            result_le[i] = diff as u8;
            borrow = if diff > r_byte { 1 } else { 0 };
        }
        // Convert to big-endian
        for i in 0..32 {
            bytes[i] = result_le[31 - i];
        }
    }
    bytes
}

/// Build a test Proof struct for execute_transact.
/// In test-mode, proof verification is bypassed, but we still need
/// correct public inputs for validation (transact_params_hash, asset_ids, etc.)
pub fn build_test_proof(
    commitment_root: [u8; 32],
    params: &TransactParams,
    public_asset_ids: [[u8; 32]; N_PUBLIC_LINES],
    public_amounts: [[u8; 32]; N_PUBLIC_LINES],
    nullifiers: [[u8; 32]; N_INS],
    commitments: [[u8; 32]; N_OUTS],
    reward_acc: [[u8; 32]; N_REWARD_LINES],
    reward_asset_id: [[u8; 32]; N_REWARD_LINES],
) -> TransactProofData {
    let transact_params_hash = compute_transact_params_hash(params);

    TransactProofData {
        // Groth16 proof elements (arbitrary in test-mode)
        proof_a: [0u8; 32],
        proof_b: [0u8; 64],
        proof_c: [0u8; 32],
        // Public inputs
        commitment_root,
        transact_params_hash,
        public_asset_ids,
        public_amounts,
        nullifiers,
        commitments,
        reward_acc,
        reward_asset_id,
    }
}

/// Build TransactParams for a single-asset deposit.
/// Uses empty encrypted outputs (SHA256 of empty bytes for each).
pub fn deposit_transact_params(
    asset_id: [u8; 32],
    mint: &Pubkey,
    amount: u64,
    fee: u64,
    relayer: &Pubkey,
) -> TransactParams {
    // Compute hashes for empty encrypted outputs
    let encrypted_output_hashes = compute_encrypted_output_hashes(&[&[][..]; N_OUTS]);
    TransactParams {
        asset_ids: [asset_id, [0u8; 32]],
        recipients: [[0u8; 32], [0u8; 32]],
        ext_amounts: [amount as i64, 0],
        fees: [fee, 0],
        mints: [mint.to_bytes(), [0u8; 32]],
        relayer: relayer.to_bytes(),
        relayer_fees: [0, 0],
        slot_expiry: 0,
        encrypted_output_hashes,
    }
}

/// Build TransactParams for a single-asset withdrawal.
/// Uses empty encrypted outputs (SHA256 of empty bytes for each).
pub fn withdrawal_transact_params(
    asset_id: [u8; 32],
    mint: &Pubkey,
    amount: u64,
    fee: u64,
    relayer_fee: u64,
    recipient: &Pubkey,
    relayer: &Pubkey,
) -> TransactParams {
    // Compute hashes for empty encrypted outputs
    let encrypted_output_hashes = compute_encrypted_output_hashes(&[&[][..]; N_OUTS]);
    TransactParams {
        asset_ids: [asset_id, [0u8; 32]],
        recipients: [recipient.to_bytes(), [0u8; 32]],
        ext_amounts: [-(amount as i64), 0],
        fees: [fee, 0],
        mints: [mint.to_bytes(), [0u8; 32]],
        relayer: relayer.to_bytes(),
        relayer_fees: [relayer_fee, 0],
        slot_expiry: 0,
        encrypted_output_hashes,
    }
}

/// Build empty/default TransactParams (for pure transfer).
/// Uses empty encrypted outputs (SHA256 of empty bytes for each).
pub fn default_transact_params(relayer: &Pubkey) -> TransactParams {
    // Compute hashes for empty encrypted outputs
    let encrypted_output_hashes = compute_encrypted_output_hashes(&[&[][..]; N_OUTS]);
    TransactParams {
        asset_ids: [[0u8; 32], [0u8; 32]],
        recipients: [[0u8; 32], [0u8; 32]],
        ext_amounts: [0, 0],
        fees: [0, 0],
        mints: [[0u8; 32], [0u8; 32]],
        relayer: relayer.to_bytes(),
        relayer_fees: [0, 0],
        slot_expiry: 0,
        encrypted_output_hashes,
    }
}

/// Build TransactParams for a pure transfer (send) with relayer fee.
/// ext_amount = 0 means no deposit/withdrawal, just internal shielded transfer.
/// Uses empty encrypted outputs (SHA256 of empty bytes for each).
pub fn transfer_transact_params(
    asset_id: [u8; 32],
    mint: &Pubkey,
    fee: u64,
    relayer_fee: u64,
    relayer: &Pubkey,
) -> TransactParams {
    // Compute hashes for empty encrypted outputs
    let encrypted_output_hashes = compute_encrypted_output_hashes(&[&[][..]; N_OUTS]);
    TransactParams {
        asset_ids: [asset_id, [0u8; 32]],
        recipients: [[0u8; 32], [0u8; 32]], // No external recipient for transfer
        ext_amounts: [0, 0],                // ext_amount = 0 for transfer
        fees: [fee, 0],
        mints: [mint.to_bytes(), [0u8; 32]],
        relayer: relayer.to_bytes(),
        relayer_fees: [relayer_fee, 0],
        slot_expiry: 0,
        encrypted_output_hashes,
    }
}

/// Per-asset accounts for execute_transact
#[derive(Clone, Copy)]
pub struct TestPerAssetAccounts {
    pub config: Pubkey,
    pub vault: Pubkey,
    pub depositor_token: Pubkey,
    pub recipient_token: Pubkey,
    pub relayer_token: Pubkey,
}

/// Derive transact session PDA
pub fn find_transact_session_pda(
    program_id: &Pubkey,
    authority: &Pubkey,
    nonce: u64,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"transact_session",
            authority.as_ref(),
            &nonce.to_le_bytes(),
        ],
        program_id,
    )
}

/// Initialize a transact session
pub fn init_transact_session(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    authority: &Keypair,
    nonce: u64,
    data_len: u32,
) -> Result<Pubkey, String> {
    let (session_pda, _) = find_transact_session_pda(program_id, &authority.pubkey(), nonce);

    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(session_pda, false),
            AccountMeta::new(authority.pubkey(), true),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::InitTransactSession as u8,
            &InitTransactSessionArgs { nonce, data_len, _padding: [0; 4] },
        ),
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| session_pda)
        .map_err(|e| format!("{:?}", e))
}

/// Upload a chunk to a transact session
pub fn upload_transact_chunk(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    session: &Pubkey,
    authority: &Keypair,
    offset: u32,
    data: Vec<u8>,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*session, false),
            AccountMeta::new_readonly(authority.pubkey(), true),
        ],
        data: build_instruction_data(
            ShieldedPoolInstruction::UploadTransactChunk as u8,
            &UploadTransactChunkArgs { offset, data },
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

/// Close a transact session
pub fn close_transact_session(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    session: &Pubkey,
    authority: &Keypair,
) -> Result<(), String> {
    let ix = Instruction {
        program_id: *program_id,
        accounts: vec![
            AccountMeta::new(*session, false),
            AccountMeta::new(authority.pubkey(), true),
        ],
        data: build_instruction_data_no_args(ShieldedPoolInstruction::CloseTransactSession as u8),
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

/// Execute transact instruction with full account layout.
///
/// Account layout (from src/instructions/transact/accounts.rs):
/// 0: transact_session (W)
/// 1: commitment_tree (W)
/// 2: receipt_tree (W)
/// 3: global_config (R)
/// 4-7: nullifiers[4] (W)
/// 8: depositor (S)
/// 9: relayer (S)
/// 10: token_program (R)
/// 11: system_program (R)
/// 12: payer (W,S)
/// 13: shielded_pool_program (R)
/// 14-21: reward_asset_configs[8] (R) - pool configs for reward registry assets
/// 22-26: asset[0] (config, vault, depositor_token, recipient_token, relayer_token)
/// 27-31: asset[1] (config, vault, depositor_token, recipient_token, relayer_token)
/// 32: unified_sol_config (optional, W)
pub fn execute_transact(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    session: &Pubkey,
    commitment_tree: &Pubkey,
    receipt_tree: &Pubkey,
    global_config: &Pubkey,
    nullifiers: [&Pubkey; N_INS],
    depositor: &Keypair,
    relayer: &Keypair,
    payer: &Keypair,
    reward_asset_configs: [&Pubkey; N_REWARD_LINES],
    asset_0: &TestPerAssetAccounts,
    asset_1: &TestPerAssetAccounts,
    unified_sol_config: Option<&Pubkey>,
) -> Result<(), String> {
    use crate::common::pda::SPL_TOKEN_PROGRAM_ID;

    let mut accounts = vec![
        // Fixed accounts 0-13
        AccountMeta::new(*session, false), // 0: transact_session
        AccountMeta::new(*commitment_tree, false), // 1: commitment_tree
        AccountMeta::new(*receipt_tree, false), // 2: receipt_tree
        AccountMeta::new_readonly(*global_config, false), // 3: global_config
        AccountMeta::new(*nullifiers[0], false), // 4: nullifier[0]
        AccountMeta::new(*nullifiers[1], false), // 5: nullifier[1]
        AccountMeta::new(*nullifiers[2], false), // 6: nullifier[2]
        AccountMeta::new(*nullifiers[3], false), // 7: nullifier[3]
        AccountMeta::new_readonly(depositor.pubkey(), true), // 8: depositor (signer)
        AccountMeta::new_readonly(relayer.pubkey(), true), // 9: relayer (signer)
        AccountMeta::new_readonly(SPL_TOKEN_PROGRAM_ID, false), // 10: token_program
        AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false), // 11: system_program
        AccountMeta::new(payer.pubkey(), true), // 12: payer (signer)
        AccountMeta::new_readonly(*program_id, false), // 13: shielded_pool_program
    ];

    // Reward asset configs 14-21
    for config in reward_asset_configs.iter() {
        accounts.push(AccountMeta::new_readonly(**config, false));
    }

    // Asset 0 accounts 22-26
    accounts.push(AccountMeta::new(asset_0.config, false)); // 22: config[0]
    accounts.push(AccountMeta::new(asset_0.vault, false)); // 23: vault[0]
    accounts.push(AccountMeta::new(asset_0.depositor_token, false)); // 24: depositor_token[0]
    accounts.push(AccountMeta::new(asset_0.recipient_token, false)); // 25: recipient_token[0]
    accounts.push(AccountMeta::new(asset_0.relayer_token, false)); // 26: relayer_token[0]

    // Asset 1 accounts 27-31
    accounts.push(AccountMeta::new(asset_1.config, false)); // 27: config[1]
    accounts.push(AccountMeta::new(asset_1.vault, false)); // 28: vault[1]
    accounts.push(AccountMeta::new(asset_1.depositor_token, false)); // 29: depositor_token[1]
    accounts.push(AccountMeta::new(asset_1.recipient_token, false)); // 30: recipient_token[1]
    accounts.push(AccountMeta::new(asset_1.relayer_token, false)); // 31: relayer_token[1]

    // Optional unified_sol_config 32
    if let Some(config) = unified_sol_config {
        accounts.push(AccountMeta::new(*config, false));
    }

    let ix = Instruction {
        program_id: *program_id,
        accounts,
        data: build_instruction_data_no_args(ShieldedPoolInstruction::ExecuteTransact as u8),
    };

    // Depositor, relayer, and payer all need to sign
    // Dedupe in case any share the same key
    let mut signers: Vec<&Keypair> = vec![depositor];
    if relayer.pubkey() != depositor.pubkey() {
        signers.push(relayer);
    }
    if payer.pubkey() != depositor.pubkey() && payer.pubkey() != relayer.pubkey() {
        signers.push(payer);
    }

    // Add compute budget instruction to increase CU limit (execute_transact uses ~250k CUs)
    let compute_budget_ix = set_compute_unit_limit(400_000);

    let tx = Transaction::new_signed_with_payer(
        &[compute_budget_ix, ix],
        Some(&payer.pubkey()),
        &signers,
        svm.latest_blockhash(),
    );

    svm.send_transaction(tx)
        .map(|_| ())
        .map_err(|e| format!("{:?}", e))
}

/// Upload proof, params, nullifier NM proof, and encrypted outputs to a transact session.
/// This is a convenience function that uploads the complete session data.
///
/// Session data layout:
/// - Proof (Pod bytes, raw)
/// - TransactParams (Pod bytes, raw)
/// - NullifierNMProofData (Pod bytes, zeros for test mode)
/// - encrypted_outputs (Borsh format: u32 length prefix + data for each output)
///
/// Note: Data is uploaded in chunks to fit within transaction size limits.
pub fn upload_proof_and_params(
    svm: &mut LiteSVM,
    program_id: &Pubkey,
    session: &Pubkey,
    authority: &Keypair,
    proof: &TransactProofData,
    params: &TransactParams,
) -> Result<(), String> {
    // Serialize proof as raw Pod bytes (matches bytemuck::from_bytes in execute_transact)
    let proof_bytes = bytemuck::bytes_of(proof);

    // Serialize params as raw Pod bytes (matches bytemuck::from_bytes in execute_transact)
    let params_bytes = bytemuck::bytes_of(params);

    // Create dummy NullifierNonMembershipProofData (160 bytes of zeros for test mode)
    let nm_proof_bytes = vec![0u8; 160];

    // Create empty encrypted outputs (N_OUTS empty Vec<u8>, each with u32 length prefix of 0)
    let mut encrypted_outputs_bytes = Vec::new();
    for _ in 0..N_OUTS {
        encrypted_outputs_bytes.extend_from_slice(&0u32.to_le_bytes()); // length = 0
    }

    // Combine all data
    let mut all_data = Vec::new();
    all_data.extend(proof_bytes);
    all_data.extend(params_bytes);
    all_data.extend(&nm_proof_bytes);
    all_data.extend(&encrypted_outputs_bytes);

    // Upload in chunks to fit within transaction size limits (~900 bytes per chunk to be safe)
    const CHUNK_SIZE: usize = 900;
    let mut offset = 0u32;
    for chunk in all_data.chunks(CHUNK_SIZE) {
        upload_transact_chunk(svm, program_id, session, authority, offset, chunk.to_vec())?;
        offset += chunk.len() as u32;
    }

    Ok(())
}
