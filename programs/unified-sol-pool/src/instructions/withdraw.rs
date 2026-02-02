//! Withdraw instruction handler.
//!
//! Pool validates amounts, approves hub_authority for the output tokens,
//! updates accounting, and returns the protocol fee. Hub handles distribution.

use crate::{
    LstConfig, PoolType, UNIFIED_SOL_POOL_CONFIG_ADDRESS, UnifiedSolPoolConfig,
    UnifiedSolPoolError, emit_event, events::UnifiedSolWithdrawalEvent, find_lst_config_pda,
    gen_lst_config_seeds, gen_unified_sol_pool_config_seeds, read_token_account_balance,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Signer as PinocchioSigner,
    program::set_return_data, pubkey::Pubkey, sysvars::Sysvar,
};
use pinocchio_log::log;
use pinocchio_token::instructions::Approve;
use zorb_pool_interface::{
    BASIS_POINTS, PoolReturnData, WithdrawParams, validate_hub_authority, virtual_sol_to_tokens,
};

/// Accounts for the Withdraw instruction.
///
/// Pool approves hub_authority to transfer output tokens from vault.
/// Hub handles the actual distribution to recipient and relayer.
///
/// Account order for unified SOL pool withdrawals:
/// 0. unified_config (mut) - Master pool configuration
/// 1. lst_config (mut) - LST-specific configuration (PDA signer)
/// 2. vault (mut) - LST vault token account (PDA derived from lst_config)
/// 3. hub_authority - Hub authority PDA (delegate for transfers)
/// 4. unified_sol_program - Program account for self-CPI
#[derive(Accounts)]
pub struct WithdrawAccounts<'info> {
    /// Unified SOL pool config account
    #[account(mut, owner = crate::ID)]
    pub unified_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// LST config account (PDA signer for vault)
    #[account(mut, owner = crate::ID)]
    pub lst_config: AccountLoader<'info, LstConfig>,

    /// Vault token account (source for transfer)
    /// PDA derived from: ["lst_vault", lst_config]
    #[account(mut, pda = LstVault, pda::lst_config = lst_config.key())]
    pub vault: &'info AccountInfo,

    /// Hub authority PDA (delegate for vault transfers)
    pub hub_authority: &'info AccountInfo,

    /// Unified SOL pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub unified_sol_program: &'info AccountInfo,
}

