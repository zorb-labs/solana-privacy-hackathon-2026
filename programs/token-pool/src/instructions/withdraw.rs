//! Withdraw instruction handler.
//!
//! Pool validates amounts, approves hub_authority for the output tokens,
//! updates accounting, and returns the protocol fee. Hub handles distribution.

use crate::{
    TokenPoolConfig, TokenPoolError, emit_event, events::TokenWithdrawalEvent,
    gen_token_pool_config_seeds,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, instruction::Signer as PinocchioSigner,
    program::set_return_data, pubkey::Pubkey, sysvars::Sysvar,
};
use pinocchio_log::log;
use pinocchio_token::instructions::Approve;
use zorb_pool_interface::{PoolReturnData, WithdrawParams, calculate_withdrawal_output, validate_hub_authority};

/// Accounts for the Withdraw instruction.
///
/// Pool approves hub_authority to transfer output tokens from vault.
/// Hub handles the actual distribution to recipient and relayer.
///
/// Account order must match zorb-pool-interface::withdraw_accounts:
/// 0. pool_config (mut, PDA signer)
/// 1. vault (mut)
/// 2. hub_authority (delegate for transfers)
/// 3. token_pool_program
#[derive(Accounts)]
pub struct WithdrawAccounts<'info> {
    /// Pool configuration account (PDA signer for vault operations)
    #[account(mut, owner = crate::ID)]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Vault token account (source for transfers)
    /// PDA derived from: ["vault", pool_config]
    #[account(mut, pda = Vault, pda::pool_config = pool_config.key())]
    pub vault: &'info AccountInfo,

    /// Hub authority PDA (delegate for vault transfers)
    pub hub_authority: &'info AccountInfo,

    /// Token pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub token_pool_program: &'info AccountInfo,

    /// SPL Token program (required for Approve CPI)
    #[account(address = pinocchio_token::ID)]
    pub token_program: &'info AccountInfo,
}

/// Process a withdrawal instruction.
///
/// 1. Validates caller is hub
/// 2. Parses params { amount, expected_output }
/// 3. Calculates fee = amount * withdrawal_fee_rate
/// 4. Validates: amount - fee == expected_output
/// 5. Approves hub_authority for expected_output (total tokens to distribute)
/// 6. Updates pool accounting
/// 7. Returns { fee } via set_return_data
///
/// Note: Hub uses the approval to transfer tokens from vault:
/// - (expected_output - relayer_fee) to recipient
/// - relayer_fee to relayer
/// Protocol fee stays in vault as revenue.
pub fn process_withdraw(ctx: Context<WithdrawAccounts>, instruction_data: &[u8]) -> ProgramResult {
    let WithdrawAccounts {
        pool_config,
        vault: vault_acc,
        hub_authority: hub_authority_acc,
        token_pool_program,
        token_program: _,
    } = ctx.accounts;

    // Validate hub_authority is the canonical PDA derived from hub program
    if !validate_hub_authority(hub_authority_acc.key()) {
        log!("withdraw: invalid hub_authority PDA");
        return Err(TokenPoolError::InvalidHubAuthority.into());
    }

    // Validate pool_config is the canonical PDA derived from its mint
    let pool_config_key = pool_config.key();
    let mint_for_pda = pool_config.map(|config| config.mint)?;
    TokenPoolConfig::validate_pda(pool_config_key, &mint_for_pda)?;

    // Parse instruction data (panchor strips discriminator, so we get raw params)
    let params = WithdrawParams::from_bytes(instruction_data)
        .ok_or(TokenPoolError::InvalidInstructionData)?;

    // Read config to validate and get values for PDA signer (borrow released after closure)
    let (fee, output, bump, mint): (u64, u64, u8, Pubkey) = pool_config.try_map(|config| {
        config.require_active()?;

        // Calculate fee using shared helper (None = no exchange rate for token pool)
        let (output, fee) = calculate_withdrawal_output(params.amount, config.withdrawal_fee_rate, None)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Validate expected_output matches
        if output != params.expected_output {
            return Err(TokenPoolError::ExpectedOutputMismatch.into());
        }

        Ok((fee, output, config.bump, config.mint))
    })?;

    // Build PDA signer seeds for pool_config using generated helper
    let bump_bytes = [bump];
    let seeds = gen_token_pool_config_seeds(&mint, &bump_bytes);
    let signer = [PinocchioSigner::from(&seeds)];

    // Get pool_config as AccountInfo for CPI authority
    let pool_config_info = pool_config.account_info();

    // Approve hub_authority for output tokens (hub handles distribution)
    // Hub will transfer: (output - relayer_fee) to recipient, relayer_fee to relayer
    Approve {
        source: vault_acc,
        delegate: hub_authority_acc,
        authority: pool_config_info,
        amount: output,
    }
    .invoke_signed(&signer)?;

    // Update pool state
    pool_config.try_inspect_mut(|config| {
        config.pending_withdrawals = config
            .pending_withdrawals
            .checked_add(params.amount as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        config.total_withdrawn = config
            .total_withdrawn
            .checked_add(params.amount as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Track protocol fees (fee stays in vault)
        if fee > 0 {
            config.total_withdrawal_fees = config
                .total_withdrawal_fees
                .checked_add(fee as u128)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;

            config.pending_withdrawal_fees = config
                .pending_withdrawal_fees
                .checked_add(fee)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;
        }

        // Increment withdrawal counter
        config.withdrawal_count = config
            .withdrawal_count
            .checked_add(1)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        Ok(())
    })?;

    // Emit withdrawal event FIRST (before set_return_data, since self-CPI clears return data)
    // Note: The actual recipient is determined by the hub - we use hub_authority as the delegate
    let new_balance = pool_config.try_map(|config| Ok(config.current_balance()?))?;

    let signer = PinocchioSigner::from(&seeds);

    emit_event(
        pool_config_info,
        token_pool_program,
        signer,
        &TokenWithdrawalEvent {
            mint,
            new_balance,
            amount: params.amount,
            fee,
            slot: pinocchio::sysvars::clock::Clock::get()?.slot,
            _padding: 0,
        },
    )?;

    // Return fee via set_return_data (AFTER emit_event to avoid CPI overwriting it)
    let return_data = PoolReturnData { fee };
    set_return_data(bytemuck::bytes_of(&return_data));

    Ok(())
}
