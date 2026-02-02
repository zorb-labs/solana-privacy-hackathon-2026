#![cfg_attr(not(any(test, feature = "idl-build")), no_std)]

extern crate alloc;

pub mod account_loaders;
pub mod errors;
pub mod events;
pub mod groth16;
pub mod indexed_merkle_tree;
pub mod instructions;
pub mod merkle_tree;
pub mod pda;
pub mod pool_cpi;
pub mod state;
pub mod token;
pub mod utils;
pub mod validation;
pub mod vault_cpi;
pub mod verifying_keys;

pub use instructions::ShieldedPoolInstruction;
pub use state::*;

// Use panchor's program! macro for instruction dispatch
// This generates: ID, check_id, id, process_instruction, default_allocator
//
// Program ID is imported from zorb-program-ids crate (single source of truth).
// The correct ID is selected at compile-time based on feature flags.

panchor::program! {
    id = zorb_program_ids::SHIELDED_POOL_ID,
    instructions = ShieldedPoolInstruction,
    accounts = state::ShieldedPoolAccount,
    events = events::EventType,
    pdas = pda::ShieldedPoolPdas,
}
