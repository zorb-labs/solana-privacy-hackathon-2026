//! Deposit instruction handler.
//!
//! Pool executes the token transfer itself, validates expected_output,
//! updates accounting, and returns the protocol fee via return data.

use crate::{
    TokenPoolConfig, TokenPoolError, emit_event, events::TokenDepositEvent,
    gen_token_pool_config_seeds,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Signer as PinocchioSigner,
    program::set_return_data, sysvars::Sysvar,
};
use pinocchio_token::instructions::Transfer;
use zorb_pool_interface::{DepositParams, PoolReturnData, calculate_deposit_output};

/// Accounts for the Deposit instruction.
///
/// Pool executes: depositor_token -> vault transfer.
///
/// Account order must match zorb-pool-interface::deposit_accounts:
/// 0. pool_config (mut)
/// 1. vault (mut)
/// 2. depositor_token (mut)
/// 3. depositor (signer)
/// 4. token_program
/// 5. self_program
#[derive(Accounts)]
pub struct DepositAccounts<'info> {
    /// Pool configuration account (writable for state updates)
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Vault token account (writable for receiving tokens)
    /// PDA derived from: ["vault", pool_config]
    #[account(mut, pda = Vault, pda::pool_config = pool_config.key())]
    pub vault: &'info AccountInfo,

    /// Depositor's token account (writable for transfer)
    #[account(mut)]
    pub depositor_token: &'info AccountInfo,

    /// Depositor authority (signer for transfer)
    pub depositor: Signer<'info>,

    /// SPL Token program (required for Transfer CPI)
    pub token_program: &'info AccountInfo,

    /// Token pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub token_pool_program: &'info AccountInfo,
}

/// Process a deposit instruction.
///
/// 1. Validates caller is hub
/// 2. Parses params { amount, expected_output }
/// 3. Calculates fee = amount * deposit_fee_rate
/// 4. Validates: amount - fee == expected_output
/// 5. Executes transfer: depositor_token -> vault (amount)
/// 6. Updates pool accounting
/// 7. Returns { fee } via set_return_data
pub fn process_deposit(ctx: Context<DepositAccounts>, instruction_data: &[u8]) -> ProgramResult {
    let DepositAccounts {
        pool_config,
        vault: vault_acc,
        depositor_token: depositor_token_acc,
        depositor: depositor_acc,
        token_program: _,
        token_pool_program,
    } = ctx.accounts;

    // Validate pool_config is the canonical PDA derived from its mint
    let pool_config_key = pool_config.key();
    let mint = pool_config.map(|config| config.mint)?;
    TokenPoolConfig::validate_pda(pool_config_key, &mint)?;

    // Parse instruction data (panchor strips discriminator, so we get raw params)
    let params = DepositParams::from_bytes(instruction_data)
        .ok_or(TokenPoolError::InvalidInstructionData)?;

    // Read config to calculate fee and validate (borrow released after closure)
    let (fee, principal) = pool_config.try_map(|config| {
        config.require_active()?;

        // Check deposit limit
        if params.amount > config.max_deposit_amount {
            return Err(TokenPoolError::DepositLimitExceeded.into());
        }

        // Calculate fee using shared helper (None = no exchange rate for token pool)
        let (principal, fee) = calculate_deposit_output(params.amount, config.deposit_fee_rate, None)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Validate expected_output matches
        if principal != params.expected_output {
            return Err(TokenPoolError::ExpectedOutputMismatch.into());
        }

        Ok((fee, principal))
    })?;

    // Execute transfer: depositor_token -> vault (borrow released)
    Transfer {
        from: depositor_token_acc,
        to: vault_acc,
        authority: depositor_acc,
        amount: params.amount,
    }
    .invoke()?;

    // Update pool state
    pool_config.try_inspect_mut(|config| {
        config.pending_deposits = config
            .pending_deposits
            .checked_add(principal as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        config.total_deposited = config
            .total_deposited
            .checked_add(principal as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Track protocol fees
        if fee > 0 {
            config.total_deposit_fees = config
                .total_deposit_fees
                .checked_add(fee as u128)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;

            config.pending_deposit_fees = config
                .pending_deposit_fees
                .checked_add(fee)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;
        }

        // Increment deposit counter
        config.deposit_count = config
            .deposit_count
            .checked_add(1)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        Ok(())
    })?;

    // Emit deposit event FIRST (before set_return_data, since self-CPI clears return data)
    let (new_balance, bump) = pool_config.try_map(|config| {
        Ok((config.current_balance()?, config.bump))
    })?;

    let bump_bytes = [bump];
    let seeds = gen_token_pool_config_seeds(&mint, &bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    emit_event(
        pool_config.account_info(),
        token_pool_program,
        signer,
        &TokenDepositEvent {
            mint,
            new_balance,
            amount: params.amount,
            fee,
            net_amount: principal,
            slot: pinocchio::sysvars::clock::Clock::get()?.slot,
        },
    )?;

    // Return fee via set_return_data (AFTER emit_event to avoid CPI overwriting it)
    let return_data = PoolReturnData { fee };
    set_return_data(bytemuck::bytes_of(&return_data));

    Ok(())
}
