//! Close a transact session and reclaim rent.

use crate::{
    errors::ShieldedPoolError,
    state::{SESSION_EXPIRY_SLOTS, TransactSession},
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_log::log;

/// Accounts for CloseTransactSession instruction.
#[derive(Accounts)]
pub struct CloseTransactSessionAccounts<'info> {
    /// Transact session PDA to close ["transact_session", authority, nonce]
    #[account(mut)]
    pub transact_session: AccountLoader<'info, TransactSession>,

    /// Authority (must match session creator) or anyone after expiry
    #[account(mut)]
    pub authority: Signer<'info>,
}

/// Close a transact session account and reclaim rent.
///
/// This instruction closes a session account and returns the lamports to the closer.
/// Can be called by:
/// - The session authority at any time
/// - Anyone after SESSION_EXPIRY_SLOTS (~24 hours) have passed since creation
pub fn process_close_transact_session(ctx: Context<CloseTransactSessionAccounts>) -> ProgramResult {
    let CloseTransactSessionAccounts {
        transact_session: transact_session_account,
        authority: closer,
    } = ctx.accounts;

    let program_id = &crate::ID;

    // Load header to validate and check authorization, extract values, then drop borrow
    let (session_authority, session_created_slot) = {
        let session = TransactSession::load_header(transact_session_account, program_id)?;
        (session.authority, session.created_slot)
    };

    // Check authorization: either authority OR session has expired
    let is_authority = session_authority == *closer.key();

    let clock = Clock::get()?;
    let slots_elapsed = clock
        .slot
        .checked_sub(session_created_slot)
        .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
    let is_expired = slots_elapsed >= SESSION_EXPIRY_SLOTS;

    if !is_authority && !is_expired {
        log!("close_transact_session: unauthorized - not authority and session not expired");
        return Err(ShieldedPoolError::Unauthorized.into());
    }

    // Transfer all lamports to closer
    let lamports = transact_session_account.lamports();

    // Use unsafe to modify lamports directly (standard pattern for closing accounts)
    unsafe {
        *transact_session_account.borrow_mut_lamports_unchecked() = 0;
        *closer.borrow_mut_lamports_unchecked() = closer
            .lamports()
            .checked_add(lamports)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
    }

    // Zero out the account data to mark it as closed
    let mut data = transact_session_account.try_borrow_mut_data()?;
    data.fill(0);

    log!("close_transact_session: session closed successfully");

    Ok(())
}
