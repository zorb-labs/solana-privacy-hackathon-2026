//! Deposit instruction handler.
//!
//! Pool executes the token transfer itself, validates expected_output
//! using exchange rate conversion, updates accounting, and returns the protocol fee.

use crate::{
    LstConfig, UNIFIED_SOL_POOL_CONFIG_ADDRESS, UnifiedSolPoolConfig, UnifiedSolPoolError,
    emit_event, events::UnifiedSolDepositEvent, find_lst_config_pda,
    gen_unified_sol_pool_config_seeds,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Signer as PinocchioSigner,
    program::set_return_data, sysvars::Sysvar,
};
use pinocchio_log::log;
use pinocchio_token::instructions::Transfer;
use zorb_pool_interface::{BASIS_POINTS, DepositParams, PoolReturnData, tokens_to_virtual_sol};

/// Accounts for the Deposit instruction.
///
/// Pool executes: depositor_token -> vault transfer.
///
/// Account order for unified SOL pool deposits:
/// 0. unified_config (mut) - Master pool configuration
/// 1. lst_config (mut) - LST-specific configuration
/// 2. vault (mut) - LST vault token account (PDA derived from lst_config)
/// 3. depositor_token (mut) - Depositor's token account
/// 4. depositor (signer) - Depositor authority
/// 5. unified_sol_program - Program account for self-CPI
/// 6. token_program - SPL Token program (required for Transfer CPI)
#[derive(Accounts)]
pub struct DepositAccounts<'info> {
    /// Unified SOL pool config account
    #[account(mut, owner = crate::ID)]
    pub unified_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// LST config account
    #[account(mut, owner = crate::ID)]
    pub lst_config: AccountLoader<'info, LstConfig>,

    /// Vault token account (writable for receiving tokens)
    /// PDA derived from: ["lst_vault", lst_config]
    #[account(mut, pda = LstVault, pda::lst_config = lst_config.key())]
    pub vault: &'info AccountInfo,

    /// Depositor's token account (writable for transfer)
    #[account(mut)]
    pub depositor_token: &'info AccountInfo,

    /// Depositor authority (signer for transfer)
    pub depositor: Signer<'info>,

    /// Unified SOL pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub unified_sol_program: &'info AccountInfo,

    /// SPL Token program (required for Transfer CPI)
    pub token_program: &'info AccountInfo,
}

