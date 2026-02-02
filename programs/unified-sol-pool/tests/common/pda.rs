//! PDA derivation helpers for unified-sol-pool tests.

use solana_sdk::pubkey::Pubkey;

use super::setup::UNIFIED_SOL_POOL_PROGRAM_ID;

// ============================================================================
// Unified SOL Pool PDAs
// ============================================================================

/// Unified SOL pool PDA seeds
pub const UNIFIED_SOL_POOL_CONFIG_SEED: &[u8] = b"unified_sol_pool";
pub const LST_CONFIG_SEED: &[u8] = b"lst_config";
pub const LST_VAULT_SEED: &[u8] = b"lst_vault";

/// Derive UnifiedSolPoolConfig PDA
pub fn find_unified_sol_pool_config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[UNIFIED_SOL_POOL_CONFIG_SEED], program_id)
}

/// Derive LstConfig PDA
pub fn find_lst_config_pda(program_id: &Pubkey, lst_mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[LST_CONFIG_SEED, lst_mint.as_ref()], program_id)
}

/// Derive LST vault PDA (uses program's module-level function signature)
pub fn find_lst_vault_pda(program_id: &Pubkey, lst_config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[LST_VAULT_SEED, lst_config.as_ref()], program_id)
}

/// Convenience function to derive LST vault PDA using default program ID
pub fn find_lst_vault_pda_default(lst_config: &Pubkey) -> (Pubkey, u8) {
    find_lst_vault_pda(&UNIFIED_SOL_POOL_PROGRAM_ID, lst_config)
}

// ============================================================================
// Common Constants
// ============================================================================

/// System program ID
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_sdk::system_program::ID;

/// SPL Token program ID
pub const SPL_TOKEN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// SPL Stake Pool Program ID
pub const SPL_STAKE_POOL_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy");
