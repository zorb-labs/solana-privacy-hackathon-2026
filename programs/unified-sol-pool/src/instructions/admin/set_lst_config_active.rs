//! Set active state for LST config.

use crate::{LstConfig, UnifiedSolPoolConfig, UnifiedSolPoolError};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;

/// Instruction data for SetLstConfigActive.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetLstConfigActiveData {
    /// New active state (1 = active/enabled, 0 = inactive/disabled)
    pub is_active: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}

/// Accounts for the SetLstConfigActive instruction.
#[derive(Accounts)]
pub struct SetLstConfigActiveAccounts<'info> {
    /// UnifiedSolPoolConfig PDA (for authority check)
    #[account(owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// LstConfig PDA to update
    #[account(mut, owner = crate::ID)]
    pub lst_config: AccountLoader<'info, LstConfig>,

    /// Must match unified_sol_pool_config.authority
    pub authority: Signer<'info>,
}

/// Set the active state for an LST config.
///
/// When inactive, deposits and withdrawals for this LST are blocked.
pub fn process_set_lst_config_active(
    ctx: Context<SetLstConfigActiveAccounts>,
    data: SetLstConfigActiveData,
) -> ProgramResult {
    let SetLstConfigActiveAccounts {
        unified_sol_pool_config,
        lst_config,
        authority,
    } = ctx.accounts;

    // Read authority from unified config (releases borrow after closure)
    let unified_authority = unified_sol_pool_config.map(|config| config.authority)?;

    if unified_authority != *authority.key() {
        log!("set_lst_config_active: unauthorized");
        return Err(UnifiedSolPoolError::Unauthorized.into());
    }

    // Update LST config active state
    lst_config.try_inspect_mut(|config| {
        // Update active state directly (no inversion)
        config.is_active = data.is_active;

        log!("set_lst_config_active: updated successfully");
        Ok(())
    })
}
