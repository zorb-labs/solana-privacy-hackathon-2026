//! CPI helpers for SPL Stake Pool integration.
//!
//! This module provides functions to interact with SPL Stake Pool implementations
//! (Jito, Sanctum vSOL, etc.) for depositing SOL and receiving LST tokens.

use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// SPL Stake Pool program ID
pub const SPL_STAKE_POOL_PROGRAM_ID: Pubkey = [
    // SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy
    0x0b, 0x0d, 0xfe, 0xd1, 0x0d, 0xd9, 0x8c, 0x67, 0x21, 0xbb, 0x4f, 0x6f, 0x6b, 0x84, 0x4f, 0x98,
    0x8e, 0x25, 0x87, 0xbe, 0x8b, 0x92, 0xfc, 0x8f, 0x5d, 0x90, 0x2b, 0xa6, 0x9c, 0x4e, 0x7f, 0x52,
];

/// SPL Stake Pool instruction discriminators
pub mod instruction {
    pub const DEPOSIT_SOL: u8 = 14;
    pub const WITHDRAW_SOL: u8 = 16;
}

/// Stake Pool account layout offsets for reading exchange rate
pub const STAKE_POOL_SIZE: usize = 639; // Approximate size
pub const STAKE_POOL_POOL_TOKEN_SUPPLY_OFFSET: usize = 258;
pub const STAKE_POOL_TOTAL_LAMPORTS_OFFSET: usize = 266;

/// Read the exchange rate from a stake pool account.
/// Returns (pool_token_supply, total_lamports).
/// Exchange rate = total_lamports / pool_token_supply
pub fn get_stake_pool_exchange_rate(stake_pool: &AccountInfo) -> Result<(u64, u64), ProgramError> {
    let data = stake_pool.try_borrow_data()?;
    if data.len() < STAKE_POOL_TOTAL_LAMPORTS_OFFSET + 8 {
        return Err(ProgramError::InvalidAccountData);
    }

    let pool_token_supply = u64::from_le_bytes(
        data[STAKE_POOL_POOL_TOKEN_SUPPLY_OFFSET..STAKE_POOL_POOL_TOKEN_SUPPLY_OFFSET + 8]
            .try_into()
            .unwrap(),
    );

    let total_lamports = u64::from_le_bytes(
        data[STAKE_POOL_TOTAL_LAMPORTS_OFFSET..STAKE_POOL_TOTAL_LAMPORTS_OFFSET + 8]
            .try_into()
            .unwrap(),
    );

    Ok((pool_token_supply, total_lamports))
}

/// Calculate the exchange rate scaled by 1e9.
/// Returns: rate = (total_lamports * 1e9) / pool_token_supply
/// This represents how many lamports each pool token is worth.
pub fn calculate_exchange_rate(pool_token_supply: u64, total_lamports: u64) -> u64 {
    if pool_token_supply == 0 {
        return 1_000_000_000; // 1:1 if no tokens yet
    }
    ((total_lamports as u128 * 1_000_000_000) / pool_token_supply as u128) as u64
}

/// Deposit SOL into the stake pool and receive LST tokens.
///
/// This wraps the SPL Stake Pool's DepositSol instruction.
///
/// # Accounts (in order):
/// 0. `[writable]` stake_pool - The stake pool account
/// 1. `[]` withdraw_authority - Pool's withdraw authority PDA
/// 2. `[writable]` reserve_stake - Pool's reserve stake account
/// 3. `[writable, signer]` from - Source of lamports (our PDA with SOL)
/// 4. `[writable]` dest_pool_token - Destination for pool tokens (vSOL)
/// 5. `[writable]` manager_fee_account - Pool's manager fee token account
/// 6. `[writable]` referrer_fee_account - Referrer fee account (can be manager's)
/// 7. `[writable]` pool_mint - Pool token mint (vSOL mint)
/// 8. `[]` system_program
/// 9. `[]` token_program
///
/// # Arguments
/// * `lamports` - Amount of SOL lamports to deposit
/// * `signer_seeds` - Seeds for the PDA that holds the SOL
pub fn deposit_sol(
    stake_pool: &AccountInfo,
    withdraw_authority: &AccountInfo,
    reserve_stake: &AccountInfo,
    from: &AccountInfo,
    dest_pool_token: &AccountInfo,
    manager_fee_account: &AccountInfo,
    referrer_fee_account: &AccountInfo,
    pool_mint: &AccountInfo,
    system_program: &AccountInfo,
    token_program: &AccountInfo,
    lamports: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    // Build instruction data: discriminator (1 byte) + lamports (8 bytes)
    let mut instruction_data = [0u8; 9];
    instruction_data[0] = instruction::DEPOSIT_SOL;
    instruction_data[1..9].copy_from_slice(&lamports.to_le_bytes());

    // Build account metas using pinocchio API
    let account_metas = [
        AccountMeta::writable(stake_pool.key()),
        AccountMeta::readonly(withdraw_authority.key()),
        AccountMeta::writable(reserve_stake.key()),
        AccountMeta::writable_signer(from.key()),
        AccountMeta::writable(dest_pool_token.key()),
        AccountMeta::writable(manager_fee_account.key()),
        AccountMeta::writable(referrer_fee_account.key()),
        AccountMeta::writable(pool_mint.key()),
        AccountMeta::readonly(system_program.key()),
        AccountMeta::readonly(token_program.key()),
    ];

    let instruction = Instruction {
        program_id: &SPL_STAKE_POOL_PROGRAM_ID,
        accounts: &account_metas,
        data: &instruction_data,
    };

    // Convert signer_seeds to Seed types
    let seeds: [Seed; 3] = [
        Seed::from(signer_seeds[0]),
        Seed::from(signer_seeds[1]),
        Seed::from(signer_seeds[2]),
    ];
    let signer = [Signer::from(&seeds[..])];

    invoke_signed(
        &instruction,
        &[
            stake_pool,
            withdraw_authority,
            reserve_stake,
            from,
            dest_pool_token,
            manager_fee_account,
            referrer_fee_account,
            pool_mint,
            system_program,
            token_program,
        ],
        &signer,
    )?;

    Ok(())
}

