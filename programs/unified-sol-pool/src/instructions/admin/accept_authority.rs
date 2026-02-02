//! `AcceptAuthority` instruction handler.
//!
//! Completes the two-step authority transfer by accepting the pending authority role.
//! Must be called by the `pending_authority` address.

use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;
use zorb_pool_interface::authority::accept_authority_impl;

use crate::UnifiedSolPoolConfig;

/// Accounts for the `AcceptAuthority` instruction.
#[derive(Accounts)]
pub struct AcceptAuthorityAccounts<'info> {
    /// Unified SOL pool config PDA ["unified_sol_pool"]
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,
    /// Pending authority (must be signer, must match unified_sol_pool_config.pending_authority)
    pub signer: Signer<'info>,
}

/// Process accept authority instruction.
///
/// Completes the two-step authority transfer. The signer must match the
/// `pending_authority` field on the unified sol pool config.
pub fn process_accept_authority(ctx: Context<AcceptAuthorityAccounts>) -> ProgramResult {
    let AcceptAuthorityAccounts {
        unified_sol_pool_config,
        signer,
    } = ctx.accounts;

    unified_sol_pool_config.try_inspect_mut(|config| {
        accept_authority_impl(config, signer.key())?;
        log!("accept_authority: authority transferred");
        Ok(())
    })
}
