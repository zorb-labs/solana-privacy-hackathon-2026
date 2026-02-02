//! PDA derivation helpers.

use solana_pubkey::Pubkey;

/// Shielded pool program ID (from centralized zorb-program-ids crate)
pub const SHIELDED_POOL_PROGRAM_ID: Pubkey = Pubkey::new_from_array(
    five8_const::decode_32_const(zorb_program_ids::SHIELDED_POOL_ID),
);

/// Token pool program ID (from centralized zorb-program-ids crate)
pub const TOKEN_POOL_PROGRAM_ID: Pubkey = Pubkey::new_from_array(
    five8_const::decode_32_const(zorb_program_ids::TOKEN_POOL_ID),
);

/// Unified SOL pool program ID (from centralized zorb-program-ids crate)
pub const UNIFIED_SOL_POOL_PROGRAM_ID: Pubkey = Pubkey::new_from_array(
    five8_const::decode_32_const(zorb_program_ids::UNIFIED_SOL_POOL_ID),
);

/// PDA seeds (must match src/instructions/initialize.rs)
pub const COMMITMENT_TREE_SEED: &[u8] = b"commitment_tree";
pub const GLOBAL_CONFIG_SEED: &[u8] = b"global_config";
pub const RECEIPT_TREE_SEED: &[u8] = b"receipt_tree";

/// Unified SOL PDA seeds
pub const UNIFIED_SOL_POOL_CONFIG_SEED: &[u8] = b"unified_sol_pool";
pub const LST_CONFIG_SEED: &[u8] = b"lst_config";
pub const LST_VAULT_SEED: &[u8] = b"lst_vault";

/// Token config PDA seeds
pub const TOKEN_CONFIG_SEED: &[u8] = b"token_config";
pub const VAULT_SEED: &[u8] = b"vault";

/// System program ID (11111111111111111111111111111111)
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_system_interface::program::ID;

/// SPL Token program ID
pub const SPL_TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
]);

/// Derive PDA addresses for the shielded pool accounts
pub fn derive_pdas(program_id: &Pubkey) -> (Pubkey, Pubkey, Pubkey, Pubkey) {
    let (tree_pda, _) = Pubkey::find_program_address(&[COMMITMENT_TREE_SEED], program_id);
    let (config_pda, _) = Pubkey::find_program_address(&[GLOBAL_CONFIG_SEED], program_id);
    let (receipt_pda, _) = Pubkey::find_program_address(&[RECEIPT_TREE_SEED], program_id);
    let (nullifier_tree_pda, _) = Pubkey::find_program_address(&[NULLIFIER_TREE_SEED], program_id);
    (tree_pda, config_pda, receipt_pda, nullifier_tree_pda)
}

/// Derive UnifiedSolPoolConfig PDA
pub fn find_unified_sol_pool_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[UNIFIED_SOL_POOL_CONFIG_SEED], program_id)
}

/// Derive LstConfig PDA
pub fn find_lst_config_pda(program_id: &Pubkey, lst_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[LST_CONFIG_SEED, lst_mint.as_ref()], program_id)
}

/// Derive LST vault PDA
pub fn find_lst_vault_pda(program_id: &Pubkey, lst_config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[LST_VAULT_SEED, lst_config.as_ref()], program_id)
}

/// Derive TokenConfig PDA
pub fn find_token_config_pda(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TOKEN_CONFIG_SEED, mint.as_ref()], program_id)
}

/// Derive Vault PDA for token config
pub fn find_vault_pda(program_id: &Pubkey, token_config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED, token_config.as_ref()], program_id)
}

/// Nullifier PDA seed
pub const NULLIFIER_SEED: &[u8] = b"nullifier";

/// Derive Nullifier PDA
pub fn find_nullifier_pda(program_id: &Pubkey, nullifier_hash: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NULLIFIER_SEED, nullifier_hash.as_ref()], program_id)
}

/// Nullifier tree PDA seed
pub const NULLIFIER_TREE_SEED: &[u8] = b"nullifier_tree";

/// Derive nullifier tree PDA
pub fn find_nullifier_tree_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NULLIFIER_TREE_SEED], program_id)
}

/// Nullifier epoch root PDA seed
pub const NULLIFIER_EPOCH_ROOT_SEED: &[u8] = b"epoch_root";

/// Derive nullifier epoch root PDA for a specific nullifier epoch
pub fn find_nullifier_epoch_root_pda(program_id: &Pubkey, nullifier_epoch: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[NULLIFIER_EPOCH_ROOT_SEED, &nullifier_epoch.to_le_bytes()], program_id)
}

/// Pool config PDA seed (for hub routing)
pub const POOL_CONFIG_SEED: &[u8] = b"pool_config";

/// Derive PoolConfig PDA for a given asset_id
pub fn find_pool_config_pda(program_id: &Pubkey, asset_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[POOL_CONFIG_SEED, asset_id.as_ref()], program_id)
}

/// Deposit escrow PDA seed
pub const DEPOSIT_ESCROW_SEED: &[u8] = b"deposit_escrow";

/// Escrow vault authority PDA seed
pub const ESCROW_VAULT_AUTHORITY_SEED: &[u8] = b"escrow_vault_authority";

/// Derive DepositEscrow PDA for a depositor and nonce
pub fn find_deposit_escrow_pda(program_id: &Pubkey, depositor: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[DEPOSIT_ESCROW_SEED, depositor.as_ref(), &nonce.to_le_bytes()],
        program_id,
    )
}

/// Derive EscrowVaultAuthority PDA for an escrow account
pub fn find_escrow_vault_authority_pda(program_id: &Pubkey, escrow: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[ESCROW_VAULT_AUTHORITY_SEED, escrow.as_ref()],
        program_id,
    )
}

/// Token pool PDA seeds
pub const TOKEN_POOL_CONFIG_SEED: &[u8] = b"token_pool";

/// Derive TokenPoolConfig PDA
pub fn find_token_pool_config_pda(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TOKEN_POOL_CONFIG_SEED, mint.as_ref()], program_id)
}

/// SPL Stake Pool Program ID
pub const SPL_STAKE_POOL_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy");
