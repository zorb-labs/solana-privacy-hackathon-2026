//! Mock account creation helpers for testing.

use litesvm::LiteSVM;
use solana_account::Account;
use solana_pubkey::Pubkey;

use super::pda::SPL_TOKEN_PROGRAM_ID;

/// Associated Token Program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    solana_pubkey::pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");

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

/// Create a mock SPL Token account
pub fn create_mock_token_account(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    owner: &Pubkey,
    balance: u64,
) -> Pubkey {
    let token_account = Pubkey::new_unique();

    // SPL Token account layout (165 bytes)
    let mut data = vec![0u8; 165];
    // mint: 32 bytes at offset 0
    data[0..32].copy_from_slice(mint.as_ref());
    // owner: 32 bytes at offset 32
    data[32..64].copy_from_slice(owner.as_ref());
    // amount: 8 bytes at offset 64
    data[64..72].copy_from_slice(&balance.to_le_bytes());
    // delegate (Option<Pubkey>): 36 bytes at offset 72
    // state: 1 byte at offset 108 (AccountState::Initialized = 1)
    data[108] = 1;
    // is_native (Option<u64>): 12 bytes at offset 109
    // delegated_amount: 8 bytes at offset 121
    // close_authority (Option<Pubkey>): 36 bytes at offset 129

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: SPL_TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(token_account, account).unwrap();

    token_account
}

/// Create a mock SPL Stake Pool account with specified exchange rate
pub fn create_mock_stake_pool(
    svm: &mut LiteSVM,
    total_lamports: u64,
    pool_token_supply: u64,
    owner: Pubkey,
) -> Pubkey {
    let stake_pool = Pubkey::new_unique();

    // SPL Stake Pool layout (at least 275 bytes)
    // Offset 259-267: total_lamports (u64)
    // Offset 267-275: pool_token_supply (u64)
    let mut data = vec![0u8; 275];
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

/// Create a stake pool account with data shorter than required (< 275 bytes)
pub fn create_short_stake_pool(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let stake_pool = Pubkey::new_unique();
    let data = vec![0u8; 100]; // Too short - needs 275 bytes

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

/// Create a stake pool with zero pool token supply (causes division by zero)
pub fn create_stake_pool_zero_supply(svm: &mut LiteSVM, owner: Pubkey) -> Pubkey {
    let stake_pool = Pubkey::new_unique();
    let mut data = vec![0u8; 275];
    // total_lamports at offset 259 = 1000 SOL
    data[259..267].copy_from_slice(&1_000_000_000_000u64.to_le_bytes());
    // pool_token_supply at offset 267 = 0 (causes division by zero)
    data[267..275].copy_from_slice(&0u64.to_le_bytes());

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

/// TokenConfig field offsets (see TokenConfigAccount in state/token_config.rs)
pub mod token_config_offsets {
    pub const PENDING_DEPOSITS: usize = 152; // u128
    pub const PENDING_WITHDRAWALS: usize = 168; // u128
    pub const PENDING_REWARDS: usize = 184; // u64
    pub const TOTAL_DEPOSITED: usize = 192; // u128
    pub const TOTAL_WITHDRAWN: usize = 208; // u128
    pub const TOTAL_DEPOSIT_FEES: usize = 224; // u128
    pub const TOTAL_WITHDRAWAL_FEES: usize = 240; // u128
    pub const TOTAL_RELAYER_FEES: usize = 272; // u128
}

/// Read TokenConfig's pending_rewards field
pub fn get_token_config_pending_rewards(svm: &LiteSVM, token_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::PENDING_REWARDS;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read TokenConfig's total_deposit_fees field
pub fn get_token_config_total_deposit_fees(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::TOTAL_DEPOSIT_FEES;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's total_withdrawal_fees field
pub fn get_token_config_total_withdrawal_fees(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::TOTAL_WITHDRAWAL_FEES;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's pending_deposits field
pub fn get_token_config_pending_deposits(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::PENDING_DEPOSITS;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's pending_withdrawals field
pub fn get_token_config_pending_withdrawals(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::PENDING_WITHDRAWALS;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's total_relayer_fees field
pub fn get_token_config_total_relayer_fees(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::TOTAL_RELAYER_FEES;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Derive the ATA address for an owner and mint
pub fn get_associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[owner.as_ref(), SPL_TOKEN_PROGRAM_ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

/// Create an Associated Token Account (ATA) for testing
/// Returns the ATA address at the correct PDA
pub fn create_ata(
    svm: &mut LiteSVM,
    mint: &Pubkey,
    owner: &Pubkey,
    initial_balance: u64,
) -> Pubkey {
    let ata = get_associated_token_address(owner, mint);

    // SPL Token account layout (165 bytes)
    let mut data = vec![0u8; 165];
    // mint: 32 bytes at offset 0
    data[0..32].copy_from_slice(mint.as_ref());
    // owner: 32 bytes at offset 32
    data[32..64].copy_from_slice(owner.as_ref());
    // amount: 8 bytes at offset 64
    data[64..72].copy_from_slice(&initial_balance.to_le_bytes());
    // delegate (Option<Pubkey>): 36 bytes at offset 72
    // state: 1 byte at offset 108 (AccountState::Initialized = 1)
    data[108] = 1;
    // is_native (Option<u64>): 12 bytes at offset 109
    // delegated_amount: 8 bytes at offset 121
    // close_authority (Option<Pubkey>): 36 bytes at offset 129

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: SPL_TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(ata, account).unwrap();

    ata
}
