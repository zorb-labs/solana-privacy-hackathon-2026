//! Set pool active state.

use crate::TokenPoolConfig;
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;

/// Instruction data for SetPoolActive.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetPoolActiveData {
    /// New active state (1 = active/enabled, 0 = inactive/disabled)
    pub is_active: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}

/// Accounts for the SetPoolActive instruction.
#[derive(Accounts)]
pub struct SetPoolActiveAccounts<'info> {
    /// Pool config PDA to update
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Must match pool_config.authority
    pub authority: Signer<'info>,
}

/// Set the active state for a token pool.
///
/// When inactive, deposits and withdrawals are blocked.
pub fn process_set_pool_active(
    ctx: Context<SetPoolActiveAccounts>,
    data: SetPoolActiveData,
) -> ProgramResult {
    let SetPoolActiveAccounts {
        pool_config,
        authority,
    } = ctx.accounts;

    pool_config.try_inspect_mut(|config| {
        config.require_authority(authority.key())?;

        // Update active state directly (no inversion)
        config.is_active = data.is_active;

        log!("set_pool_active: success");
        Ok(())
    })
}
