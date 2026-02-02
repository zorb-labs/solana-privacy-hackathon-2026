//! Harvest LST appreciation into pending rewards.

use crate::{
    LstConfig, PoolType, UnifiedSolPoolConfig, UnifiedSolPoolError, emit_event,
    events::{AppreciationHarvestedEvent, ExchangeRateUpdatedEvent},
    gen_unified_sol_pool_config_seeds, read_token_account_balance,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    pubkey::Pubkey,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_log::log;

/// Accounts for the HarvestLstAppreciation instruction.
#[derive(Accounts)]
pub struct HarvestLstAppreciationAccounts<'info> {
    /// UnifiedSolPoolConfig PDA
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// LstConfig PDA for the LST being harvested
    #[account(mut, owner = crate::ID)]
    pub lst_config: AccountLoader<'info, LstConfig>,

    /// Rate data account: stake pool (for SPL pools) or lst_vault (for WSOL)
    pub rate_data_account: &'info AccountInfo,

    /// Unified SOL pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub unified_sol_program: &'info AccountInfo,
    // Remaining accounts: lst_vault (for SplStakePool type)
}

/// Harvest LST appreciation for a specific LST.
///
/// This permissionless instruction reads the current exchange rate and calculates
/// appreciation since the last update. The appreciation is added to pending rewards.
pub fn process_harvest_lst_appreciation(
    ctx: Context<HarvestLstAppreciationAccounts>,
) -> ProgramResult {
    let HarvestLstAppreciationAccounts {
        unified_sol_pool_config,
        lst_config,
        rate_data_account,
        unified_sol_program,
    } = ctx.accounts;

    // Read values from both configs (releases borrows after closure)
    let (current_epoch, unified_bump) =
        unified_sol_pool_config.map(|config| (config.reward_epoch, config.bump))?;

    let (lst_vault, stake_pool, stake_pool_program, pool_type, is_active): (
        Pubkey,
        Pubkey,
        Pubkey,
        u8,
        u8,
    ) = lst_config.map(|config| {
        (
            config.lst_vault,
            config.stake_pool,
            config.stake_pool_program,
            config.pool_type,
            config.is_active,
        )
    })?;

    // Check if LST is active
    if is_active == 0 {
        log!("harvest_lst_appreciation: LST is not active");
        return Err(UnifiedSolPoolError::LstNotActive.into());
    }

    // Get current slot and epoch from clock
    let clock = Clock::get()?;
    let current_slot = clock.slot;
    let solana_epoch = clock.epoch;

    // Read pool type
    let pool_type = PoolType::from_u8(pool_type).ok_or_else(|| {
        log!("harvest_lst_appreciation: invalid pool_type");
        UnifiedSolPoolError::InvalidPoolType
    })?;

    match pool_type {
        PoolType::Wsol => {
            // For WSOL, the rate_data_account is the lst_vault token account
            if *rate_data_account.key() != lst_vault {
                log!("harvest_lst_appreciation: expected lst_vault for WSOL");
                return Err(UnifiedSolPoolError::InvalidVault.into());
            }

            // Read vault balance for invariant check
            let vault_balance = read_token_account_balance(rate_data_account)?;

            // INVARIANT: Counter must match actual vault balance
            // This catches bugs in deposit/withdraw tracking. External transfers
            // directly to vault are not tracked (free equity to pool).
            let counter_balance = lst_config.map(|c| c.vault_token_balance)?;
            if counter_balance != vault_balance {
                log!(
                    "harvest_lst_appreciation: vault balance mismatch - counter: {}, actual: {}",
                    counter_balance,
                    vault_balance
                );
                return Err(UnifiedSolPoolError::VaultBalanceMismatch.into());
            }

            // Update LST config (WSOL rate is always 1:1, no appreciation)
            // Note: total_virtual_sol is calculated atomically at finalize time
            lst_config.inspect_mut(|lst| {
                lst.last_rate_update_slot = current_slot;
                // AUDIT: EPOCH MODEL - Mark as harvested this epoch
                // See init_unified_sol_pool_config.rs for epoch model documentation
                lst.last_harvest_epoch = current_epoch;
            })?;

            log!("harvest_lst_appreciation: WSOL harvested");
        }

        PoolType::SplStakePool => {
            // For SPL Stake Pool, the rate_data_account is the stake pool
            if *rate_data_account.key() != stake_pool {
                log!("harvest_lst_appreciation: expected stake_pool for SplStakePool");
                return Err(UnifiedSolPoolError::InvalidStakePool.into());
            }

            // Validate stake pool program ownership
            if *rate_data_account.owner() != stake_pool_program {
                log!("harvest_lst_appreciation: stake pool has wrong owner");
                return Err(UnifiedSolPoolError::InvalidStakePool.into());
            }

            // Read exchange rate from stake pool data
            // M-02 AUDIT FIX: Validates stake pool was updated in current Solana epoch
            // (see read_spl_stake_pool_rate for details)
            let new_exchange_rate =
                read_spl_stake_pool_rate(rate_data_account, solana_epoch)?;

            // AUDIT TODO: Exchange Rate Invariant - THIS IS THE ONLY RUNTIME CHECK
            // =============================================================================
            // CRITICAL: This check enforces `exchange_rate >= RATE_PRECISION (1e9)`.
            // This invariant is required for `virtual_sol_to_tokens` to be safe.
            //
            // Without this check, the unchecked `as u64` cast in `virtual_sol_to_tokens`
            // could silently truncate, causing users to receive fewer tokens.
            //
            // This is the SINGLE enforcement point after initialization. If adding new
            // code paths that modify exchange rates, they MUST also enforce this check.
            //
            // See: README.md "Audit TODOs" section for full analysis.
            // =============================================================================
            if new_exchange_rate < LstConfig::RATE_PRECISION {
                log!("harvest_lst_appreciation: exchange rate below 1:1");
                return Err(UnifiedSolPoolError::InvalidExchangeRate.into());
            }

            // Get lst_vault account from remaining accounts
            let lst_vault_account = ctx.remaining_accounts.first().ok_or_else(|| {
                log!("harvest_lst_appreciation: missing lst_vault account for SplStakePool");
                UnifiedSolPoolError::InvalidVault
            })?;

            if *lst_vault_account.key() != lst_vault {
                log!("harvest_lst_appreciation: invalid lst_vault account");
                return Err(UnifiedSolPoolError::InvalidVault.into());
            }

            let vault_balance = read_token_account_balance(lst_vault_account)?;

            // INVARIANT: Counter must match actual vault balance
            // This catches bugs in deposit/withdraw tracking. External transfers
            // directly to vault are not tracked (free equity to pool).
            let counter_balance = lst_config.map(|c| c.vault_token_balance)?;
            if counter_balance != vault_balance {
                log!(
                    "harvest_lst_appreciation: vault balance mismatch - counter: {}, actual: {}",
                    counter_balance,
                    vault_balance
                );
                return Err(UnifiedSolPoolError::VaultBalanceMismatch.into());
            }

            // Capture old exchange rate before validation and update
            let old_exchange_rate = lst_config.map(|lst| lst.exchange_rate)?;

            // AUDIT: ARBITRAGE BOUND - validate_rate_change enforces MAX_RATE_CHANGE_BPS (0.5%)
            // This bounds the maximum window for harvest-finalize timing attacks.
            // Between harvest and finalize, `exchange_rate` diverges from `harvested_exchange_rate`
            // by at most 0.5%. This caps both the stale-rate entry cost AND the capturable
            // appreciation, ensuring the attack remains unprofitable.
            // See: state.rs `finalize_rewards()` for the full economic safety proof.
            lst_config.try_inspect(|lst| {
                lst.validate_rate_change(new_exchange_rate)?;
                Ok(())
            })?;

            // Get lst_mint for events
            let lst_mint = lst_config.map(|lst| lst.lst_mint)?;

            // Update LST config and calculate appreciation using a shared variable
            // Note: total_virtual_sol is calculated atomically at finalize time
            let mut appreciation_value = 0u64;
            lst_config.try_inspect_mut(|lst| {
                appreciation_value =
                    lst.update_exchange_rate(vault_balance, new_exchange_rate, current_slot)?;
                // AUDIT: EPOCH MODEL - Mark as harvested this epoch
                // See init_unified_sol_pool_config.rs for epoch model documentation
                lst.last_harvest_epoch = current_epoch;
                Ok(())
            })?;

            if appreciation_value > 0 {
                // Add appreciation to unified config pending rewards
                unified_sol_pool_config.try_inspect_mut(|unified| {
                    unified.add_appreciation(appreciation_value)?;
                    Ok(())
                })?;
                log!("harvest_lst_appreciation: appreciation harvested");
            } else {
                log!("harvest_lst_appreciation: no appreciation to harvest");
            }

            // Emit events using unified_config as signer
            let bump_bytes = [unified_bump];
            let seeds = gen_unified_sol_pool_config_seeds(&bump_bytes);
            let signer = PinocchioSigner::from(&seeds);

            // Always emit ExchangeRateUpdatedEvent when rate is read
            emit_event(
                unified_sol_pool_config.account_info(),
                unified_sol_program,
                signer,
                &ExchangeRateUpdatedEvent {
                    lst_mint,
                    previous_rate: old_exchange_rate,
                    current_rate: new_exchange_rate,
                    slot: current_slot,
                },
            )?;

            // Emit AppreciationHarvestedEvent if appreciation occurred
            if appreciation_value > 0 {
                let signer = PinocchioSigner::from(&seeds);
                emit_event(
                    unified_sol_pool_config.account_info(),
                    unified_sol_program,
                    signer,
                    &AppreciationHarvestedEvent {
                        lst_mint,
                        previous_rate: old_exchange_rate,
                        current_rate: new_exchange_rate,
                        appreciation_amount: appreciation_value,
                        epoch: current_epoch,
                        slot: current_slot,
                    },
                )?;
            }
        }

        PoolType::Marinade | PoolType::Lido => {
            // Not yet implemented
            log!("harvest_lst_appreciation: pool type not yet implemented");
            return Err(UnifiedSolPoolError::InvalidPoolType.into());
        }
    }

    Ok(())
}

