//! Set pool fee rates.

use crate::{TokenPoolConfig, TokenPoolError};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;
use zorb_pool_interface::BASIS_POINTS;

/// Instruction data for SetFeeRates.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetFeeRatesData {
    /// New deposit fee rate in basis points (max 10000)
    pub deposit_fee_rate: u16,
    /// New withdrawal fee rate in basis points (max 10000)
    pub withdrawal_fee_rate: u16,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 4],
}

/// Accounts for the SetFeeRates instruction.
#[derive(Accounts)]
pub struct SetFeeRatesAccounts<'info> {
    /// Pool config PDA to update
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Must match pool_config.authority
    pub authority: Signer<'info>,
}

/// Update fee rates for a token pool.
///
/// Fee rates are in basis points (100 = 1%, max 10000 = 100%).
pub fn process_set_fee_rates(
    ctx: Context<SetFeeRatesAccounts>,
    data: SetFeeRatesData,
) -> ProgramResult {
    let SetFeeRatesAccounts {
        pool_config,
        authority,
    } = ctx.accounts;

    pool_config.try_inspect_mut(|config| {
        config.require_authority(authority.key())?;

        // Validate fee rates are within bounds (max 100% = 10000 basis points)
        if data.deposit_fee_rate > BASIS_POINTS as u16
            || data.withdrawal_fee_rate > BASIS_POINTS as u16
        {
            log!("set_fee_rates: fee rate exceeds 100%");
            return Err(TokenPoolError::InvalidFeeRate.into());
        }

        // Update fee rates
        config.deposit_fee_rate = data.deposit_fee_rate;
        config.withdrawal_fee_rate = data.withdrawal_fee_rate;

        log!("set_fee_rates: success");
        Ok(())
    })
}
