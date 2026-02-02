//! Set fee rates for unified SOL pool config.

use crate::{UnifiedSolPoolConfig, UnifiedSolPoolError};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;
use zorb_pool_interface::BASIS_POINTS;

/// Instruction data for SetUnifiedSolPoolConfigFeeRates.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetUnifiedSolPoolConfigFeeRatesData {
    /// New deposit fee rate in basis points (max 10000)
    pub deposit_fee_rate: u16,
    /// New withdrawal fee rate in basis points (max 10000)
    pub withdrawal_fee_rate: u16,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 4],
}

/// Accounts for the SetUnifiedSolPoolConfigFeeRates instruction.
#[derive(Accounts)]
pub struct SetUnifiedSolPoolConfigFeeRatesAccounts<'info> {
    /// UnifiedSolPoolConfig PDA to update
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// Must match unified_sol_pool_config.authority
    pub authority: Signer<'info>,
}

/// Set the fee rates for unified SOL pool config.
pub fn process_set_unified_sol_pool_config_fee_rates(
    ctx: Context<SetUnifiedSolPoolConfigFeeRatesAccounts>,
    data: SetUnifiedSolPoolConfigFeeRatesData,
) -> ProgramResult {
    let SetUnifiedSolPoolConfigFeeRatesAccounts {
        unified_sol_pool_config,
        authority,
    } = ctx.accounts;

    // Validate fee rates (max 100%)
    if data.deposit_fee_rate > BASIS_POINTS as u16 || data.withdrawal_fee_rate > BASIS_POINTS as u16
    {
        log!("set_unified_sol_pool_config_fee_rates: fee rate too high");
        return Err(UnifiedSolPoolError::InvalidFeeRate.into());
    }

    unified_sol_pool_config.try_inspect_mut(|config| {
        // Verify authority
        if config.authority != *authority.key() {
            log!("set_unified_sol_pool_config_fee_rates: unauthorized");
            return Err(UnifiedSolPoolError::Unauthorized.into());
        }

        // Update fee rates
        config.deposit_fee_rate = data.deposit_fee_rate;
        config.withdrawal_fee_rate = data.withdrawal_fee_rate;

        log!("set_unified_sol_pool_config_fee_rates: updated successfully");
        Ok(())
    })
}
