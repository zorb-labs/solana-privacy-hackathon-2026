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
//!
//! For unit variants (like UnifiedSolPoolConfig), it also generates:
//! - `X_ADDRESS` - Compile-time derived PDA address
//! - `X_BUMP` - Compile-time derived bump

use panchor::pdas;
use pinocchio::pubkey::Pubkey;

/// PDA variants for the Unified SOL Pool program
#[pdas]
pub enum UnifiedSolPoolPdas {
    /// Unified SOL pool config singleton
    /// Seeds: ["unified_sol_pool"]
    #[seeds("unified_sol_pool")]
    UnifiedSolPoolConfig,

    /// LST config PDA - per LST mint
    /// Seeds: ["lst_config", lst_mint]
    #[seeds("lst_config")]
    LstConfig {
        /// The LST mint address
        lst_mint: Pubkey,
    },

    /// LST vault PDA - per LST config
    /// Seeds: ["lst_vault", lst_config]
    #[seeds("lst_vault")]
    LstVault {
        /// The LST config PDA
        lst_config: Pubkey,
    },
}
