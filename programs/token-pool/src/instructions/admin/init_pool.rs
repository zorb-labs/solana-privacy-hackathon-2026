//! Initialize token pool configuration.

use crate::{
    TokenPoolConfig, TokenPoolError, VAULT_SEED, find_token_pool_config_pda, find_vault_pda,
};
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, instruction::Seed, pubkey::Pubkey};
use pinocchio_log::log;
use pinocchio_token::{instructions::InitializeAccount3, state::Mint};
use solana_poseidon::{Endianness, Parameters, hashv};
use zorb_pool_interface::BASIS_POINTS;

/// SPL Token account size
const TOKEN_ACCOUNT_SIZE: usize = 165;

/// SPL Token Program ID
const SPL_TOKEN_PROGRAM_ID: Pubkey = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];

/// SPL Token-2022 Program ID
const SPL_TOKEN_2022_PROGRAM_ID: Pubkey = [
    0x06, 0xa7, 0xd5, 0x17, 0x18, 0x7b, 0xd1, 0x65, 0x35, 0x50, 0xc4, 0x9a, 0x3a, 0x8b, 0x9a, 0x28,
    0xb9, 0x51, 0x9f, 0x60, 0x7d, 0x1f, 0x55, 0xb8, 0x26, 0xb4, 0x53, 0x06, 0x76, 0x8b, 0x9f, 0x71,
];

/// Instruction data for InitPool.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct InitPoolData {
    /// Maximum tokens allowed per deposit transaction
    pub max_deposit_amount: u64,
    /// Fee rate for deposits in basis points (100 = 1%)
    pub deposit_fee_rate: u16,
    /// Fee rate for withdrawals in basis points (100 = 1%)
    pub withdrawal_fee_rate: u16,
    /// Padding for 8-byte alignment
    pub _padding: [u8; 4],
}

/// Accounts for the InitPool instruction.
#[derive(Accounts)]
pub struct InitPoolAccounts<'info> {
    /// SPL Token mint to register
    pub mint_account: &'info AccountInfo,

    /// Pool config PDA ["token_pool", mint] to create
    #[account(init, payer = authority, pda = TokenPoolConfig, pda::mint = mint_account.key())]
    pub pool_config: AccountLoader<'info, TokenPoolConfig>,

    /// Vault PDA ["vault", pool_config] to create
    /// Note: Manually created as token account (owned by token program, not pool program)
    #[account(mut)]
    pub vault: &'info AccountInfo,

    /// Authority for this pool (pays for account creation)
    #[account(mut)]
    pub authority: Signer<'info>,

    /// SPL Token program (required for CPI, validated to match mint.owner())
    pub token_program: &'info AccountInfo,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Initialize a new token pool.
