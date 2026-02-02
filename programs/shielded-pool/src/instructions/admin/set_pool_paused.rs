//! Set the paused state for the pool.

use crate::{
    errors::ShieldedPoolError,
    events::{PoolPauseChangedEvent, emit_event},
    pda::gen_global_config_seeds,
    state::GlobalConfig,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    msg,
    sysvars::{Sysvar, clock::Clock},
};

/// Instruction data for SetPoolPaused.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct SetPoolPausedData {
    /// New paused state (1 = paused, 0 = active)
    pub is_paused: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}

/// Accounts for the SetPoolPaused instruction.
#[derive(Accounts)]
pub struct SetPoolPausedAccounts<'info> {
    /// Global config PDA ["global_config"]
    #[account(mut, owner = crate::ID)]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Must match global_config.authority
    pub authority: Signer<'info>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Set the paused state for the pool.
///
/// Enables or disables the pool for all operations.
///
/// # Arguments
///
/// * `is_paused` - New paused state (1 = paused, 0 = active)
pub fn process_set_pool_paused(
    ctx: Context<SetPoolPausedAccounts>,
    data: SetPoolPausedData,
) -> ProgramResult {
    let SetPoolPausedAccounts {
        global_config,
        authority,
        shielded_pool_program,
    } = ctx.accounts;

    // Get current slot for event
    let clock = Clock::get()?;

    // Validate authority and update paused state, get bump for event emission
    let bump = global_config.try_map_mut(|global_config_data| {
        if global_config_data.authority != *authority.key() {
            msg!("set_pool_paused: unauthorized");
            return Err(ShieldedPoolError::Unauthorized.into());
        }

        global_config_data.is_paused = data.is_paused;

        msg!("set_pool_paused: success");
        Ok(global_config_data.bump)
    })?;

    // Emit event
    let bump_bytes = [bump];
    let seeds = gen_global_config_seeds(&bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    let event = PoolPauseChangedEvent {
        authority: *authority.key(),
        is_paused: data.is_paused,
        _padding: [0u8; 7],
        slot: clock.slot,
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        signer,
        &event,
    )?;

    Ok(())
}
