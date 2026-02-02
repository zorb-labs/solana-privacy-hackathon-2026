//! Constants for validation tests

use solana_sdk::pubkey::Pubkey;

/// Program ID for the validation-test program
pub const PROGRAM_ID: Pubkey = solana_sdk::pubkey!("E6VXXXxTUkibL82Ed41yCkYbXCNPgyiMLspnvwA67aBg");

/// System program ID
pub const SYSTEM_PROGRAM_ID: Pubkey = solana_sdk::pubkey!("11111111111111111111111111111111");

/// Token program ID
pub const TOKEN_PROGRAM_ID: Pubkey =
    solana_sdk::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// One SOL in lamports
pub const SOL: u64 = 1_000_000_000;
