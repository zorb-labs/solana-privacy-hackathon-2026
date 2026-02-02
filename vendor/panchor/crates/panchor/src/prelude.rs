//! Prelude module for convenient imports
//!
//! Import everything from this module to get access to all extension traits:
//!
//! ```ignore
//! use panchor::prelude::*;
//! ```

// pinocchio-contrib
pub use pinocchio_contrib::{bail_err, constants::WSOL_MINT, prelude::*, require};

// bytemuck
pub use bytemuck::{Pod, Zeroable};

// panchor-derive
pub use panchor_derive::{program, *};

// panchor (this crate)
pub use crate::{
    AccountOperations,
    account_loaders::AccountLoaders,
    accounts::{
        AccountDataValidate, AccountDeserialize, AccountLoader, AsAccountInfo, Bumps, Id,
        LazyAccount, PdaAccount, PdaAccountWithBump, Program, SetBump, Signer,
    },
    context::{Context, Parsed},
    create_pda::CreatePda,
    discriminator::Discriminator,
    events::{Event, EventBytes, EventLog},
    find_pda::{FindProgramAddress, SignerSeeds},
    idl_type,
    inner_size::InnerSize,
    instruction_processor::InstructionDispatch,
    processor::process_instruction,
    program_owned::ProgramOwned,
    programs::{AssociatedToken, System, Token, TokenMetadata},
    space::{DISCRIMINATOR_SIZE, InitSpace},
    spl_token::TokenAccountExt,
};
