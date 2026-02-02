//! Finalize rewards by updating the reward accumulator.

use crate::{
    TokenPoolConfig, emit_event, events::TokenRewardsFinalizedEvent, gen_token_pool_config_seeds,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, instruction::Signer as PinocchioSigner,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_log::log;

/// Accounts for the FinalizeRewards instruction.
#[derive(Accounts)]
pub struct FinalizeRewardsAccounts<'info> {
    /// Pool config PDA to update
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Token pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub token_pool_program: &'info AccountInfo,
}

/// Finalize pending rewards by updating the reward accumulator.
///
/// This instruction finalizes the reward accumulator, which:
/// - Calculates reward delta from pending_rewards / (finalized_balance + pending_deposits - pending_withdrawals)
/// - Updates reward_accumulator by adding the delta
/// - Updates finalized_balance = finalized_balance + pending_deposits - pending_withdrawals
/// - Resets pending values to 0
/// - Updates last_finalized_slot
///
/// After finalization, clients can generate ZK proofs against the frozen accumulator value.
///
/// # Notes
///
/// - Anyone can call this instruction (permissionless)
/// - Will fail if UPDATE_SLOT_INTERVAL slots have not passed since last update
/// - Does nothing if there are no deposits or rewards to distribute
pub fn process_finalize_rewards(ctx: Context<FinalizeRewardsAccounts>) -> ProgramResult {
    let FinalizeRewardsAccounts { pool_config, token_pool_program } = ctx.accounts;

    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Capture pre-finalization values for event
    let (mint, deposit_fees, withdrawal_fees, funded_rewards, bump) = pool_config.map(|config| {
        (
            config.mint,
            config.pending_deposit_fees,
            config.pending_withdrawal_fees,
            config.pending_funded_rewards,
            config.bump,
        )
    })?;

    // Finalize the reward accumulator
    pool_config.try_inspect_mut(|config| {
        config.finalize_rewards(current_slot)?;
        log!("finalize_rewards: reward accumulator finalized");
        Ok(())
    })?;

    // Capture post-finalization values and emit event
    let (total_pool, new_accumulator) = pool_config.map(|config| {
        (config.finalized_balance, config.reward_accumulator)
    })?;

    let bump_bytes = [bump];
    let seeds = gen_token_pool_config_seeds(&mint, &bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    emit_event(
        pool_config.account_info(),
        token_pool_program,
        signer,
        &TokenRewardsFinalizedEvent {
            mint,
            total_pool,
            new_accumulator,
            deposit_fees,
            withdrawal_fees,
            funded_rewards,
            slot: current_slot,
        },
    )?;

    Ok(())
}
