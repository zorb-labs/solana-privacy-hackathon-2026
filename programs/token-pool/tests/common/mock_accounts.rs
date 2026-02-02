//! Mock account creation helpers for token-pool tests.

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

/// Read a token account's balance
pub fn get_token_balance(svm: &LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_account)
        .expect("token_account should exist");
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

// ============================================================================
// TokenPoolConfig Reading Helpers
// ============================================================================

/// Panchor account discriminator size (8 bytes)
pub const DISC_SIZE: usize = 8;

/// TokenConfig field offsets (see TokenPoolConfig in token-pool crate)
pub mod token_config_offsets {
    /// Offset to asset_id field: discriminator(8) + authority(32) + pending_authority(32) + mint(32) + vault(32) = 136
    pub const ASSET_ID: usize = 136;
    pub const FINALIZED_BALANCE: usize = 168; // u128
    pub const REWARD_ACCUMULATOR: usize = 184; // u128
    pub const PENDING_DEPOSITS: usize = 200; // u128
    pub const PENDING_WITHDRAWALS: usize = 216; // u128
    pub const PENDING_DEPOSIT_FEES: usize = 232; // u64
    pub const PENDING_WITHDRAWAL_FEES: usize = 240; // u64
    pub const PENDING_FUNDED_REWARDS: usize = 248; // u64
    pub const TOTAL_DEPOSITED: usize = 264; // u128
    pub const TOTAL_WITHDRAWN: usize = 280; // u128
    pub const TOTAL_REWARDS_DISTRIBUTED: usize = 296; // u128
    pub const TOTAL_DEPOSIT_FEES: usize = 312; // u128
    pub const TOTAL_WITHDRAWAL_FEES: usize = 328; // u128
    pub const TOTAL_FUNDED_REWARDS: usize = 344; // u128
    pub const MAX_DEPOSIT_AMOUNT: usize = 376; // u64
    pub const DEPOSIT_COUNT: usize = 384; // u64
    pub const WITHDRAWAL_COUNT: usize = 392; // u64
    pub const LAST_FINALIZED_SLOT: usize = 400; // u64
}

/// Read TokenConfig's asset_id field
pub fn get_token_pool_config_asset_id(svm: &LiteSVM, token_config: &Pubkey) -> [u8; 32] {
    let account = svm
        .get_account(token_config)
        .expect("token_pool_config should exist");
    let offset = token_config_offsets::ASSET_ID;
    let mut asset_id = [0u8; 32];
    asset_id.copy_from_slice(&account.data[offset..offset + 32]);
    asset_id
}

/// Read TokenConfig's pending_deposit_fees field
pub fn get_token_config_pending_deposit_fees(svm: &LiteSVM, token_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::PENDING_DEPOSIT_FEES;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read TokenConfig's pending_withdrawal_fees field
pub fn get_token_config_pending_withdrawal_fees(svm: &LiteSVM, token_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::PENDING_WITHDRAWAL_FEES;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read TokenConfig's pending_funded_rewards field
pub fn get_token_config_pending_funded_rewards(svm: &LiteSVM, token_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::PENDING_FUNDED_REWARDS;
    u64::from_le_bytes(account.data[offset..offset + 8].try_into().unwrap())
}

/// Read TokenConfig's reward_accumulator field
pub fn get_token_config_reward_accumulator(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::REWARD_ACCUMULATOR;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's finalized_balance field
pub fn get_token_config_finalized_balance(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::FINALIZED_BALANCE;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's last_finalized_slot field
pub fn get_token_config_last_finalized_slot(svm: &LiteSVM, token_config: &Pubkey) -> u64 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::LAST_FINALIZED_SLOT;
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

/// Read TokenConfig's total_funded_rewards field
pub fn get_token_config_total_funded_rewards(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::TOTAL_FUNDED_REWARDS;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Read TokenConfig's total_rewards_distributed field
pub fn get_token_config_total_rewards_distributed(svm: &LiteSVM, token_config: &Pubkey) -> u128 {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let offset = token_config_offsets::TOTAL_REWARDS_DISTRIBUTED;
    u128::from_le_bytes(account.data[offset..offset + 16].try_into().unwrap())
}

/// Set TokenConfig's finalized_balance field (for testing purposes)
pub fn set_token_config_finalized_balance(
    svm: &mut LiteSVM,
    token_config: &Pubkey,
    balance: u128,
) {
    let account = svm
        .get_account(token_config)
        .expect("token_config should exist");
    let mut data = account.data.clone();
    let offset = token_config_offsets::FINALIZED_BALANCE;
    data[offset..offset + 16].copy_from_slice(&balance.to_le_bytes());
    let updated = Account {
        lamports: account.lamports,
        data,
        owner: account.owner,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(*token_config, updated).unwrap();
}
