//! Common Solana program constants
//!
//! Well-known program IDs and addresses used across Solana programs.

use pinocchio::pubkey::Pubkey;

/// System Program ID
pub const SYSTEM_PROGRAM_ID: Pubkey = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");

/// SPL Token Program ID
pub const TOKEN_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// Associated Token Program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

/// Metaplex Token Metadata Program ID
pub const TOKEN_METADATA_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::pubkey!("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");

/// Wrapped SOL mint address
pub const WSOL_MINT: Pubkey =
    pinocchio_pubkey::pubkey!("So11111111111111111111111111111111111111112");
