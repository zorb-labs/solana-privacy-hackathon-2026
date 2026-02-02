//! CPI helpers for calling pool plugin programs.
//!
//! # Architecture
//!
//! Pool CPI functions are pure CPI calls. They invoke the pool program and return
//! minimal data (expected_output or protocol_fee). All token distribution (recipient,
//! relayer) is handled by the orchestration layer in public_slots.rs.
//!
//! ```text
//! DEPOSIT:
//! Hub: CPI → Pool { amount, expected_output }
//! Pool: Transfer depositor→vault (amount)
//! Pool: Validate expected_output, update state, return { fee }
//!
//! WITHDRAW:
//! Hub: CPI → Pool { amount, expected_output }
//! Pool: Approve hub_authority for expected_output (total tokens to distribute)
//! Pool: Update state, return { fee }
//! ```

use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer as PinocchioSigner},
    program_error::ProgramError,
};
use zorb_pool_interface::{
    DepositParams, PoolReturnData, TOKEN_POOL_PROGRAM_ID, UNIFIED_SOL_POOL_PROGRAM_ID,
    WithdrawParams, build_deposit_instruction_data, build_withdraw_instruction_data,
};

use crate::pda::{HUB_AUTHORITY_BUMP, gen_hub_authority_seeds};

// ============================================================================
// Hub-Side Transfer Helper
// ============================================================================

/// Execute a signed transfer from vault using hub_authority delegation.
///
/// Used by the orchestration layer for both recipient and relayer transfers.
/// Skips the transfer if amount is 0 or destination is the system program (null address).
pub fn execute_signed_vault_transfer<'a>(
    vault: &'a AccountInfo,
    destination: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    amount: u64,
) -> ProgramResult {
    use pinocchio_token::instructions::Transfer;

    const NULL_ADDRESS: [u8; 32] = [0u8; 32];

    if amount == 0 || *destination.key() == NULL_ADDRESS {
        return Ok(());
    }

    let bump_bytes = [HUB_AUTHORITY_BUMP];
    let seeds = gen_hub_authority_seeds(&bump_bytes);
    let signer = [PinocchioSigner::from(&seeds)];

    Transfer {
        from: vault,
        to: destination,
        authority: hub_authority,
        amount,
    }
    .invoke_signed(&signer)
}

// ============================================================================
// CPI Return Data Helpers
// ============================================================================

/// Read pool return data after CPI.
///
/// Pool sets return data with PoolReturnData { fee }.
fn read_pool_return_data() -> Result<PoolReturnData, ProgramError> {
    let return_data =
        pinocchio::program::get_return_data().ok_or(ProgramError::InvalidAccountData)?;

    PoolReturnData::from_bytes(return_data.as_slice()).ok_or(ProgramError::InvalidAccountData)
}

// ============================================================================
// Token Pool CPI
// ============================================================================

/// Invoke token-pool withdraw via CPI (pure CPI call).
///
/// Pool validates amounts, approves hub_authority for output tokens,
/// updates state, and returns protocol fee.
///
/// Returns `expected_output` for the orchestration layer to distribute.
///
/// # Account Layout (matches token-pool WithdrawAccounts struct)
/// 0. `[writable]` Pool config account (PDA signer)
/// 1. `[writable]` Vault token account
/// 2. `[]` Hub authority PDA (delegate for vault transfers)
/// 3. `[]` Token pool program (for self-CPI events)
/// 4. `[]` SPL Token program (for Approve CPI)
pub fn execute_token_withdrawal_cpi<'a>(
    pool_config: &'a AccountInfo,
    vault: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    pool_program: &'a AccountInfo,
    token_program: &'a AccountInfo,
    amount: u64,
    expected_output: u64,
) -> Result<u64, ProgramError> {
    let params = WithdrawParams {
        amount,
        expected_output,
    };

    let instruction_data = build_withdraw_instruction_data(&params);

    let account_metas = [
        AccountMeta::writable(pool_config.key()),
        AccountMeta::writable(vault.key()),
        AccountMeta::readonly(hub_authority.key()),
        AccountMeta::readonly(pool_program.key()),
        AccountMeta::readonly(token_program.key()),
    ];

    let instruction = Instruction {
        program_id: &TOKEN_POOL_PROGRAM_ID,
        accounts: &account_metas,
        data: &instruction_data,
    };

    // Note: pool_program and token_program must be included for Solana runtime
    pinocchio::program::invoke(
        &instruction,
        &[pool_config, vault, hub_authority, pool_program, token_program],
    )?;

    // Read return data (validates pool acknowledged the CPI)
    let _return_data = read_pool_return_data()?;

    Ok(expected_output)
}

