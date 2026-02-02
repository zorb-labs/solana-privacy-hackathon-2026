//! Pinocchio extension traits and utilities
//!
//! This crate provides ergonomic extension traits for working with Pinocchio
//! in Solana programs.
//!
//! # Usage
//!
//! Import the prelude to get access to all extension traits:
//!
//! ```ignore
//! use panchor::prelude::*;
//!
//! // Account assertions
//! signer.assert_signer()?;
//! mine_info.assert_owner(program_id)?.assert_key(&expected_key)?;
//!
//! // Loading account data
//! let data: &MyAccount = account.load()?;
//!
//! // Transfers
//! from_account.transfer(to_account, amount)?;
//! ```

#![cfg_attr(target_os = "solana", no_std)]

extern crate alloc;

mod account_loaders;
pub mod accounts;
mod context;
mod create_pda;
mod discriminator;
pub mod events;
mod find_pda;
mod idl_type;
mod inner_size;
mod instruction_data;
#[cfg(feature = "idl-build")]
mod instruction_idl;
mod instruction_processor;
mod processor;
mod program_owned;
pub mod programs;
mod space;
mod spl_token;

pub mod prelude;

// Re-export from pinocchio-contrib
pub use pinocchio_contrib::constants;
pub use pinocchio_contrib::{
    AccountAssertions, AccountAssertionsNoTrace, AccountOperations, log_account_validation_error,
    log_caller_location, trace,
};

pub use account_loaders::AccountLoaders;
pub use accounts::{
    AccountDataValidate, AccountDeserialize, AccountLoader, AsAccountInfo, Bumps, Id, LazyAccount,
    PdaAccount, PdaAccountWithBump, Program, SetBump, Signer,
};
pub use context::{Context, ParseResult, Parsed};
pub use create_pda::CreatePda;
pub use discriminator::{Discriminator, SetDiscriminator};
pub use events::{Event, EventBytes, EventLog};
pub use find_pda::{FindProgramAddress, SignerSeeds};
pub use idl_type::IdlType;
pub use inner_size::InnerSize;
pub use instruction_data::parse_instruction_data;
#[cfg(feature = "idl-build")]
pub use instruction_idl::InstructionIdl;
pub use instruction_processor::InstructionDispatch;
pub use processor::process_instruction;
pub use program_owned::ProgramOwned;
pub use programs::{AssociatedToken, System, Token, TokenMetadata};
pub use space::{DISCRIMINATOR_SIZE, InitSpace};
pub use spl_token::TokenAccountExt;

// Re-export derive macros
pub use panchor_derive::{
    Accounts, EventLog, FindProgramAddress, IdlType, InstructionArgs, InstructionDispatch, account,
    constant, error_code, event, instruction, instructions, pdas, program, zero_copy,
};

// Re-export paste for macro usage
#[doc(hidden)]
pub use paste;

// Re-export core dependencies used by macros.
// These allow downstream crates to use `::panchor::pinocchio` etc. without
// adding explicit dependencies.
#[doc(hidden)]
pub use bytemuck;
#[doc(hidden)]
pub use five8_const;
#[doc(hidden)]
pub use num_enum;
#[doc(hidden)]
pub use pinocchio;
#[doc(hidden)]
pub use pinocchio_log;
#[doc(hidden)]
pub use pinocchio_pubkey;
#[doc(hidden)]
pub use strum;

// Re-export panchor_idl for IDL building (only when idl-build feature is enabled)
#[cfg(feature = "idl-build")]
#[doc(hidden)]
pub use panchor_idl;
