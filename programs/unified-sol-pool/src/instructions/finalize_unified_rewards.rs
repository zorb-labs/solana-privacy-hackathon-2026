//! Finalize unified SOL rewards by updating the reward accumulator.

use crate::{
    LstConfig, UnifiedSolPoolConfig, UnifiedSolPoolError, emit_event,
    events::UnifiedSolRewardsFinalizedEvent, find_lst_config_pda,
    gen_unified_sol_pool_config_seeds,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, instruction::Signer as PinocchioSigner,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_log::log;

/// Maximum number of LST configs supported.
const MAX_LST_CONFIGS: usize = 16;

/// Accounts for the FinalizeUnifiedRewards instruction.
/// Additional LstConfig PDAs are passed via remaining accounts.
#[derive(Accounts)]
pub struct FinalizeUnifiedRewardsAccounts<'info> {
    /// UnifiedSolPoolConfig PDA
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// Unified SOL pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub unified_sol_program: &'info AccountInfo,
    // Remaining accounts: All registered LstConfig PDAs (mutable)
}

/// Finalize unified SOL rewards by updating the reward accumulator.
///
/// This permissionless instruction distributes pending rewards by updating
/// the accumulator. Anyone can call it once the finalization interval has passed.
///
/// Before finalizing, validates that ALL registered LST configs have been
/// harvested in the current reward epoch.
///
/// After finalization, clients can generate ZK proofs against the frozen
/// accumulator and exchange rate values.
pub fn process_finalize_unified_rewards(
    ctx: Context<FinalizeUnifiedRewardsAccounts>,
) -> ProgramResult {
    let FinalizeUnifiedRewardsAccounts {
        unified_sol_pool_config,
        unified_sol_program,
    } = ctx.accounts;

    let program_id = &crate::ID;

    // Read values from unified config (releases borrow after closure)
    let (current_epoch, lst_count, is_active, pending_deposit_fees, pending_withdrawal_fees, pending_appreciation, bump) =
        unified_sol_pool_config.map(|config| {
            (
                config.reward_epoch,
                config.lst_count,
                config.is_active,
                config.pending_deposit_fees,     // Deposit fees
                config.pending_withdrawal_fees,  // Withdrawal fees
                config.pending_appreciation,     // LST appreciation rewards
                config.bump,
            )
        })?;

    if is_active == 0 {
        log!("finalize_unified_rewards: pool is not active");
        return Err(UnifiedSolPoolError::PoolPaused.into());
    }

    // Validate correct number of LST configs
    let expected_lst_count = lst_count as usize;
    let provided_lst_count = ctx.remaining_accounts.len();

    if provided_lst_count != expected_lst_count {
        log!("finalize_unified_rewards: wrong number of LST configs");
        return Err(UnifiedSolPoolError::MissingLstConfigs.into());
    }

    // Validate all LST configs are distinct and harvested
    let mut seen_mints: [[u8; 32]; MAX_LST_CONFIGS] = [[0u8; 32]; MAX_LST_CONFIGS];
    let mut seen_count = 0usize;

    for lst_config_account in ctx.remaining_accounts {
        // Validate ownership
        if lst_config_account.owner() != program_id {
            log!("finalize_unified_rewards: invalid lst config owner");
            return Err(UnifiedSolPoolError::InvalidLstConfig.into());
        }

        // Use AccountLoader for validation
        let loader = AccountLoader::<LstConfig>::new(lst_config_account)?;
        let (lst_mint, last_harvest_epoch, is_lst_active) = loader
            .try_map(|lst_config| Ok((lst_config.lst_mint, lst_config.last_harvest_epoch, lst_config.is_active)))?;

        // Verify lst_config is the canonical PDA derived from its mint
        let (expected_pda, _) = find_lst_config_pda(&lst_mint);
        if *lst_config_account.key() != expected_pda {
            log!("finalize_unified_rewards: invalid lst_config PDA");
            return Err(UnifiedSolPoolError::InvalidLstConfig.into());
        }

        // Check for duplicate (compare lst_mint)
        for j in 0..seen_count {
            if seen_mints[j] == lst_mint {
                log!("finalize_unified_rewards: duplicate LST config");
                return Err(UnifiedSolPoolError::DuplicateLstConfig.into());
            }
        }

        // Add to seen set
        if seen_count < MAX_LST_CONFIGS {
            seen_mints[seen_count] = lst_mint;
            seen_count += 1;
        }

        // AUDIT: EPOCH MODEL - Validate LST was harvested THIS epoch
        // =====================================================================
        // This check ensures exchange rates are fresh before finalization:
        //
        // 1. harvest_lst_appreciation sets: last_harvest_epoch = reward_epoch
        // 2. This check requires: last_harvest_epoch == reward_epoch
        // 3. After finalize: reward_epoch increments, rates are frozen
        //
        // For newly initialized LSTs (last_harvest_epoch = 0), this check fails
        // because reward_epoch >= 1. They must be harvested before finalization.
        //
        // Invariant: finalize only succeeds when ALL LSTs have been harvested
        // in the current epoch, ensuring harvested_exchange_rate reflects
        // actual on-chain rates (not initialization defaults).
        //
        // See also:
        // - init_unified_sol_pool_config.rs: reward_epoch starts at 1
        // - init_lst_config.rs: last_harvest_epoch starts at 0
        // - harvest_lst_appreciation.rs: sets last_harvest_epoch = current_epoch
        // =====================================================================
        // AUDIT: INACTIVE LST HANDLING
        // Inactive LSTs skip harvest check but must still be passed.
        // Their frozen rate (harvested_exchange_rate) remains unchanged.
        // When reactivated, must harvest before next finalization.
        if is_lst_active != 0 && last_harvest_epoch != current_epoch {
            log!("finalize_unified_rewards: active LST not harvested this epoch");
            return Err(UnifiedSolPoolError::LstNotHarvested.into());
        }
    }

    // Get current slot
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Check if finalization interval has passed
    let can_finalize = unified_sol_pool_config.map(|config| {
        current_slot >= config.last_finalized_slot + UnifiedSolPoolConfig::UPDATE_SLOT_INTERVAL
    })?;

    if !can_finalize {
        log!("finalize_unified_rewards: not enough slots elapsed");
        return Err(UnifiedSolPoolError::RewardsNotReady.into());
    }

    // AUDIT: HARVEST-FINALIZE TIMING SAFETY
    // =========================================================================
    // These two steps (finalize_rewards + freeze rates) form an atomic unit:
    //
    // 1. finalize_rewards(): updates accumulator using current_balance() as denominator.
    //    This INCLUDES pending_deposits, which is correct for conservation:
    //    delta × total_pool == total_pending (zero-sum distribution).
    //
    // 2. Freeze rates: sets harvested_exchange_rate = exchange_rate for all LSTs.
    //    After this point, new deposits use the CURRENT rate (no stale-rate gap).
    //
    // Between epochs (after harvest, before this finalize), deposits enter at the
    // stale harvested_exchange_rate and capture a share of pending_appreciation.
    // This is NOT an exploit because:
    //   - Entry cost (stale rate) ≥ captured yield (proof in state.rs finalize_rewards)
    //   - MAX_RATE_CHANGE_BPS (0.5%) bounds the maximum rate divergence
    //   - Same pattern as Compound V2's cToken frozen exchangeRate
    //
    // DO NOT split these steps across transactions or change the denominator
    // in finalize_rewards to use finalized_balance — that breaks conservation.
    // =========================================================================

    // Finalize the rewards
    unified_sol_pool_config.try_inspect_mut(|config| {
        config
            .finalize_rewards(current_slot)
            .map_err(|_| UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    })?;

    // Calculate total_virtual_sol atomically from vault_token_balance × exchange_rate (INV-8)
    // This ensures value is computed at the moment rates are frozen, not from stale harvest-time data
    let mut total_pool_virtual_sol: u128 = 0;
    for lst_config_account in ctx.remaining_accounts {
        let loader = AccountLoader::<LstConfig>::new(lst_config_account)?;

        // Read vault_token_balance, exchange_rate, harvested_exchange_rate, and is_active
        let (vault_token_balance, exchange_rate, harvested_exchange_rate, is_lst_active) = loader
            .map(|c| (c.vault_token_balance, c.exchange_rate, c.harvested_exchange_rate, c.is_active))?;

        // Active LSTs: use current exchange_rate (will be frozen)
        // Inactive LSTs: use existing harvested_exchange_rate (already frozen)
        let rate_for_value = if is_lst_active != 0 { exchange_rate } else { harvested_exchange_rate };

        // Calculate: lst_total = vault_token_balance × rate_for_value / RATE_PRECISION
        let lst_total = (vault_token_balance as u128)
            .checked_mul(rate_for_value as u128)
            .and_then(|v| v.checked_div(LstConfig::RATE_PRECISION as u128))
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        total_pool_virtual_sol = total_pool_virtual_sol
            .checked_add(lst_total)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        // Update LstConfig: set total_virtual_sol and freeze exchange rate (active LSTs only)
        loader.inspect_mut(|lst_config| {
            lst_config.total_virtual_sol = lst_total;
            // Only freeze rate for active LSTs; inactive LSTs keep their frozen rate
            if lst_config.is_active != 0 {
                lst_config.harvested_exchange_rate = lst_config.exchange_rate;
            }
        })?;
    }

    // Update unified config's total_virtual_sol with the sum of all LST values
    unified_sol_pool_config.inspect_mut(|config| {
        config.total_virtual_sol = total_pool_virtual_sol;
    })?;

    log!("finalize_unified_rewards: rewards finalized, rates frozen");

    // Emit finalization event (total_virtual_sol now reflects the sum)
    let (total_virtual_sol, new_accumulator, new_epoch) = unified_sol_pool_config.map(|config| {
        (
            config.total_virtual_sol,
            config.reward_accumulator,
            config.reward_epoch,
        )
    })?;

    let bump_bytes = [bump];
    let seeds = gen_unified_sol_pool_config_seeds(&bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    emit_event(
        unified_sol_pool_config.account_info(),
        unified_sol_program,
        signer,
        &UnifiedSolRewardsFinalizedEvent {
            total_virtual_sol,
            new_accumulator,
            deposit_fees: pending_deposit_fees,
            withdrawal_fees: pending_withdrawal_fees,
            appreciation_rewards: pending_appreciation,
            epoch: new_epoch, // The new epoch after finalization
            slot: current_slot,
            lst_count,
            _padding: [0u8; 7],
        },
    )?;

    Ok(())
}
