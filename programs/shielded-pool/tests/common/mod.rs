//! Shared test helpers organized by domain.
//!
//! This module provides common utilities for all shielded-pool tests:
//! - `setup`: Program deployment and pool initialization
//! - `pda`: PDA derivation helpers
//! - `mock_accounts`: Mock SPL token/mint/stake pool creation
//! - `instructions`: Instruction helpers organized by domain

pub mod instructions;
pub mod mock_accounts;
pub mod pda;
pub mod setup;

// Re-export commonly used items
pub use instructions::*;
pub use mock_accounts::*;
pub use pda::*;
pub use setup::*;