// ============================================================================
// Unified SOL Pool CPI
// ============================================================================

/// Invoke unified-sol-pool withdraw via CPI (pure CPI call).
///
/// Pool validates amounts, approves hub_authority for output tokens,
/// updates state, and returns protocol fee.
///
/// Returns `expected_output` for the orchestration layer to distribute.
///
/// # Account Layout (matches unified-sol-pool WithdrawAccounts struct)
/// 0. `[writable]` Unified SOL config account
/// 1. `[writable]` LST config account (PDA signer)
/// 2. `[writable]` Vault token account
/// 3. `[]` Hub authority PDA (delegate for vault transfers)
/// 4. `[]` Pool program (UNIFIED_SOL_POOL_PROGRAM_ID - for self-CPI events)
/// 5. `[]` SPL Token program (for Approve CPI)
pub fn execute_unified_sol_withdrawal_cpi<'a>(
    unified_config: &'a AccountInfo,
    lst_config: &'a AccountInfo,
    vault: &'a AccountInfo,
    hub_authority: &'a AccountInfo,
    pool_program: &'a AccountInfo,
    token_program: &'a AccountInfo,
    amount: u64,
    expected_output: u64,
) -> Result<u64, ProgramError> {
    let params = WithdrawParams {
        amount,
        expected_output,
    };

    let instruction_data = build_withdraw_instruction_data(&params);

    let account_metas = [
        AccountMeta::writable(unified_config.key()),
        AccountMeta::writable(lst_config.key()),
        AccountMeta::writable(vault.key()),
        AccountMeta::readonly(hub_authority.key()),
        AccountMeta::readonly(pool_program.key()),
        AccountMeta::readonly(token_program.key()),
    ];

    let instruction = Instruction {
        program_id: &UNIFIED_SOL_POOL_PROGRAM_ID,
        accounts: &account_metas,
        data: &instruction_data,
    };

    // Note: pool_program and token_program must be included for Solana runtime
    pinocchio::program::invoke(
        &instruction,
        &[unified_config, lst_config, vault, hub_authority, pool_program, token_program],
    )?;

    // Read return data (validates pool acknowledged the CPI)
    let _return_data = read_pool_return_data()?;

    Ok(expected_output)
}

// ============================================================================
// Escrow Deposit CPI Functions
// ============================================================================

/// Execute a token pool deposit from escrow vault (pure CPI call).
///
/// CPI to pool with escrow_vault_authority as signed depositor.
/// Pool executes escrow_vault→vault transfer and updates accounting.
///
/// Returns `protocol_fee`.
#[allow(clippy::too_many_arguments)]
pub fn execute_token_deposit_from_escrow_cpi<'a>(
    pool_config: &'a AccountInfo,
    vault: &'a AccountInfo,
    escrow_vault: &'a AccountInfo,
    escrow_vault_authority: &'a AccountInfo,
    token_program: &'a AccountInfo,
    escrow: &'a AccountInfo,
    pool_program: &'a AccountInfo,
    vault_authority_bump: u8,
    amount: u64,
    expected_output: u64,
) -> Result<u64, ProgramError> {
    use crate::pda::gen_escrow_vault_authority_seeds;

    let params = DepositParams {
        amount,
        expected_output,
    };

    // Build signer seeds for escrow_vault_authority PDA
    let bump_slice = [vault_authority_bump];
    let seeds = gen_escrow_vault_authority_seeds(escrow.key(), &bump_slice);
    let signer = [PinocchioSigner::from(&seeds)];

    // CPI to pool with escrow_vault_authority as the signed depositor
    let instruction_data = build_deposit_instruction_data(&params);

    let account_metas = [
        AccountMeta::writable(pool_config.key()),
        AccountMeta::writable(vault.key()),
        AccountMeta::writable(escrow_vault.key()),
        AccountMeta::readonly_signer(escrow_vault_authority.key()),
        AccountMeta::readonly(token_program.key()),
        AccountMeta::readonly(pool_program.key()), // token_pool_program for self-CPI events
    ];

    let instruction = Instruction {
        program_id: &TOKEN_POOL_PROGRAM_ID,
        accounts: &account_metas,
        data: &instruction_data,
    };

    // Note: pool_program and token_program must be included for Solana runtime to find the program executables
    pinocchio::program::invoke_signed(
        &instruction,
        &[pool_config, vault, escrow_vault, escrow_vault_authority, token_program, pool_program],
        &signer,
    )?;

    // Read return data from pool
    let return_data = read_pool_return_data()?;

    Ok(return_data.fee)
}

