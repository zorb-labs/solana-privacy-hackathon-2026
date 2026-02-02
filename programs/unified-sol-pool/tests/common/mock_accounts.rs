//! Mock account creation helpers for unified-sol-pool tests.

use litesvm::LiteSVM;
use solana_sdk::account::Account;
use solana_sdk::pubkey::Pubkey;

use super::pda::SPL_TOKEN_PROGRAM_ID;

/// Create a mock SPL Token mint account
pub fn create_mock_mint(svm: &mut LiteSVM, decimals: u8) -> Pubkey {
    let mint = Pubkey::new_unique();

    // SPL Token mint layout (82 bytes)
    let mut data = vec![0u8; 82];
    // mint_authority (Option<Pubkey>): 36 bytes (4 + 32)
    data[0] = 1; // Some
    // supply: 8 bytes at offset 36
    // decimals: 1 byte at offset 44
    data[44] = decimals;
    // is_initialized: 1 byte at offset 45
    data[45] = 1;
    // freeze_authority (Option<Pubkey>): 36 bytes at offset 46

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: SPL_TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(mint, account).unwrap();

    mint
}

/// Create an "invalid" mint - account not owned by SPL Token program
pub fn create_invalid_mint(svm: &mut LiteSVM, decimals: u8) -> Pubkey {
    let mint = Pubkey::new_unique();

    // Same data as a real mint, but owned by system program
    let mut data = vec![0u8; 82];
    data[0] = 1; // Some
    data[44] = decimals;
    data[45] = 1;

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: solana_sdk::system_program::ID, // Wrong owner!
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(mint, account).unwrap();

    mint
}

/// Create a mock SPL Stake Pool account with specified exchange rate
pub fn create_mock_stake_pool(
    svm: &mut LiteSVM,
    pool_mint: &Pubkey,
    total_lamports: u64,
    pool_token_supply: u64,
    owner: Pubkey,
) -> Pubkey {
    let stake_pool = Pubkey::new_unique();

    // SPL Stake Pool layout (at least 283 bytes for harvest_lst_appreciation)
    // Offset 73-105: pool_mint (32 bytes)
    // Offset 259-267: total_lamports (u64)
    // Offset 267-275: pool_token_supply (u64)
    // Offset 275-283: last_update_epoch (u64) - required by harvest
    let mut data = vec![0u8; 283];
    data[73..105].copy_from_slice(pool_mint.as_ref());
    data[259..267].copy_from_slice(&total_lamports.to_le_bytes());
    data[267..275].copy_from_slice(&pool_token_supply.to_le_bytes());

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(stake_pool, account).unwrap();

    stake_pool
}

/// Update a token account's balance
pub fn update_vault_balance(svm: &mut LiteSVM, vault: &Pubkey, new_balance: u64) {
    let account = svm.get_account(vault).expect("vault should exist");
    let mut data = account.data.clone();
    data[64..72].copy_from_slice(&new_balance.to_le_bytes());
    let updated = Account {
        lamports: account.lamports,
        data,
        owner: account.owner,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*vault, updated).unwrap();
}

/// Update LstConfig's vault_token_balance counter to match actual vault balance.
/// This must be called alongside update_vault_balance to keep the program's
/// invariant check happy (counter must match actual balance).
pub fn update_lst_config_vault_balance(svm: &mut LiteSVM, lst_config: &Pubkey, new_balance: u64) {
    use super::lst_config_offsets::VAULT_TOKEN_BALANCE;
    let account = svm.get_account(lst_config).expect("lst_config should exist");
    let mut data = account.data.clone();
    data[VAULT_TOKEN_BALANCE..VAULT_TOKEN_BALANCE + 8]
        .copy_from_slice(&new_balance.to_le_bytes());
    let updated = Account {
        lamports: account.lamports,
        data,
        owner: account.owner,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*lst_config, updated).unwrap();
}

/// Update stake pool exchange rate (simulate appreciation)
pub fn update_stake_pool_rate(
    svm: &mut LiteSVM,
    stake_pool: &Pubkey,
    new_total_lamports: u64,
    pool_token_supply: u64,
) {
    let account = svm
        .get_account(stake_pool)
        .expect("stake_pool should exist");
    let mut data = account.data.clone();
    // total_lamports at offset 259-267
    data[259..267].copy_from_slice(&new_total_lamports.to_le_bytes());
    // pool_token_supply at offset 267-275
    data[267..275].copy_from_slice(&pool_token_supply.to_le_bytes());
    let updated = Account {
        lamports: account.lamports,
        data,
        owner: account.owner,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*stake_pool, updated).unwrap();
}

/// Read a token account's balance
pub fn get_token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_account)
        .expect("token_account should exist");
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

// ============================================================================
// UnifiedSolPoolConfig Reading Helpers
// ============================================================================

/// UnifiedSolPoolConfig field offsets (from unified-sol-pool/src/state.rs)
pub mod unified_config_offsets {
    pub const REWARD_ACCUMULATOR: usize = 184; // u128
    pub const PENDING_APPRECIATION: usize = 224; // u64
    pub const FINALIZED_BALANCE: usize = 232; // u128
    pub const PENDING_DEPOSITS: usize = 248; // u128
    pub const PENDING_WITHDRAWALS: usize = 264; // u128
    pub const TOTAL_REWARDS_DISTRIBUTED: usize = 344; // u128
    pub const TOTAL_APPRECIATION: usize = 408; // u128
}

