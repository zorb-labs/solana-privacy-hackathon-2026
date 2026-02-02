//! Initialize LST (Liquid Staking Token) configuration.

use crate::{
    LST_VAULT_SEED, LstConfig, PoolType, UnifiedSolPoolConfig, UnifiedSolPoolError,
    find_lst_config_pda, find_lst_vault_pda,
};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{Seed, Signer as PinocchioSigner},
    pubkey::Pubkey,
    sysvars::{Sysvar, rent::Rent},
};
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::InitializeAccount3;

/// SPL Token account size
const TOKEN_ACCOUNT_SIZE: usize = 165;

/// Maximum number of LST configs supported.
/// This limit matches the fixed array size in advance_unified_epoch.
const MAX_LST_CONFIGS: u8 = 16;

/// SPL Token Program ID
const SPL_TOKEN_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

/// SPL Token-2022 Program ID
const SPL_TOKEN_2022_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");

/// SPL Stake Pool Program ID
/// This is the canonical SPL Stake Pool program that manages stake pools for most LSTs
const SPL_STAKE_POOL_PROGRAM_ID: Pubkey =
    pinocchio_pubkey::pubkey!("SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy");

/// Instruction data for InitLstConfig.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct InitLstConfigData {
    /// Pool type (0=Wsol, 1=SplStakePool, 2=Marinade, 3=Lido)
    pub pool_type: u8,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 7],
}

/// Accounts for the InitLstConfig instruction.
#[derive(Accounts)]
pub struct InitLstConfigAccounts<'info> {
    /// UnifiedSolPoolConfig PDA (for authority check and lst_count update)
    /// PDA derived from: ["unified_sol_pool"]
    #[account(mut, owner = crate::ID)]
    pub unified_sol_pool_config: AccountLoader<'info, UnifiedSolPoolConfig>,

    /// LstConfig PDA to create ["lst_config", lst_mint]
    #[account(init, payer = authority, pda = LstConfig, pda::lst_mint = lst_mint.key())]
    pub lst_config: AccountLoader<'info, LstConfig>,

    /// LST mint account (e.g., vSOL, jitoSOL)
    pub lst_mint: &'info AccountInfo,

    /// LST vault PDA to create ["lst_vault", lst_config]
    /// Note: Manually created as token account (owned by token program, not pool program)
    #[account(mut)]
    pub lst_vault: &'info AccountInfo,

    /// The stake pool for this LST (for WSOL, can be any account)
    pub stake_pool: &'info AccountInfo,

    /// The program that owns the stake pool
    pub stake_pool_program: &'info AccountInfo,

    /// Must match unified_sol_config.authority (pays for account creation)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// SPL Token program (required for CPI, validated to match lst_mint.owner())
    pub token_program: &'info AccountInfo,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Initialize LST configuration for a specific LST within the unified sol pool.