/// Read the exchange rate from an SPL Stake Pool account.
///
/// # Layout Dependency
/// This function reads data at offsets 259-283 which corresponds to:
/// - [259..267]: total_lamports (u64) - total SOL value in the pool
/// - [267..275]: pool_token_supply (u64) - total pool tokens minted
/// - [275..283]: last_update_epoch (u64) - epoch when pool was last updated
///
/// This is based on SPL Stake Pool program version 0.9.x (program ID: SPoo1...).
/// If the stake pool program layout changes, this function must be updated.
///
/// The exchange rate is calculated as: (total_lamports * 1e9) / pool_token_supply
///
/// # Epoch Validation (M-02 Audit Fix)
/// This function validates that the stake pool was updated in the current epoch.
/// At epoch boundaries, stake rewards are distributed to validators but the stake
/// pool's `total_lamports` field isn't updated until someone calls `UpdateStakePoolBalance`.
/// Reading stale data would under-count appreciation. Requiring epoch freshness ensures
/// callers get accurate exchange rates.
fn read_spl_stake_pool_rate(
    stake_pool: &AccountInfo,
    current_epoch: u64,
) -> Result<u64, UnifiedSolPoolError> {
    let data = stake_pool
        .try_borrow_data()
        .map_err(|_| UnifiedSolPoolError::InvalidInstructionData)?;

    // Need 283 bytes minimum: up to last_update_epoch field end
    if data.len() < 283 {
        log!("harvest_lst_appreciation: stake pool data too short");
        return Err(UnifiedSolPoolError::InvalidInstructionData);
    }

    // SPL stake pool offsets (struct field order: total_lamports, pool_token_supply, last_update_epoch)
    let total_lamports = u64::from_le_bytes(
        data[259..267]
            .try_into()
            .map_err(|_| UnifiedSolPoolError::InvalidInstructionData)?,
    );
    let pool_token_supply = u64::from_le_bytes(
        data[267..275]
            .try_into()
            .map_err(|_| UnifiedSolPoolError::InvalidInstructionData)?,
    );
    let last_update_epoch = u64::from_le_bytes(
        data[275..283]
            .try_into()
            .map_err(|_| UnifiedSolPoolError::InvalidInstructionData)?,
    );

    // M-02 AUDIT FIX: Validate stake pool was updated in current epoch
    // At epoch boundaries, stake rewards haven't been distributed yet, so
    // the stake pool's total_lamports may be stale. Requiring epoch freshness
    // ensures we read accurate exchange rates.
    if last_update_epoch != current_epoch {
        log!(
            "harvest_lst_appreciation: stake pool stale (updated epoch {}, current {})",
            last_update_epoch,
            current_epoch
        );
        return Err(UnifiedSolPoolError::StaleStakePoolRate.into());
    }

    if pool_token_supply == 0 {
        log!("harvest_lst_appreciation: pool_token_supply is zero");
        return Err(UnifiedSolPoolError::InvalidExchangeRate);
    }

    let rate = (total_lamports as u128)
        .checked_mul(LstConfig::RATE_PRECISION as u128)
        .and_then(|v| v.checked_div(pool_token_supply as u128))
        .ok_or_else(|| {
            log!("harvest_lst_appreciation: rate calculation overflow");
            UnifiedSolPoolError::ArithmeticOverflow
        })? as u64;

    Ok(rate)
}