/// Withdraw SOL from the stake pool by burning LST tokens.
///
/// This wraps the SPL Stake Pool's WithdrawSol instruction.
///
/// # Accounts (in order):
/// 0. `[writable]` stake_pool - The stake pool account
/// 1. `[]` withdraw_authority - Pool's withdraw authority PDA
/// 2. `[signer]` user_transfer_authority - Authority over the pool tokens
/// 3. `[writable]` burn_from - Source pool token account (vSOL to burn)
/// 4. `[writable]` reserve_stake - Pool's reserve stake account
/// 5. `[writable]` dest - Destination for lamports
/// 6. `[writable]` manager_fee_account - Pool's manager fee token account
/// 7. `[writable]` pool_mint - Pool token mint (vSOL mint)
/// 8. `[]` clock - Clock sysvar
/// 9. `[]` stake_history - Stake history sysvar
/// 10. `[]` stake_program - Stake program
/// 11. `[]` token_program
///
/// # Arguments
/// * `pool_tokens` - Amount of pool tokens (vSOL) to burn
/// * `signer_seeds` - Seeds for the PDA that owns the pool tokens
pub fn withdraw_sol(
    stake_pool: &AccountInfo,
    withdraw_authority: &AccountInfo,
    user_transfer_authority: &AccountInfo,
    burn_from: &AccountInfo,
    reserve_stake: &AccountInfo,
    dest: &AccountInfo,
    manager_fee_account: &AccountInfo,
    pool_mint: &AccountInfo,
    clock: &AccountInfo,
    stake_history: &AccountInfo,
    stake_program: &AccountInfo,
    token_program: &AccountInfo,
    pool_tokens: u64,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    // Build instruction data: discriminator (1 byte) + pool_tokens (8 bytes)
    let mut instruction_data = [0u8; 9];
    instruction_data[0] = instruction::WITHDRAW_SOL;
    instruction_data[1..9].copy_from_slice(&pool_tokens.to_le_bytes());

    // Build account metas using pinocchio API
    let account_metas = [
        AccountMeta::writable(stake_pool.key()),
        AccountMeta::readonly(withdraw_authority.key()),
        AccountMeta::readonly_signer(user_transfer_authority.key()),
        AccountMeta::writable(burn_from.key()),
        AccountMeta::writable(reserve_stake.key()),
        AccountMeta::writable(dest.key()),
        AccountMeta::writable(manager_fee_account.key()),
        AccountMeta::writable(pool_mint.key()),
        AccountMeta::readonly(clock.key()),
        AccountMeta::readonly(stake_history.key()),
        AccountMeta::readonly(stake_program.key()),
        AccountMeta::readonly(token_program.key()),
    ];

    let instruction = Instruction {
        program_id: &SPL_STAKE_POOL_PROGRAM_ID,
        accounts: &account_metas,
        data: &instruction_data,
    };

    // Convert signer_seeds to Seed types
    let seeds: [Seed; 3] = [
        Seed::from(signer_seeds[0]),
        Seed::from(signer_seeds[1]),
        Seed::from(signer_seeds[2]),
    ];
    let signer = [Signer::from(&seeds[..])];

    invoke_signed(
        &instruction,
        &[
            stake_pool,
            withdraw_authority,
            user_transfer_authority,
            burn_from,
            reserve_stake,
            dest,
            manager_fee_account,
            pool_mint,
            clock,
            stake_history,
            stake_program,
            token_program,
        ],
        &signer,
    )?;

    Ok(())
}

/// Calculate expected LST tokens for a given SOL deposit amount.
/// Uses the current exchange rate from the stake pool.
pub fn calculate_deposit_output(lamports: u64, pool_token_supply: u64, total_lamports: u64) -> u64 {
    if total_lamports == 0 || pool_token_supply == 0 {
        // Initial deposit: 1:1 ratio
        return lamports;
    }
    // output = lamports * pool_token_supply / total_lamports
    ((lamports as u128 * pool_token_supply as u128) / total_lamports as u128) as u64
}

 /// Calculate expected SOL for a given LST withdrawal amount.
/// Uses the current exchange rate from the stake pool.
pub fn calculate_withdrawal_output(
    pool_tokens: u64,
    pool_token_supply: u64,
    total_lamports: u64,
) -> u64 {
    if pool_token_supply == 0 {
        return 0;
    }
    // output = pool_tokens * total_lamports / pool_token_supply
    ((pool_tokens as u128 * total_lamports as u128) / pool_token_supply as u128) as u64
}
