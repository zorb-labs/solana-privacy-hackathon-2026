//! Pool program ID constants.
//!
//! These program IDs are imported from the central `zorb-program-ids` crate,
//! which is the single source of truth for all program addresses.
//!
//! The correct addresses are selected at compile-time based on the network feature.

use pinocchio::pubkey::Pubkey;

use crate::PoolType;

// =============================================================================
// Program IDs (imported from zorb-program-ids crate)
// =============================================================================

/// Token Pool program ID.
///
/// Handles SPL token deposits and withdrawals with 1:1 exchange rate.
pub const TOKEN_POOL_PROGRAM_ID: Pubkey =
    five8_const::decode_32_const(zorb_program_ids::TOKEN_POOL_ID);

/// Unified SOL Pool program ID.
///
/// Handles LST deposits and withdrawals with exchange rate conversion.
pub const UNIFIED_SOL_POOL_PROGRAM_ID: Pubkey =
    five8_const::decode_32_const(zorb_program_ids::UNIFIED_SOL_POOL_ID);

/// Hub program ID (shielded-pool).
///
/// Used by pools to validate CPI callers.
pub const HUB_PROGRAM_ID: Pubkey =
    five8_const::decode_32_const(zorb_program_ids::SHIELDED_POOL_ID);

// =============================================================================
// Helper Functions
// =============================================================================

/// Get the program ID for a pool type.
pub const fn get_pool_program_id(pool_type: PoolType) -> Pubkey {
    match pool_type {
        PoolType::Token => TOKEN_POOL_PROGRAM_ID,
        PoolType::UnifiedSol => UNIFIED_SOL_POOL_PROGRAM_ID,
    }
}

/// Check if a program ID is a valid pool program.
pub fn is_valid_pool_program(program_id: &Pubkey) -> bool {
    *program_id == TOKEN_POOL_PROGRAM_ID || *program_id == UNIFIED_SOL_POOL_PROGRAM_ID
}

/// Check if a program ID matches the expected pool type.
pub fn validate_pool_program(program_id: &Pubkey, expected_type: PoolType) -> bool {
    *program_id == get_pool_program_id(expected_type)
}

/// Hub authority PDA seed.
pub const HUB_AUTHORITY_SEED: &[u8] = b"hub_authority";

/// Derive the hub authority PDA from the hub program.
///
/// This is the canonical PDA that pools should validate for withdrawal delegation.
/// Returns (address, bump).
pub fn find_hub_authority_pda() -> (Pubkey, u8) {
    pinocchio::pubkey::find_program_address(&[HUB_AUTHORITY_SEED], &HUB_PROGRAM_ID)
}

/// Validate that an account is the canonical hub authority PDA.
pub fn validate_hub_authority(hub_authority: &Pubkey) -> bool {
    let (expected, _) = find_hub_authority_pda();
    *hub_authority == expected
}
