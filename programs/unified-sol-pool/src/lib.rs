//! Unified SOL Pool Program
//!
//! Handles LST (Liquid Staking Token) deposits and withdrawals for the Zorb privacy system.
//! This program is invoked by the shielded-pool hub via CPI.
//!
//! # Architecture
//!
//! The unified-sol-pool is a "plugin" program that:
//! - Manages multiple LST types (WSOL, vSOL, jitoSOL, mSOL, etc.)
//! - Applies exchange rate conversion (LST tokens ↔ virtual SOL)
//! - Enables fungibility: deposit one LST, withdraw another
//! - Tracks virtual SOL balances for each LST vault
//!
//! # Exchange Rate Model
//!
//! - Each LST has a harvested_exchange_rate frozen at epoch boundaries
//! - Deposits: `virtual_sol = lst_tokens * exchange_rate / 1e9`
//! - Withdrawals: `lst_tokens = virtual_sol * 1e9 / exchange_rate`
//! - Fees are in virtual SOL units (not token units)
//!
//! # Instructions
//!
//! - `Deposit`: Transfer LST tokens from depositor to vault, credit virtual SOL
//! - `Withdraw`: Transfer LST tokens from vault to recipient, debit virtual SOL

#![cfg_attr(not(any(test, feature = "idl-build")), no_std)]

extern crate alloc;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod pda;
pub mod state;
pub mod utils;

// Error and event types
pub use errors::UnifiedSolPoolError;
pub use events::{
    AppreciationHarvestedEvent, EventType, ExchangeRateUpdatedEvent, UnifiedSolDepositEvent,
    UnifiedSolRewardsFinalizedEvent, UnifiedSolWithdrawalEvent, emit_event,
};

// Instruction enum for panchor dispatch
pub use instructions::UnifiedSolPoolInstruction;

// PDA derivation helpers
pub use pda::*;

// State types and constants
// Note: PDA seeds (LST_CONFIG_SEED, etc.) come from pda::* above
pub use state::{LstConfig, PoolType, UNIFIED_SOL_ASSET_ID, UnifiedSolPoolConfig};

// Utility functions
pub use utils::read_token_account_balance;

// Use panchor's program! macro for instruction dispatch
// This generates: ID, check_id, id, process_instruction, default_allocator
//
// Program ID is imported from zorb-program-ids crate (single source of truth).
// The correct ID is selected at compile-time based on feature flags.

panchor::program! {
    id = zorb_program_ids::UNIFIED_SOL_POOL_ID,
    instructions = UnifiedSolPoolInstruction,
    accounts = state::UnifiedSolPoolAccount,
    pdas = pda::UnifiedSolPoolPdas,
}