///
/// Creates a TokenPoolConfig PDA and the vault token account.
/// Vault is created at PDA address derived from pool_config:
/// - vault = PDA ["vault", pool_config] - owned by pool_config PDA
pub fn process_init_pool(ctx: Context<InitPoolAccounts>, data: InitPoolData) -> ProgramResult {
    let InitPoolAccounts {
        mint_account,
        pool_config,
        vault,
        authority,
        token_program,
        system_program,
    } = ctx.accounts;

    // Validate system program
    if *system_program.key() != pinocchio_contrib::constants::SYSTEM_PROGRAM_ID {
        log!("init_pool: invalid system program");
        return Err(TokenPoolError::InvalidSystemProgram.into());
    }

    // Validate token program (SPL Token or Token-2022)
    let token_program_id = token_program.key();
    let is_token_program =
        *token_program_id == SPL_TOKEN_PROGRAM_ID || *token_program_id == SPL_TOKEN_2022_PROGRAM_ID;
    if !is_token_program {
        log!("init_pool: invalid token program");
        return Err(TokenPoolError::InvalidTokenProgram.into());
    }

    // Validate mint is owned by token program
    if mint_account.owner() != token_program_id {
        log!("init_pool: mint not owned by token program");
        return Err(TokenPoolError::InvalidMint.into());
    }

    // Read decimals from mint account using pinocchio_token typed access
    let decimals = Mint::from_account_info(mint_account)
        .map_err(|_| TokenPoolError::InvalidMint)?
        .decimals();

    // Validate fee rates (max 100%)
    if data.deposit_fee_rate > BASIS_POINTS as u16 || data.withdrawal_fee_rate > BASIS_POINTS as u16
    {
        log!("init_pool: fee rate exceeds 100%");
        return Err(TokenPoolError::InvalidFeeRate.into());
    }

    // Get PDA bumps for account creation
    // Note: pool_config PDA is created by panchor via init constraint
    let (expected_config_pda, config_bump) = find_token_pool_config_pda(mint_account.key());
    let (expected_vault_pda, vault_bump) = find_vault_pda(&expected_config_pda);

    // Validate vault PDA address
    if *vault.key() != expected_vault_pda {
        log!("init_pool: invalid vault PDA");
        return Err(TokenPoolError::InvalidVaultPda.into());
    }

    // Create vault token account PDA (owned by token program, not pool program)
    let vault_bump_bytes = [vault_bump];
    let vault_seeds = [
        Seed::from(VAULT_SEED),
        Seed::from(expected_config_pda.as_ref()),
        Seed::from(&vault_bump_bytes),
    ];

    vault.create_pda_account_with_space(
        authority,
        &vault_seeds,
        system_program.account_info(),
        TOKEN_ACCOUNT_SIZE,
        token_program_id,
    )?;

    // Initialize vault as token account with pool_config as owner
    InitializeAccount3 {
        account: vault,
        mint: mint_account,
        owner: &expected_config_pda,
    }
    .invoke()?;

    // Compute asset_id from mint using Poseidon hash
    let asset_id = compute_asset_id(mint_account.key());

    // Initialize pool config data
    // Note: Account and discriminator already created by panchor's init constraint
    pool_config.inspect_mut(|config| {
        config.authority = *authority.key();
        config.pending_authority = [0u8; 32];
        config.mint = *mint_account.key();
        config.vault = expected_vault_pda;
        config.asset_id = asset_id;
        config.finalized_balance = 0;
        config.pending_deposits = 0;
        config.pending_withdrawals = 0;
        config.pending_deposit_fees = 0;
        config.pending_withdrawal_fees = 0;
        config.pending_funded_rewards = 0;
        config._pad_fees = 0;
        config.total_deposited = 0;
        config.total_withdrawn = 0;
        config.total_rewards_distributed = 0;
        config.total_deposit_fees = 0;
        config.total_withdrawal_fees = 0;
        config.total_funded_rewards = 0;
        config._reserved_stats = 0;
        config.max_deposit_amount = data.max_deposit_amount;
        config.deposit_count = 0;
        config.withdrawal_count = 0;
        config.reward_accumulator = 0;
        config.last_finalized_slot = 0;
        config.deposit_fee_rate = data.deposit_fee_rate;
        config.withdrawal_fee_rate = data.withdrawal_fee_rate;
        config.decimals = decimals;
        config.is_active = 1;
        config.bump = config_bump;
        config._padding = [0u8; 9];
    })?;

    log!("init_pool: pool initialized successfully");

    Ok(())
}

/// Compute asset_id from mint address using Poseidon hash.
///
/// The mint address (32 bytes, little-endian) is split into two 128-bit limbs
/// and hashed together. This matches the circuit's representation where a
/// 256-bit value is split into two field elements.
///
/// Layout (little-endian source -> big-endian Poseidon input):
/// - Low limb:  mint[0..16]  (LE) -> reversed to BE, zero-padded to 32 bytes
/// - High limb: mint[16..32] (LE) -> reversed to BE, zero-padded to 32 bytes
///
/// Uses BN254 curve with X5 parameters and big-endian encoding.
fn compute_asset_id(mint: &Pubkey) -> [u8; 32] {
    // Split mint into two 128-bit limbs (little-endian source)
    // Low limb: bytes 0-15, High limb: bytes 16-31
    let mint_bytes = mint;
    let mut low_limb = [0u8; 32];
    let mut high_limb = [0u8; 32];

    // Convert each 16-byte little-endian chunk to 32-byte big-endian field element
    // Reverse bytes and place in low 16 bytes of the 32-byte array
    for i in 0..16 {
        low_limb[31 - i] = mint_bytes[i]; // Reverse bytes[0..16] -> positions [16..32]
        high_limb[31 - i] = mint_bytes[16 + i]; // Reverse bytes[16..32] -> positions [16..32]
    }

    // Hash both limbs: Poseidon(low_limb, high_limb)
    let hash_result = hashv(
        Parameters::Bn254X5,
        Endianness::BigEndian,
        &[&low_limb, &high_limb],
    )
    .expect("Poseidon hash should succeed");
    hash_result.to_bytes()
}