/// Execute a unified SOL pool deposit from escrow vault (pure CPI call).
///
/// CPI to pool with escrow_vault_authority as signed depositor.
/// Pool executes escrow_vault→vault transfer and updates accounting.
///
/// Returns `protocol_fee`.
///
/// # Account Layout (matches unified-sol-pool DepositAccounts struct)
/// 0. `[writable]` Unified SOL config account
/// 1. `[writable]` LST config account
/// 2. `[writable]` Vault token account
/// 3. `[writable]` Escrow vault (depositor's token source)
/// 4. `[signer]` Escrow vault authority (signed via PDA)
/// 5. `[]` Pool program (UNIFIED_SOL_POOL_PROGRAM_ID - for self-CPI events)
/// 6. `[]` SPL Token program (required for Transfer CPI within pool)
#[allow(clippy::too_many_arguments)]
pub fn execute_unified_sol_deposit_from_escrow_cpi<'a>(
    unified_config: &'a AccountInfo,
    lst_config: &'a AccountInfo,
    vault: &'a AccountInfo,
    escrow_vault: &'a AccountInfo,
    escrow_vault_authority: &'a AccountInfo,
    escrow: &'a AccountInfo,
    pool_program: &'a AccountInfo,
    token_program: &'a AccountInfo,
    vault_authority_bump: u8,
    amount: u64,
    expected_output: u64,
) -> Result<u64, ProgramError> {
    use crate::pda::gen_escrow_vault_authority_seeds;

    let params = DepositParams {
        amount,
        expected_output,
    };

    // Build signer seeds for escrow_vault_authority PDA
    let bump_slice = [vault_authority_bump];
    let seeds = gen_escrow_vault_authority_seeds(escrow.key(), &bump_slice);
    let signer = [PinocchioSigner::from(&seeds)];

    // CPI to pool with escrow_vault_authority as the signed depositor
    let instruction_data = build_deposit_instruction_data(&params);

    let account_metas = [
        AccountMeta::writable(unified_config.key()),
        AccountMeta::writable(lst_config.key()),
        AccountMeta::writable(vault.key()),
        AccountMeta::writable(escrow_vault.key()),
        AccountMeta::readonly_signer(escrow_vault_authority.key()),
        AccountMeta::readonly(pool_program.key()),
        AccountMeta::readonly(token_program.key()),
    ];

    let instruction = Instruction {
        program_id: &UNIFIED_SOL_POOL_PROGRAM_ID,
        accounts: &account_metas,
        data: &instruction_data,
    };

    // Note: pool_program and token_program must be included for Solana runtime to find the program executables
    // token_program is needed because the pool's Deposit handler does Transfer::invoke() to SPL Token
    pinocchio::program::invoke_signed(
        &instruction,
        &[unified_config, lst_config, vault, escrow_vault, escrow_vault_authority, pool_program, token_program],
        &signer,
    )?;

    // Read return data from pool
    let return_data = read_pool_return_data()?;

    Ok(return_data.fee)
}
