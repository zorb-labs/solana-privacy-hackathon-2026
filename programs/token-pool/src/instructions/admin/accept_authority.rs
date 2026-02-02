//! `AcceptAuthority` instruction handler.
//!
//! Completes the two-step authority transfer by accepting the pending authority role.
//! Must be called by the `pending_authority` address.

use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;
use zorb_pool_interface::authority::accept_authority_impl;

use crate::TokenPoolConfig;

/// Accounts for the `AcceptAuthority` instruction.
#[derive(Accounts)]
pub struct AcceptAuthorityAccounts<'info> {
    /// Pool config PDA ["token_pool", mint]
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,
    /// Pending authority (must be signer, must match pool_config.pending_authority)
    pub signer: Signer<'info>,
}

/// Process accept authority instruction.
///
/// Completes the two-step authority transfer. The signer must match the
/// `pending_authority` field on the pool config.
pub fn process_accept_authority(ctx: Context<AcceptAuthorityAccounts>) -> ProgramResult {
    let AcceptAuthorityAccounts {
        pool_config,
        signer,
    } = ctx.accounts;

    pool_config.try_inspect_mut(|config| {
        accept_authority_impl(config, signer.key())?;
        log!("accept_authority: authority transferred");
        Ok(())
    })
}
