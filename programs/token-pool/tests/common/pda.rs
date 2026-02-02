//! PDA derivation helpers for token-pool tests.

use solana_sdk::pubkey::Pubkey;

// ============================================================================
// Token Pool PDAs
// ============================================================================

/// Token pool PDA seeds
pub const TOKEN_POOL_CONFIG_SEED: &[u8] = b"token_pool";
pub const VAULT_SEED: &[u8] = b"vault";

/// Derive TokenPoolConfig PDA
pub fn find_token_pool_config_pda(program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TOKEN_POOL_CONFIG_SEED, mint.as_ref()], program_id)
}

/// Derive Vault PDA for token config
pub fn find_vault_pda(program_id: &Pubkey, token_config: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[VAULT_SEED, token_config.as_ref()], program_id)
}

// ============================================================================
// Common Constants
// ============================================================================

/// System program ID
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_sdk::system_program::ID;

/// SPL Token program ID
pub const SPL_TOKEN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
