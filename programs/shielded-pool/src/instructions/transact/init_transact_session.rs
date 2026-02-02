//! Initialize a transact session for chunked proof upload.

use crate::{
    errors::ShieldedPoolError,
    pda::{find_transact_session_pda, gen_transact_session_seeds},
    state::{TransactSession, MAX_SESSION_DATA_LEN},
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    sysvars::{Sysvar, clock::Clock, rent::Rent},
};
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;

/// Instruction data for InitTransactSession.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct InitTransactSessionData {
    /// Unique nonce for this session
    pub nonce: u64,
    /// Total size of transaction data to be uploaded
    pub data_len: u32,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 4],
}

/// Accounts for InitTransactSession instruction.
#[derive(Accounts)]
pub struct InitTransactSessionAccounts<'info> {
    /// Transact session PDA to create ["transact_session", authority, nonce]
    /// Raw AccountInfo since we're creating this account via CPI
    #[account(mut)]
    pub transact_session: &'info AccountInfo,

    /// Authority (payer) for the session
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Create a transact session account for uploading transaction data in chunks.
///
/// This instruction creates a PDA account that can store transaction data uploaded
/// across multiple transactions. Once complete, the execute_transact instruction
/// reads from this account.
pub fn process_init_transact_session(
    ctx: Context<InitTransactSessionAccounts>,
    data: InitTransactSessionData,
) -> ProgramResult {
    let InitTransactSessionAccounts {
        transact_session,
        authority,
        system_program: _,
    } = ctx.accounts;

    let program_id = &crate::ID;
    let nonce = data.nonce;
    let data_len = data.data_len;

    // M-1 audit fix: Validate data_len bounds early to fail fast
    // This check is also performed in execute_transact, but checking here
    // saves compute and lamports by rejecting invalid sessions at creation time
    if data_len > MAX_SESSION_DATA_LEN {
        log!("init_transact_session: data_len exceeds MAX_SESSION_DATA_LEN");
        return Err(ShieldedPoolError::ProofPayloadOverflow.into());
    }

    // Derive and verify PDA
    let (expected_pda, bump) = find_transact_session_pda(authority.key(), nonce);
    if transact_session.key() != &expected_pda {
        log!("init_transact_session: invalid PDA");
        return Err(pinocchio::program_error::ProgramError::InvalidSeeds);
    }

    // Create the account
    let space = TransactSession::account_size(data_len);
    let rent = Rent::get()?;

    let nonce_bytes = nonce.to_le_bytes();
    let bump_slice = [bump];
    let seeds = gen_transact_session_seeds(authority.key(), &nonce_bytes, &bump_slice);
    let signer = PinocchioSigner::from(&seeds);

    CreateAccount {
        from: authority,
        to: transact_session,
        lamports: rent.minimum_balance(space),
        space: space as u64,
        owner: program_id,
    }
    .invoke_signed(&[signer])?;

    // Get current slot for expiry tracking
    let clock = Clock::get()?;

    // Initialize the account header using the helper method
    TransactSession::init_account(
        transact_session,
        authority.key(),
        nonce,
        data_len,
        bump,
        clock.slot,
    )?;

    log!("init_transact_session: session created successfully");

    Ok(())
}
