//! Program Derived Address (PDA) helpers
//!
//! All PDAs are derived using standardized seeds for each account type.
//!
//! # Generated Functions
//!
//! The `#[pdas]` macro generates the following for each PDA variant:
//! - `X_SEED` - The seed constant as a byte string literal
//! - `find_x_pda(...)` - Derives the PDA address and bump
//! - `gen_x_seeds(...)` - Creates signer seeds for CPIs

use panchor::pdas;
use pinocchio::pubkey::Pubkey;

/// PDA variants for the Token Pool program
#[pdas]
pub enum TokenPoolPdas {
    /// Token pool config PDA - per mint
    /// Seeds: ["token_pool", mint]
    #[seeds("token_pool")]
    TokenPoolConfig {
        /// The SPL token mint address
        mint: Pubkey,
    },

    /// Vault token account PDA - per pool config
    /// Seeds: ["vault", pool_config]
    #[seeds("vault")]
    Vault {
        /// The pool config PDA
        pool_config: Pubkey,
    },
}
