//! `TransferAuthority` instruction handler.
//!
//! Initiates a two-step authority transfer by setting `pending_authority`.
//! The new authority must call `accept_authority` to complete the transfer.

use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo};
use pinocchio_log::log;
use zorb_pool_interface::authority::transfer_authority_impl;

use crate::TokenPoolConfig;

/// Accounts for the `TransferAuthority` instruction.
#[derive(Accounts)]
pub struct TransferAuthorityAccounts<'info> {
    /// Pool config PDA ["token_pool", mint]
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,
    /// Current authority (must be signer, must match pool_config.authority)
    pub authority: Signer<'info>,
    /// New authority address (read-only)
    pub new_authority: &'info AccountInfo,
}

/// Process transfer authority instruction.
///
/// Sets the `pending_authority` field on the pool config. The new authority
/// must call `accept_authority` to complete the transfer.
pub fn process_transfer_authority(ctx: Context<TransferAuthorityAccounts>) -> ProgramResult {
    let TransferAuthorityAccounts {
        pool_config,
        authority,
        new_authority,
    } = ctx.accounts;

    pool_config.try_inspect_mut(|config| {
        transfer_authority_impl(config, authority.key(), new_authority.key())?;
        log!("transfer_authority: pending authority set");
        Ok(())
    })
}
