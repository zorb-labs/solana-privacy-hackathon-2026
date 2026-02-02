//! Public slot execution orchestration for execute_transact.
//!
//! This module orchestrates all operations for each active public slot:
//! 1. Escrow verification (deposits only)
//! 2. Pool CPI (deposit or withdrawal)
//! 3. Escrow consumption (deposits only)
//! 4. Recipient distribution (withdrawals only)
//! 5. Relayer fee transfer
//!
//! # Relayer Fee Source
//! - **Deposit** (ext_amount > 0): escrow_vault → relayer_token (escrow_vault_authority signs)
//! - **Withdrawal** (ext_amount < 0): vault → relayer_token (hub_authority signs via delegation)

use crate::{
    errors::ShieldedPoolError,
    instructions::types::{N_PUBLIC_LINES, TransactParams},
    pda::find_escrow_vault_authority_pda,
    pool_cpi::{
        execute_signed_vault_transfer, execute_token_deposit_from_escrow_cpi,
        execute_token_withdrawal_cpi, execute_unified_sol_deposit_from_escrow_cpi,
        execute_unified_sol_withdrawal_cpi,
    },
    state::{LstConfig, TokenPoolConfig, UnifiedSolPoolConfig},
};

use super::accounts::{SlotAccounts, TokenSlotAccounts, UnifiedSolSlotAccounts};
use super::deposit_escrow::{mark_escrow_consumed, verify_escrow_for_deposit};
use panchor::prelude::AccountLoader;
use pinocchio::{
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use pinocchio_token::instructions::Transfer;
use zorb_pool_interface::{tokens_to_virtual_sol, virtual_sol_to_tokens};

use super::fee::calculate_fee;

// ============================================================================
// Public Slot Execution
// ============================================================================

/// Execute all operations for active public slots.
///
/// For each non-zero ext_amount, orchestrates:
/// 1. Escrow verification (deposits)
/// 2. Pool CPI (deposit/withdrawal)
/// 3. Escrow consumption (deposits)
/// 4. Recipient distribution (withdrawals)
/// 5. Relayer fee transfer
#[inline(never)]
pub fn execute_public_slots<'a>(
    program_id: &Pubkey,
    slot_accounts: &[Option<SlotAccounts<'a>>; N_PUBLIC_LINES],
    token_program: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    transact_params: &TransactParams,
    session_data: &[u8],
    relayer_key: &Pubkey,
) -> Result<(), ProgramError> {
    for i in 0..N_PUBLIC_LINES {
        let ext_amount = transact_params.ext_amounts[i];

        // Skip inactive slots
        if ext_amount == 0 {
            continue;
        }

        let slot = slot_accounts[i]
            .as_ref()
            .ok_or(ShieldedPoolError::MissingAccounts)?;

        let relayer_fee = transact_params.relayer_fees[i];

        // 1. Verify escrow (deposits only)
        if ext_amount > 0 {
            verify_escrow_for_deposit(program_id, slot.escrow(), session_data, relayer_key)?;
        }

        // 2. Pool CPI — returns expected_output for withdrawals (0 for deposits)
        let expected_output = match slot {
            SlotAccounts::UnifiedSol(unified) => {
                execute_unified_sol_slot_cpi(
                    token_program,
                    hub_authority,
                    unified,
                    ext_amount,
                )?
            }
            SlotAccounts::Token(token) => {
                execute_token_slot_cpi(
                    token_program,
                    hub_authority,
                    token,
                    ext_amount,
                )?
            }
        };

        // 3. Mark escrow consumed (deposits only)
        if ext_amount > 0 {
            mark_escrow_consumed(slot.escrow())?;
        }

        // 4. Distribute withdrawal output to recipient
        if ext_amount < 0 {
            let recipient_amount = expected_output
                .checked_sub(relayer_fee)
                .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
            distribute_to_recipient(
                slot.vault(),
                slot.recipient_token(),
                hub_authority,
                recipient_amount,
            )?;
        }

        // 5. Relayer fee — direction determines source
        transfer_relayer_fee(slot, hub_authority, relayer_fee, ext_amount)?;
    }
    Ok(())
}

// ============================================================================
// Pool CPI Dispatch (compute amounts + call pool)
// ============================================================================

/// Execute unified SOL pool CPI for a slot.
///
/// Computes deposit/withdrawal amounts with exchange rate conversion,
/// then calls the pure pool CPI. Returns `expected_output` in tokens (0 for deposits).
///
/// # Domain Boundary Principle
///
/// ext_amount is always in **LST tokens** (domain E). Conversion to virtual SOL (domain S)
/// happens at the execution boundary via φ (tokens → virtual SOL) and φ⁻¹ (virtual SOL → tokens).
///
/// - Deposit: `s = φ(e)`, fee calculated on s, `p = s - f`
/// - Withdrawal: `|e| = φ⁻¹(s - f)`, reverse-engineer s from |e|, `p = -s`
#[inline(never)]
fn execute_unified_sol_slot_cpi<'a>(
    token_program: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    slot: &UnifiedSolSlotAccounts<'a>,
    ext_amount: i64,
) -> Result<u64, ProgramError> {
    // Load exchange rate from LstConfig
    let exchange_rate = AccountLoader::<LstConfig>::new(slot.lst_config)?
        .map(|config| config.harvested_exchange_rate)?;

    if ext_amount > 0 {
        // Deposit: ext_amount is GROSS tokens (domain E)
        // Per formal model: s = φ(e), p = s - f
        let amount_tokens = ext_amount as u64;

        // Convert tokens to virtual SOL at boundary: s = φ(e)
        let virtual_sol = tokens_to_virtual_sol(amount_tokens, exchange_rate)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)? as u64;

        let deposit_fee_rate =
            AccountLoader::<UnifiedSolPoolConfig>::new(slot.unified_sol_pool_config)?
                .map(|config| config.deposit_fee_rate)?;

        // Fee calculated in domain S (virtual SOL)
        let fee = calculate_fee(virtual_sol, deposit_fee_rate)?;

        let expected_output = virtual_sol
            .checked_sub(fee)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        let (_, vault_authority_bump) = find_escrow_vault_authority_pda(slot.escrow.key());

        execute_unified_sol_deposit_from_escrow_cpi(
            slot.unified_sol_pool_config,
            slot.lst_config,
            slot.vault,
            slot.escrow_token,
            slot.escrow_vault_authority,
            slot.escrow,
            slot.pool_program,
            token_program,
            vault_authority_bump,
            amount_tokens,
            expected_output,
        )?;

        Ok(0) // No expected_output to distribute for deposits
    } else {
        // Withdrawal: ext_amount is NET tokens (domain E)
        // Per formal model: |e| = φ⁻¹(s - f), so φ(|e|) = s - f
        // We have |e| in tokens and need to recover s (gross virtual SOL)
        let net_tokens = (-ext_amount) as u64;

        // Convert net tokens to net virtual SOL: φ(|e|) = s - f
        let net_virtual_sol = tokens_to_virtual_sol(net_tokens, exchange_rate)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)? as u64;

        let withdrawal_fee_rate =
            AccountLoader::<UnifiedSolPoolConfig>::new(slot.unified_sol_pool_config)?
                .map(|config| config.withdrawal_fee_rate)?;

        // Reverse-engineer gross virtual SOL from net:
        // Given: net = gross - fee = gross - (gross × rate / B) = gross × (B - rate) / B
        // Solve: gross = net × B / (B - rate)
        let rate = withdrawal_fee_rate as u64;
        let denominator = BASIS_POINTS
            .checked_sub(rate)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        if denominator == 0 {
            return Err(ShieldedPoolError::ArithmeticOverflow.into());
        }

        let gross_virtual_sol = net_virtual_sol
            .checked_mul(BASIS_POINTS)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?
            .checked_div(denominator)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        // Pool calculates: fee = gross × rate / B, net = gross - fee
        // Convert net to tokens for actual transfer: tokens = φ⁻¹(net)
        let fee = calculate_fee(gross_virtual_sol, withdrawal_fee_rate)?;
        let actual_net_virtual_sol = gross_virtual_sol
            .checked_sub(fee)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        let expected_output_tokens = virtual_sol_to_tokens(actual_net_virtual_sol, exchange_rate)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        execute_unified_sol_withdrawal_cpi(
            slot.unified_sol_pool_config,
            slot.lst_config,
            slot.vault,
            hub_authority,
            slot.pool_program,
            token_program,
            gross_virtual_sol,
            expected_output_tokens,
        )?;

        Ok(expected_output_tokens)
    }
}