/// LstConfig field offsets (with 8-byte panchor discriminator)
///
/// Calculated from struct layout:
/// - discriminator: 0-7 (8 bytes)
/// - pool_type: 8 (1 byte), is_active: 9 (1 byte), bump: 10 (1 byte), _header_pad: 11-15 (5 bytes)
/// - lst_mint: 16-47 (32 bytes)
/// - lst_vault: 48-79 (32 bytes)
/// - exchange_rate: 80-87 (8 bytes)
/// - harvested_exchange_rate: 88-95 (8 bytes)
/// - last_rate_update_slot: 96-103 (8 bytes)
/// - last_harvest_epoch: 104-111 (8 bytes)
/// - _pad_for_u128: 112-119 (8 bytes)
/// - total_virtual_sol: 120-135 (16 bytes, u128)
/// - vault_token_balance: 136-143 (8 bytes)
/// - _value_pad: 144-151 (8 bytes)
/// - total_deposited: 152-167 (16 bytes, u128)
/// - total_withdrawn: 168-183 (16 bytes, u128)
/// - total_appreciation_harvested: 184-191 (8 bytes)
/// - deposit_count: 192-199 (8 bytes)
/// - withdrawal_count: 200-207 (8 bytes)
/// - _stat_pad: 208-215 (8 bytes)
/// - stake_pool: 216-247 (32 bytes)
/// - stake_pool_program: 248-279 (32 bytes)
/// - previous_exchange_rate: 280-287 (8 bytes)
/// - _reserved: 288-295 (8 bytes)
pub mod lst_config_offsets {
    pub const EXCHANGE_RATE: usize = 80; // u64
    pub const HARVESTED_EXCHANGE_RATE: usize = 88; // u64
    pub const LAST_HARVEST_EPOCH: usize = 104; // u64
    pub const VAULT_TOKEN_BALANCE: usize = 136; // u64
    pub const TOTAL_APPRECIATION_HARVESTED: usize = 184; // u64
    pub const PREVIOUS_EXCHANGE_RATE: usize = 280; // u64
}

/// Rate precision constant (1e9) for exchange rate calculations
pub const RATE_PRECISION: u64 = 1_000_000_000;

/// Compute virtual SOL value from vault balance and exchange rate.
/// Formula: virtual_sol_value = vault_balance * exchange_rate / 1e9
pub fn compute_virtual_sol_value(vault_balance: u64, exchange_rate: u64) -> u128 {
    (vault_balance as u128 * exchange_rate as u128) / RATE_PRECISION as u128
}

/// Read UnifiedSolConfig's reward_epoch field
pub fn get_unified_config_reward_epoch(svm: &LiteSVM, unified_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    // reward_epoch at offset: discriminator(8) + asset_id(32) + authority(32) + pending_authority(32) = 104
    let offset = 104;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read UnifiedSolConfig's reward_accumulator field
pub fn get_unified_config_reward_accumulator(svm: &LiteSVM, unified_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::REWARD_ACCUMULATOR;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read UnifiedSolConfig's pending_appreciation field
pub fn get_unified_config_pending_appreciation(svm: &LiteSVM, unified_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::PENDING_APPRECIATION;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read UnifiedSolConfig's finalized_balance field
pub fn get_unified_config_finalized_balance(svm: &LiteSVM, unified_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::FINALIZED_BALANCE;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read UnifiedSolConfig's pending_deposits field
pub fn get_unified_config_pending_deposits(svm: &LiteSVM, unified_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::PENDING_DEPOSITS;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read UnifiedSolConfig's pending_withdrawals field
pub fn get_unified_config_pending_withdrawals(svm: &LiteSVM, unified_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::PENDING_WITHDRAWALS;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read UnifiedSolConfig's total_appreciation field
pub fn get_unified_config_total_appreciation(svm: &LiteSVM, unified_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::TOTAL_APPRECIATION;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read UnifiedSolConfig's total_rewards_distributed field
pub fn get_unified_config_total_rewards_distributed(svm: &LiteSVM, unified_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(unified_config)
        .expect("unified_config should exist");
    let offset = unified_config_offsets::TOTAL_REWARDS_DISTRIBUTED;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

// ============================================================================
// LstConfig Reading Helpers
// ============================================================================

/// Read LstConfig's exchange_rate field
pub fn get_lst_config_exchange_rate(svm: &LiteSVM, lst_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(lst_config)
        .expect("lst_config should exist");
    let offset = lst_config_offsets::EXCHANGE_RATE;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read LstConfig's previous_exchange_rate field
pub fn get_lst_config_previous_exchange_rate(svm: &LiteSVM, lst_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(lst_config)
        .expect("lst_config should exist");
    let offset = lst_config_offsets::PREVIOUS_EXCHANGE_RATE;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read LstConfig's harvested_exchange_rate field
pub fn get_lst_config_harvested_exchange_rate(svm: &LiteSVM, lst_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(lst_config)
        .expect("lst_config should exist");
    let offset = lst_config_offsets::HARVESTED_EXCHANGE_RATE;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read LstConfig's last_harvest_epoch field
pub fn get_lst_config_last_harvest_epoch(svm: &LiteSVM, lst_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(lst_config)
        .expect("lst_config should exist");
    let offset = lst_config_offsets::LAST_HARVEST_EPOCH;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read LstConfig's total_appreciation_harvested field
pub fn get_lst_config_total_appreciation_harvested(svm: &LiteSVM, lst_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(lst_config)
        .expect("lst_config should exist");
    let offset = lst_config_offsets::TOTAL_APPRECIATION_HARVESTED;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}
