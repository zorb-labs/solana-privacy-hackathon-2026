//! Set active state for unified SOL pool config.

use crate::{UnifiedSolPoolConfig, UnifiedSolPoolError};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;

/// Instruction data for SetUnifiedSolPoolConfigActive.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetUnifiedSolPoolConfigActiveData {
    /// New active state (1 = active/enabled, 0 = inactive/disabled)
    pub is_active: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}

/// Accounts for the SetUnifiedSolPoolConfigActive instruction.
#[derive(Accounts)]
pub struct SetUnifiedSolPoolConfigActiveAccounts<'info> {
    /// UnifiedSolPoolConfig PDA to update
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// Must match unified_sol_pool_config.authority
    pub authority: Signer<'info>,
}

/// Set the active state for unified SOL pool config.
///
/// When inactive, deposits and withdrawals are blocked.
pub fn process_set_unified_sol_pool_config_active(
    ctx: Context<SetUnifiedSolPoolConfigActiveAccounts>,
    data: SetUnifiedSolPoolConfigActiveData,
) -> ProgramResult {
    let SetUnifiedSolPoolConfigActiveAccounts {
        unified_sol_pool_config,
        authority,
    } = ctx.accounts;

    unified_sol_pool_config.try_inspect_mut(|config| {
        // Verify authority
        if config.authority != *authority.key() {
            log!("set_unified_sol_pool_config_active: unauthorized");
            return Err(UnifiedSolPoolError::Unauthorized.into());
        }

        // Update active state directly (no inversion)
        config.is_active = data.is_active;

        log!("set_unified_sol_pool_config_active: updated successfully");
        Ok(())
    })
}
