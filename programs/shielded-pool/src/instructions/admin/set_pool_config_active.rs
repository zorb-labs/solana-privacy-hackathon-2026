//! Set the active state for a pool config.
//!
//! Enables or disables pool routing for an asset in the hub.

use crate::{
    errors::ShieldedPoolError,
    events::{PoolConfigActiveChangedEvent, emit_event},
    pda::gen_global_config_seeds,
    state::{GlobalConfig, PoolConfig},
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    msg,
};

/// Instruction data for SetPoolConfigActive.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetPoolConfigActiveData {
    /// New active state (1 = active, 0 = inactive)
    pub is_active: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}

/// Accounts for the SetPoolConfigActive instruction.
#[derive(Accounts)]
pub struct SetPoolConfigActiveAccounts<'info> {
    /// Global config PDA ["global_config"]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Pool config PDA ["pool_config", asset_id]
    #[account(mut)]
    pub pool_config: AccountLoader<'info, PoolConfig>,

    /// Must match global_config.authority
    pub authority: Signer<'info>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Set the active state for a pool config.
///
/// Enables or disables pool routing for this asset in the hub.
/// When inactive, deposits/withdrawals for this asset will fail.
///
/// # Arguments
///
/// * `is_active` - New active state (1 = active, 0 = inactive)
///
/// # Authority
///
/// Must be GlobalConfig.authority.
pub fn process_set_pool_config_active(
    ctx: Context<SetPoolConfigActiveAccounts>,
    data: SetPoolConfigActiveData,
) -> ProgramResult {
    let SetPoolConfigActiveAccounts {
        global_config,
        pool_config,
        authority,
        shielded_pool_program,
    } = ctx.accounts;

    // Validate authority against GlobalConfig and get bump for event emission
    let bump = global_config.try_map(|global_config_data| {
        if global_config_data.authority != *authority.key() {
            msg!("set_pool_config_active: unauthorized");
            return Err(ShieldedPoolError::Unauthorized.into());
        }
        Ok(global_config_data.bump)
    })?;

    // Update pool_config active state and get asset_id for event
    let asset_id = pool_config.map_mut(|pool_config_account| {
        pool_config_account.is_active = data.is_active;
        pool_config_account.asset_id
    })?;

    msg!("set_pool_config_active: success");

    // Emit event
    let bump_bytes = [bump];
    let seeds = gen_global_config_seeds(&bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    let event = PoolConfigActiveChangedEvent {
        authority: *authority.key(),
        asset_id,
        is_active: data.is_active,
        _padding: [0u8; 7],
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        signer,
        &event,
    )?;

    Ok(())
}