/// Basis points constant (100% = 10000).
const BASIS_POINTS: u64 = 10_000;

/// Execute token pool CPI for a slot.
///
/// Computes deposit/withdrawal amounts with fee calculation,
/// then calls the pure pool CPI. Returns `expected_output` (0 for deposits).
///
/// # Withdrawal Semantics
///
/// For withdrawals, `|ext_amount|` is the NET amount (after protocol fee deduction).
/// This is what TypeScript computes: `ext_amount = -(gross_shielded - fee)`.
///
/// We must reverse-engineer the gross amount for the pool CPI, since the pool
/// expects to calculate fee from gross and validate: `gross - fee == expected_output`.
#[inline(never)]
fn execute_token_slot_cpi<'a>(
    token_program: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    slot: &TokenSlotAccounts<'a>,
    ext_amount: i64,
) -> Result<u64, ProgramError> {
    if ext_amount > 0 {
        // Deposit: ext_amount is GROSS (what enters the vault)
        let amount = ext_amount as u64;

        let deposit_fee_rate = AccountLoader::<TokenPoolConfig>::new(slot.token_pool_config)?
            .map(|config| config.deposit_fee_rate)?;

        let fee = calculate_fee(amount, deposit_fee_rate)?;

        let expected_output = amount
            .checked_sub(fee)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        let (_, vault_authority_bump) = find_escrow_vault_authority_pda(slot.escrow.key());

        execute_token_deposit_from_escrow_cpi(
            slot.token_pool_config,
            slot.vault,
            slot.escrow_token,
            slot.escrow_vault_authority,
            token_program,
            slot.escrow,
            slot.pool_program,
            vault_authority_bump,
            amount,
            expected_output,
        )?;

        Ok(0) // No expected_output to distribute for deposits
    } else {
        // Withdrawal: ext_amount is NET (what recipient+relayer will receive)
        // Formula from spec: ext_amount = -(gross - fee), so |ext_amount| = gross - fee = NET
        let net_output = (-ext_amount) as u64;

        let withdrawal_fee_rate = AccountLoader::<TokenPoolConfig>::new(slot.token_pool_config)?
            .map(|config| config.withdrawal_fee_rate)?;

        // Reverse-engineer gross amount: gross = net * 10000 / (10000 - rate)
        // This ensures pool's calculation: gross - fee(gross) == net
        let rate = withdrawal_fee_rate as u64;
        let denominator = BASIS_POINTS
            .checked_sub(rate)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        if denominator == 0 {
            return Err(ShieldedPoolError::ArithmeticOverflow.into());
        }

        let gross_amount = net_output
            .checked_mul(BASIS_POINTS)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?
            .checked_div(denominator)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        // Compute actual expected_output (what pool will calculate and approve)
        // May differ slightly from net_output due to integer division rounding
        let fee = calculate_fee(gross_amount, withdrawal_fee_rate)?;
        let expected_output = gross_amount
            .checked_sub(fee)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        execute_token_withdrawal_cpi(
            slot.token_pool_config,
            slot.vault,
            hub_authority,
            slot.pool_program,
            token_program,
            gross_amount,
            expected_output,
        )?;

        Ok(expected_output)
    }
}

