//! `TransferAuthority` instruction handler.
//!
//! Initiates a two-step authority transfer by setting `pending_authority`.
//! The new authority must call `accept_authority` to complete the transfer.

use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo};
use pinocchio_log::log;
use zorb_pool_interface::authority::transfer_authority_impl;

use crate::UnifiedSolPoolConfig;

/// Accounts for the `TransferAuthority` instruction.
#[derive(Accounts)]
pub struct TransferAuthorityAccounts<'info> {
    /// Unified SOL pool config PDA ["unified_sol_pool"]
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,
    /// Current authority (must be signer, must match unified_sol_pool_config.authority)
    pub authority: Signer<'info>,
    /// New authority address (read-only)
    pub new_authority: &'info AccountInfo,
}

/// Process transfer authority instruction.
///
/// Sets the `pending_authority` field on the unified sol pool config. The new authority
/// must call `accept_authority` to complete the transfer.
pub fn process_transfer_authority(ctx: Context<TransferAuthorityAccounts>) -> ProgramResult {
    let TransferAuthorityAccounts {
        unified_sol_pool_config,
        authority,
        new_authority,
    } = ctx.accounts;

    unified_sol_pool_config.try_inspect_mut(|config| {
        transfer_authority_impl(config, authority.key(), new_authority.key())?;
        log!("transfer_authority: pending authority set");
        Ok(())
    })
}
