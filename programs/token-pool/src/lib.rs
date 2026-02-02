//! Token Pool Program
//!
//! Handles SPL token deposits and withdrawals for the Zorb privacy system.
//! This program is invoked by the shielded-pool hub via CPI.
//!
//! # Architecture
//!
//! The token-pool is a "plugin" program that:
//! - Receives pre-computed amounts from the hub (no fee calculation)
//! - Executes token transfers (deposit: user→vault, withdraw: vault→recipient)
//! - Updates pool state (pending deposits/withdrawals, fees, etc.)
//!
//! # Instructions
//!
//! - `Deposit`: Transfer tokens from depositor to vault
//! - `Withdraw`: Transfer tokens from vault to recipient

#![cfg_attr(not(any(test, feature = "idl-build")), no_std)]

extern crate alloc;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod pda;
pub mod state;

// Error and event types
pub use errors::TokenPoolError;
pub use events::{
    EventType, SweepExcessEvent, TokenDepositEvent, TokenRewardsFinalizedEvent,
    TokenWithdrawalEvent, emit_event,
};

// Instruction enum for panchor dispatch
pub use instructions::TokenPoolInstruction;

// PDA derivation helpers
pub use pda::*;

// State types
// Note: PDA seeds (VAULT_SEED, etc.) come from pda::* above
pub use state::TokenPoolConfig;

// Use panchor's program! macro for instruction dispatch
// This generates: ID, check_id, id, process_instruction, default_allocator
//
// Program ID is imported from zorb-program-ids crate (single source of truth).
// The correct ID is selected at compile-time based on feature flags.

panchor::program! {
    id = zorb_program_ids::TOKEN_POOL_ID,
    instructions = TokenPoolInstruction,
    accounts = state::TokenPoolAccount,
    pdas = pda::TokenPoolPdas,
}