/// Process a withdrawal instruction.
///
/// 1. Validates caller is hub
/// 2. Parses params { amount, expected_output }
/// 3. Calculates fee = amount * withdrawal_fee_rate (in virtual SOL)
/// 4. Validates: output_tokens = φ⁻¹(amount - fee) == expected_output
/// 5. Approves hub_authority for output_tokens (total tokens to distribute)
/// 6. Updates pool accounting
/// 7. Returns { fee } via set_return_data
///
/// Note: Hub uses the approval to transfer tokens from vault:
/// - (output_tokens - relayer_fee_tokens) to recipient
/// - relayer_fee_tokens to relayer
/// Protocol fee stays in vault as revenue.
pub fn process_withdraw(ctx: Context<WithdrawAccounts>, instruction_data: &[u8]) -> ProgramResult {
    let WithdrawAccounts {
        unified_config,
        lst_config,
        vault: vault_acc,
        hub_authority: hub_authority_acc,
        unified_sol_program,
    } = ctx.accounts;

    // Validate hub_authority is the canonical PDA derived from hub program
    if !validate_hub_authority(hub_authority_acc.key()) {
        log!("withdraw: invalid hub_authority PDA");
        return Err(UnifiedSolPoolError::InvalidHubAuthorityPda.into());
    }

    // Validate unified_config is the canonical singleton PDA
    if *unified_config.key() != UNIFIED_SOL_POOL_CONFIG_ADDRESS {
        log!("withdraw: invalid unified_config PDA");
        return Err(UnifiedSolPoolError::InvalidUnifiedConfigPda.into());
    }

    // Validate lst_config is the canonical PDA derived from its mint
    let lst_config_key = lst_config.key();
    let lst_mint = lst_config.map(|config| config.lst_mint)?;
    let (expected_lst_pda, _) = find_lst_config_pda(&lst_mint);
    if *lst_config_key != expected_lst_pda {
        log!("withdraw: invalid lst_config PDA");
        return Err(UnifiedSolPoolError::InvalidLstConfig.into());
    }

    // Parse instruction data (panchor strips discriminator, so we get raw params)
    let params = WithdrawParams::from_bytes(instruction_data)
        .ok_or(UnifiedSolPoolError::InvalidInstructionData)?;

    // Read values from unified config (releases borrow after closure)
    let (withdrawal_fee_rate, unified_bump) = unified_config.try_map(|config| {
        // Check pool is active
        if !config.is_active() {
            return Err(UnifiedSolPoolError::PoolPaused.into());
        }
        Ok((config.withdrawal_fee_rate, config.bump))
    })?;

    // Read values from LST config (releases borrow after closure)
    // AUDIT: Uses harvested_exchange_rate (symmetric with deposit per INV-1).
    // For withdrawals, stale rate means user gets slightly MORE tokens than current
    // market value. This is bounded by MAX_RATE_CHANGE_BPS (0.5%) and the vault
    // always has sufficient tokens since deposits entered at the same stale rate.
    // See: deposit.rs and state.rs finalize_rewards() for full timing safety analysis.
    let (exchange_rate, bump, lst_mint, pool_type): (u64, u8, Pubkey, u8) =
        lst_config.try_map(|config| {
            // Check LST is active
            if !config.is_active() {
                return Err(UnifiedSolPoolError::LstNotActive.into());
            }
            Ok((
                config.harvested_exchange_rate,
                config.bump,
                config.lst_mint,
                config.pool_type,
            ))
        })?;

    // params.amount is virtual SOL being withdrawn
    let virtual_sol = params.amount;

    // Calculate protocol fee from withdrawal_fee_rate (basis points)
    let fee = (virtual_sol as u128)
        .checked_mul(withdrawal_fee_rate as u128)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS as u128)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)? as u64;

    // Calculate output: (virtual_sol - fee) converted to tokens
    let net_virtual_sol = virtual_sol
        .checked_sub(fee)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

    // Convert virtual SOL to tokens: φ⁻¹(s) = s × ρ / λ
    let output_tokens = virtual_sol_to_tokens(net_virtual_sol, exchange_rate)
        .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

    // Validate expected_output matches
    if output_tokens != params.expected_output {
        return Err(UnifiedSolPoolError::ExpectedOutputMismatch.into());
    }

    // WSOL buffer gating: ensure minimum WSOL liquidity is maintained
    // Only WSOL withdrawals are gated - other LST withdrawals don't affect WSOL liquidity
    if pool_type == PoolType::Wsol as u8 {
        let wsol_vault_balance = read_token_account_balance(vault_acc)?;
        let required_buffer = unified_config.map(|c| c.calculate_required_buffer())??;

        let remaining = wsol_vault_balance
            .checked_sub(output_tokens)
            .ok_or(UnifiedSolPoolError::InsufficientBalance)?;

        if remaining < required_buffer {
            log!("withdraw: WSOL buffer violation");
            return Err(UnifiedSolPoolError::InsufficientBuffer.into());
        }
    }

    // Build PDA signer seeds for LST config using generated helper
    let bump_bytes = [bump];
    let seeds = gen_lst_config_seeds(&lst_mint, &bump_bytes);
    let signer = [PinocchioSigner::from(&seeds)];

    // Get lst_config AccountInfo for CPI authority
    let lst_config_info = lst_config.account_info();

    // Approve hub_authority for output tokens (hub handles distribution)
    // Hub will transfer: (output_tokens - relayer_fee) to recipient, relayer_fee to relayer
    Approve {
        source: vault_acc,
        delegate: hub_authority_acc,
        authority: lst_config_info,
        amount: output_tokens,
    }
    .invoke_signed(&signer)?;

    // Update LstConfig state: track vault token balance and virtual SOL value
    // Subtract output_tokens from vault balance counter, net_virtual_sol from value
    lst_config.try_inspect_mut(|config| {
        // Decrement vault token balance counter
        config.vault_token_balance = config
            .vault_token_balance
            .checked_sub(output_tokens)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        // Track virtual SOL value (will be recalculated atomically at finalize)
        config.total_virtual_sol = config
            .total_virtual_sol
            .checked_sub(net_virtual_sol as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        // Increment LST-specific withdrawal counter
        config.withdrawal_count = config
            .withdrawal_count
            .checked_add(1)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    })?;

    // Update unified config state (using virtual SOL)
    unified_config.try_inspect_mut(|config| {
        config.pending_withdrawals = config
            .pending_withdrawals
            .checked_add(virtual_sol as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        config.total_withdrawn = config
            .total_withdrawn
            .checked_add(virtual_sol as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        // Track total virtual SOL across all LST vaults
        // Subtract net_virtual_sol (value of tokens leaving the vault)
        config.total_virtual_sol = config
            .total_virtual_sol
            .checked_sub(net_virtual_sol as u128)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        // Track protocol fees (fees are in virtual SOL terms)
        if fee > 0 {
            config.total_withdrawal_fees = config
                .total_withdrawal_fees
                .checked_add(fee as u128)
                .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

            config.pending_withdrawal_fees = config
                .pending_withdrawal_fees
                .checked_add(fee)
                .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        }

        // Increment pool-wide withdrawal counter
        config.withdrawal_count = config
            .withdrawal_count
            .checked_add(1)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;

        Ok(())
    })?;

    // Emit withdrawal event FIRST (before set_return_data, since self-CPI clears return data)
    // Note: The actual recipient is determined by the hub - we use hub_authority as the delegate
    let unified_bump_bytes = [unified_bump];
    let unified_seeds = gen_unified_sol_pool_config_seeds(&unified_bump_bytes);
    let unified_signer = PinocchioSigner::from(&unified_seeds);

    emit_event(
        unified_config.account_info(),
        unified_sol_program,
        unified_signer,
        &UnifiedSolWithdrawalEvent {
            lst_mint,
            lst_amount: output_tokens,
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