/// Process a deposit instruction.
///
/// 1. Validates caller is hub
/// 2. Parses params { amount, expected_output }
/// 3. Converts amount to virtual SOL using exchange rate
/// 4. Calculates fee = virtual_sol * deposit_fee_rate
/// 5. Validates: virtual_sol - fee == expected_output
/// 6. Executes transfer: depositor_token -> vault (amount)
/// 7. Updates pool accounting
/// 8. Returns { fee } via set_return_data
pub fn process_deposit(ctx: Context<DepositAccounts>, instruction_data: &[u8]) -> ProgramResult {
    let DepositAccounts {
        unified_config,
        lst_config,
        vault: vault_acc,
        depositor_token: depositor_token_acc,
        depositor: depositor_acc,
        unified_sol_program,
        token_program: _,
    } = ctx.accounts;

    // Validate unified_config is the canonical singleton PDA
    if *unified_config.key() != UNIFIED_SOL_POOL_CONFIG_ADDRESS {
        log!("deposit: invalid unified_config PDA");
        return Err(UnifiedSolPoolError::InvalidUnifiedConfigPda.into());
    }

    // Validate lst_config is the canonical PDA derived from its mint
    let lst_config_key = lst_config.key();
    let lst_mint = lst_config.map(|config| config.lst_mint)?;
    let (expected_lst_pda, _) = find_lst_config_pda(&lst_mint);
    if *lst_config_key != expected_lst_pda {
        log!("deposit: invalid lst_config PDA");
        return Err(UnifiedSolPoolError::InvalidLstConfig.into());
    }

    // Parse instruction data (panchor strips discriminator, so we get raw params)
    let params = DepositParams::from_bytes(instruction_data)
        .ok_or(UnifiedSolPoolError::InvalidInstructionData)?;

    // Read values from unified config (releases borrow after closure)
    let (deposit_fee_rate, reward_epoch, unified_bump) = unified_config.try_map(|config| {
        // Check pool is active
        if !config.is_active() {
            return Err(UnifiedSolPoolError::PoolPaused.into());
        }
        Ok((config.deposit_fee_rate, config.reward_epoch, config.bump))
    })?;

    // Read values from LST config (releases borrow after closure)
    //
    // AUDIT: STALE RATE AS ECONOMIC BARRIER (Harvest-Finalize Timing Safety)
    // =========================================================================
    // This uses `harvested_exchange_rate` (frozen at previous finalization), NOT the
    // current `exchange_rate` (updated at harvest). After harvest but before finalize,
    // `exchange_rate > harvested_exchange_rate`, meaning depositors receive FEWER
    // virtual SOL than their tokens' true market value.
    //
    // This stale-rate entry cost is the natural economic barrier that prevents
    // harvest-finalize timing arbitrage:
    //
    //   cost_of_entry = tokens × (exchange_rate - harvested_exchange_rate) / 1e9
    //   captured_yield = virtual_sol_credited × pending_rewards / total_pool
    //
    // Mathematical proof that cost ≥ yield:
    //   Both scale with rate_delta, but captured_yield is further diluted by the
    //   ratio (vault_tokens / total_pool) ≤ 1, so cost always dominates.
    //
    // DO NOT "fix" this to use `exchange_rate` — that would remove the economic
    // barrier and enable profitable timing attacks on the reward accumulator.
    //
    // See: state.rs `finalize_rewards()` for the conservation proof.
    // See: state.rs `MAX_RATE_CHANGE_BPS` for the arbitrage bound (0.5% max).
    // =========================================================================
    let exchange_rate = lst_config.try_map(|config| {
        // Check LST is active
        if !config.is_active() {
            return Err(UnifiedSolPoolError::LstNotActive.into());
        }
        // Validate harvest epoch (LST must be harvested before deposits)
        if reward_epoch > 0 && config.last_harvest_epoch < reward_epoch.checked_sub(1).unwrap() {
            return Err(UnifiedSolPoolError::LstNotHarvested.into());
        }
        Ok(config.harvested_exchange_rate)
    })?;

    // Convert token amount to virtual SOL: φ(e) = e × λ / ρ
    let virtual_sol = tokens_to_virtual_sol(params.amount, exchange_rate)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)? as u64;

    // Calculate protocol fee from deposit_fee_rate (basis points)
    let fee = (virtual_sol as u128)
        .checked_mul(deposit_fee_rate as u128)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS as u128)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)? as u64;

    // Validate expected_output matches
    let principal = virtual_sol
        .checked_sub(fee)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

    if principal != params.expected_output {
        return Err(UnifiedSolPoolError::ExpectedOutputMismatch.into());
    }

    // Execute transfer: depositor_token -> vault
    Transfer {
        from: depositor_token_acc,
        to: vault_acc,
        authority: depositor_acc,
        amount: params.amount,
    }
    .invoke()?;

    // Update LstConfig state: track vault token balance and virtual SOL value
    // Uses harvested_exchange_rate for consistency with ZK proofs
    lst_config.try_inspect_mut(|config| {
        // Increment vault token balance counter
        config.vault_token_balance = config
            .vault_token_balance
            .checked_add(params.amount)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        // Track virtual SOL value (will be recalculated atomically at finalize)
        config.total_virtual_sol = config
            .total_virtual_sol
            .checked_add(virtual_sol as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        // Increment LST-specific deposit counter
        config.deposit_count = config
            .deposit_count
            .checked_add(1)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    })?;

    // Update unified config state (using principal, not virtual_sol)
    unified_config.try_inspect_mut(|config| {
        config.pending_deposits = config
            .pending_deposits
            .checked_add(principal as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        config.total_deposited = config
            .total_deposited
            .checked_add(principal as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        // Track total virtual SOL across all LST vaults
        config.total_virtual_sol = config
            .total_virtual_sol
            .checked_add(virtual_sol as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        // Track protocol fees (fees are in virtual SOL terms)
        if fee > 0 {
            config.total_deposit_fees = config
                .total_deposit_fees
                .checked_add(fee as u128)
                .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

            config.pending_deposit_fees = config
                .pending_deposit_fees
                .checked_add(fee)
                .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        }

        // Increment pool-wide deposit counter
        config.deposit_count = config
            .deposit_count
            .checked_add(1)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        Ok(())
    })?;

    // Emit deposit event FIRST (before set_return_data, since self-CPI clears return data)
    let bump_bytes = [unified_bump];
    let seeds = gen_unified_sol_pool_config_seeds(&bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    emit_event(
        unified_config.account_info(),
        unified_sol_program,
        signer,
        &UnifiedSolDepositEvent {
            lst_mint,
            lst_amount: params.amount,
            sol_value: virtual_sol,
            fee,
            exchange_rate,
            slot: pinocchio::sysvars::clock::Clock::get()?.slot,
            _padding: 0,
        },
    )?;

    // Return fee via set_return_data (AFTER emit_event to avoid CPI overwriting it)
    let return_data = PoolReturnData { fee };
    set_return_data(bytemuck::bytes_of(&return_data));

    Ok(())
}