pub fn process_init_lst_config(
    ctx: Context<InitLstConfigAccounts>,
    data: InitLstConfigData,
) -> ProgramResult {
    let InitLstConfigAccounts {
        unified_sol_pool_config,
        lst_mint,
        lst_config,
        lst_vault,
        stake_pool,
        stake_pool_program,
        authority,
        token_program,
        system_program,
    } = ctx.accounts;

    // Validate system program
    if *system_program.key() != pinocchio_contrib::constants::SYSTEM_PROGRAM_ID {
        log!("init_lst_config: invalid system program");
        return Err(UnifiedSolPoolError::InvalidSystemProgram.into());
    }

    // Validate token program (SPL Token or Token-2022)
    let token_program_id = token_program.key();
    let is_token_program =
        *token_program_id == SPL_TOKEN_PROGRAM_ID || *token_program_id == SPL_TOKEN_2022_PROGRAM_ID;
    if !is_token_program {
        log!("init_lst_config: invalid token program");
        return Err(UnifiedSolPoolError::InvalidTokenProgram.into());
    }

    // Validate lst_mint is owned by token program
    if lst_mint.owner() != token_program_id {
        log!("init_lst_config: lst_mint not owned by token program");
        return Err(UnifiedSolPoolError::InvalidTokenProgram.into());
    }

    // Read and verify authority from unified config (releases borrow)
    let unified_authority = unified_sol_pool_config.map(|config| config.authority)?;

    if unified_authority != *authority.key() {
        log!("init_lst_config: unauthorized");
        return Err(UnifiedSolPoolError::Unauthorized.into());
    }

    // Parse pool type for validation
    let pool_type = PoolType::from_u8(data.pool_type).ok_or_else(|| {
        log!("init_lst_config: invalid pool_type");
        UnifiedSolPoolError::InvalidPoolType
    })?;

    // AUDIT TODO: LST Mint Validation
    // =============================================================================
    // MISSING CHECK: No validation that `lst_mint` is a valid SPL token mint.
    //
    // Current state: We only verify the token_program is SPL Token or Token-2022,
    // but we don't verify that `lst_mint` is actually owned by that program and
    // has valid mint data structure.
    //
    // Risk: An attacker could pass a non-mint account as lst_mint. The vault
    // creation would fail (InitializeAccount3 validates mint), but this is an
    // implicit rather than explicit check.
    //
    // Recommendation: Add explicit check:
    //   - Verify lst_mint.owner == token_program.key()
    //   - Optionally verify mint data structure (supply field, decimals, etc.)
    //
    // Severity: LOW - InitializeAccount3 provides implicit validation
    // =============================================================================

    // Validate stake pool configuration based on pool type
    // NOTE: Match arm order follows enum variant order (Wsol=0, SplStakePool=1, Marinade=2, Lido=3)
    match pool_type {
        PoolType::Wsol => {
            // WSOL doesn't need stake pool validation (rate is always 1:1)
        }
        PoolType::SplStakePool => {
            // Validate stake_pool_program is the canonical SPL Stake Pool program
            if *stake_pool_program.key() != SPL_STAKE_POOL_PROGRAM_ID {
                log!("init_lst_config: invalid stake pool program");
                return Err(UnifiedSolPoolError::InvalidStakePoolProgram.into());
            }

            // Validate stake_pool is owned by stake_pool_program
            if *stake_pool.owner() != SPL_STAKE_POOL_PROGRAM_ID {
                log!("init_lst_config: stake pool not owned by stake pool program");
                return Err(UnifiedSolPoolError::InvalidStakePool.into());
            }

            // Validate stake pool's pool_mint matches provided lst_mint
            // SPL Stake Pool layout:
            //   [0]:      account_type (1 byte)
            //   [1-33]:   manager (32 bytes)
            //   [33-65]:  staker (32 bytes)
            //   [65-66]:  stake_deposit_authority_bump (1 byte)
            //   [66-67]:  validator_list_bump (1 byte)
            //   [67-73]:  preferred_... (various)
            //   [73-105]: pool_mint (32 bytes)
            const POOL_MINT_OFFSET: usize = 73;
            const PUBKEY_SIZE: usize = 32;

            let stake_pool_data = stake_pool.try_borrow_data()?;
            if stake_pool_data.len() < POOL_MINT_OFFSET + PUBKEY_SIZE {
                log!("init_lst_config: stake pool data too short");
                return Err(UnifiedSolPoolError::InvalidStakePool.into());
            }

            let pool_mint_bytes =
                &stake_pool_data[POOL_MINT_OFFSET..POOL_MINT_OFFSET + PUBKEY_SIZE];

            if pool_mint_bytes != lst_mint.key().as_ref() {
                log!(
                    "init_lst_config: stake pool mint mismatch - pool has different mint"
                );
                return Err(UnifiedSolPoolError::StakePoolMintMismatch.into());
            }
        }
        PoolType::Marinade | PoolType::Lido => {
            // These pool types are not yet supported
            log!("init_lst_config: pool type not yet supported");
            return Err(UnifiedSolPoolError::InvalidPoolType.into());
        }
    }

    // Get PDA bumps for account creation
    // Note: lst_config PDA is created by panchor via init constraint
    let (expected_lst_config_pda, lst_config_bump) = find_lst_config_pda(lst_mint.key());
    let (expected_lst_vault_pda, lst_vault_bump) = find_lst_vault_pda(&expected_lst_config_pda);

    // Validate lst_vault PDA address
    if *lst_vault.key() != expected_lst_vault_pda {
        log!("init_lst_config: invalid lst_vault PDA");
        return Err(UnifiedSolPoolError::InvalidLstVaultPda.into());
    }

    // Get rent sysvar
    let rent = Rent::get()?;

    // Create LST vault token account PDA (manually, as it's owned by token program)
    let lst_vault_bump_bytes = [lst_vault_bump];
    let lst_vault_seeds = [
        Seed::from(LST_VAULT_SEED),
        Seed::from(expected_lst_config_pda.as_ref()),
        Seed::from(&lst_vault_bump_bytes[..]),
    ];
    let lst_vault_signer = PinocchioSigner::from(&lst_vault_seeds[..]);

    CreateAccount {
        from: authority,
        to: lst_vault,
        lamports: rent.minimum_balance(TOKEN_ACCOUNT_SIZE),
        space: TOKEN_ACCOUNT_SIZE as u64,
        owner: token_program_id,
    }
    .invoke_signed(&[lst_vault_signer])?;

    // Initialize LST vault token account with lst_config as owner
    InitializeAccount3 {
        account: lst_vault,
        mint: lst_mint,
        owner: &expected_lst_config_pda,
    }
    .invoke()?;

    // Initialize LST config data
    // Note: Account and discriminator already created by panchor's init constraint
    //
    // AUDIT TODO: Exchange Rate Invariant
    // =============================================================================
    // CRITICAL: All exchange rates MUST be >= RATE_PRECISION (1e9) for the
    // `virtual_sol_to_tokens` cast to be safe. Rates are initialized here to
    // exactly RATE_PRECISION. The only other place rates are modified is
    // `harvest_lst_appreciation`, which enforces `new_rate >= RATE_PRECISION`.
    //
    // If this invariant is violated, `virtual_sol_to_tokens` will silently
    // truncate the result, causing users to receive fewer tokens than expected.
    //
    // See: README.md "Audit TODOs" section for full analysis.
    // =============================================================================
    lst_config.inspect_mut(|config| {
        // === Header ===
        config.pool_type = data.pool_type;
        config.is_active = 1;
        config.bump = lst_config_bump;
        config._header_pad = [0u8; 5];

        // === Common References ===
        config.lst_mint = *lst_mint.key();
        config.lst_vault = expected_lst_vault_pda;

        // === Exchange Rate State ===
        config.exchange_rate = LstConfig::RATE_PRECISION; // Start at 1:1 (MUST be >= 1e9)
        config.harvested_exchange_rate = LstConfig::RATE_PRECISION; // Start at 1:1 (MUST be >= 1e9)
        config.last_rate_update_slot = 0;
        // AUDIT: EPOCH MODEL - last_harvest_epoch = 0 means "never harvested"
        // =====================================================================
        // Since reward_epoch starts at 1 (see init_unified_sol_pool_config.rs),
        // initializing to 0 ensures this LST cannot pass finalize_unified_rewards
        // (which checks last_harvest_epoch == reward_epoch) until it has been
        // harvested at least once via harvest_lst_appreciation.
        //
        // This prevents finalizing with dummy/uninitialized exchange rates.
        // After first harvest: last_harvest_epoch = current reward_epoch.
        //
        // See also:
        // - init_unified_sol_pool_config.rs: reward_epoch starts at 1
        // - finalize_unified_rewards.rs: last_harvest_epoch == reward_epoch check
        // - harvest_lst_appreciation.rs: sets last_harvest_epoch = current_epoch
        // =====================================================================
        config.last_harvest_epoch = 0;
        config._pad_for_u128 = 0;

        // === Value Tracking ===
        config.total_virtual_sol = 0;
        config.vault_token_balance = 0;
        config._value_pad = 0;

        // === Statistics ===
        config.total_deposited = 0;
        config.total_withdrawn = 0;
        config.total_appreciation_harvested = 0;
        config.deposit_count = 0;
        config.withdrawal_count = 0;
        config._stat_pad = 0;

        // === Stake Pool Specific ===
        config.stake_pool = *stake_pool.key();
        config.stake_pool_program = *stake_pool_program.key();
        config.previous_exchange_rate = LstConfig::RATE_PRECISION;

        // === Reserved ===
        config._reserved = 0;
    })?;

    // Increment LST count in unified config (check limit first)
    unified_sol_pool_config.try_inspect_mut(|unified| {
        if unified.lst_count >= MAX_LST_CONFIGS {
            log!("init_lst_config: max LST configs reached");
            return Err(UnifiedSolPoolError::MaxLstConfigsReached.into());
        }
        unified.lst_count = unified
            .lst_count
            .checked_add(1)
            .ok_or(UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    })?;

    log!("init_lst_config: LST config initialized successfully");

    Ok(())
}