// ============================================================================
// Distribution Helpers
// ============================================================================

/// Transfer withdrawal output to recipient via hub_authority delegation.
#[inline(always)]
fn distribute_to_recipient<'a>(
    vault: &'a AccountInfo,
    recipient_token: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    recipient_amount: u64,
) -> Result<(), ProgramError> {
    execute_signed_vault_transfer(vault, recipient_token, hub_authority, recipient_amount)
}

/// Transfer relayer fee from the appropriate source.
///
/// - **Deposit**: escrow_vault → relayer_token (escrow_vault_authority signs)
/// - **Withdrawal**: vault → relayer_token (hub_authority signs via delegation)
#[inline(never)]
fn transfer_relayer_fee<'a>(
    slot: &SlotAccounts<'a>,
    hub_authority: &'a AccountInfo,
    relayer_fee: u64,
    ext_amount: i64,
) -> Result<(), ProgramError> {
    if relayer_fee == 0 {
        return Ok(());
    }

    const NULL_ADDRESS: [u8; 32] = [0u8; 32];
    if *slot.relayer_token().key() == NULL_ADDRESS {
        return Ok(());
    }

    if ext_amount > 0 {
        // Deposit: escrow pays relayer
        use crate::pda::gen_escrow_vault_authority_seeds;

        let (_, bump) = find_escrow_vault_authority_pda(slot.escrow().key());
        let bump_slice = [bump];
        let seeds = gen_escrow_vault_authority_seeds(slot.escrow().key(), &bump_slice);
        let signer = [PinocchioSigner::from(&seeds)];

        Transfer {
            from: slot.escrow_token(),
            to: slot.relayer_token(),
            authority: slot.escrow_vault_authority(),
            amount: relayer_fee,
        }
        .invoke_signed(&signer)?;
    } else {
        // Withdrawal: unshielded output pays relayer from vault
        execute_signed_vault_transfer(
            slot.vault(),
            slot.relayer_token(),
            hub_authority,
            relayer_fee,
        )?;
    }

    Ok(())
}
