//! `LiteSVM` setup and utilities

pub use litesvm::LiteSVM;
use solana_sdk::{account::Account, pubkey::Pubkey};

use super::constants::{PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_PROGRAM_ID};

/// Create a new `LiteSVM` instance with the validation-test program loaded
pub fn create_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();

    // Load the validation-test program
    let program_bytes = include_bytes!("../../../../target/deploy/validation_test.so");
    let _ = svm.add_program(PROGRAM_ID, program_bytes);

    svm
}

/// Airdrop SOL to an account
pub fn airdrop(svm: &mut LiteSVM, pubkey: &Pubkey, amount: u64) {
    svm.airdrop(pubkey, amount).unwrap();
}

/// Find PDA for test account: ["test", authority]
pub fn find_test_account_pda(authority: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"test", authority.as_ref()], &PROGRAM_ID)
}

// ============================================================================
// TestAccount helpers (for AccountLoader tests)
// ============================================================================

/// `TestAccount` size: 8 (discriminator) + 32 (authority) + 8 (value) + 1 (bump) + 7 (padding)
pub const TEST_ACCOUNT_SIZE: usize = 8 + 32 + 8 + 1 + 7;

/// `TestAccount` discriminator (`ValidationAccount::TestAccount` = 0)
pub const TEST_ACCOUNT_DISCRIMINATOR: u64 = 0;

/// Create a valid `TestAccount` in the SVM
///
/// This account has:
/// - Owner: validation-test program
/// - Discriminator: 0 (`TestAccount`)
/// - Size: `TEST_ACCOUNT_SIZE` bytes
pub fn create_valid_test_account(svm: &mut LiteSVM, pubkey: &Pubkey, authority: &Pubkey) {
    let mut data = vec![0u8; TEST_ACCOUNT_SIZE];

    // Set discriminator (8 bytes, little-endian)
    data[..8].copy_from_slice(&TEST_ACCOUNT_DISCRIMINATOR.to_le_bytes());

    // Set authority pubkey (32 bytes at offset 8)
    data[8..40].copy_from_slice(authority.as_ref());

    // value = 0, bump = 0, padding = 0 (already zero)

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*pubkey, account).unwrap();
}

/// Create a `TestAccount` with wrong owner (system program instead of validation program)
pub fn create_test_account_wrong_owner(svm: &mut LiteSVM, pubkey: &Pubkey) {
    let mut data = vec![0u8; TEST_ACCOUNT_SIZE];
    data[..8].copy_from_slice(&TEST_ACCOUNT_DISCRIMINATOR.to_le_bytes());

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: SYSTEM_PROGRAM_ID, // Wrong owner
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*pubkey, account).unwrap();
}

/// Create a `TestAccount` with wrong discriminator
pub fn create_test_account_wrong_discriminator(svm: &mut LiteSVM, pubkey: &Pubkey) {
    let mut data = vec![0u8; TEST_ACCOUNT_SIZE];

    // Set wrong discriminator (99 instead of 0)
    data[..8].copy_from_slice(&99u64.to_le_bytes());

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*pubkey, account).unwrap();
}

/// Create a `TestAccount` with wrong size (too small)
pub fn create_test_account_wrong_size(svm: &mut LiteSVM, pubkey: &Pubkey) {
    // Only 16 bytes instead of TEST_ACCOUNT_SIZE
    let mut data = vec![0u8; 16];
    data[..8].copy_from_slice(&TEST_ACCOUNT_DISCRIMINATOR.to_le_bytes());

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*pubkey, account).unwrap();
}

/// Create an uninitialized `TestAccount` (empty data)
pub fn create_test_account_uninitialized(svm: &mut LiteSVM, pubkey: &Pubkey) {
    let account = Account {
        lamports: 1_000_000_000,
        data: vec![], // Empty
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*pubkey, account).unwrap();
}

// ============================================================================
// Mint helpers (for LazyAccount<Mint> tests)
// ============================================================================

/// SPL Token Mint size
pub const MINT_SIZE: usize = 82;

/// Create a valid SPL Token Mint account in the SVM
///
/// This account has:
/// - Owner: Token Program
/// - Size: 82 bytes
pub fn create_valid_mint(svm: &mut LiteSVM, mint: &Pubkey, authority: &Pubkey, decimals: u8) {
    // Mint account data structure (82 bytes):
    // - 36 bytes: COption<Pubkey> mint_authority
    // - 8 bytes: supply (u64)
    // - 1 byte: decimals
    // - 1 byte: is_initialized
    // - 36 bytes: COption<Pubkey> freeze_authority
    let mut data = vec![0u8; MINT_SIZE];

    // mint_authority = Some(authority)
    data[0..4].copy_from_slice(&1u32.to_le_bytes()); // tag = Some
    data[4..36].copy_from_slice(authority.as_ref());

    // supply = 0 (bytes 36-43, already zero)

    // decimals
    data[44] = decimals;

    // is_initialized = true
    data[45] = 1;

    // freeze_authority = None (bytes 46-81, already zero)

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*mint, account).unwrap();
}

/// Create a Mint account with wrong owner (system program instead of token program)
pub fn create_mint_wrong_owner(svm: &mut LiteSVM, mint: &Pubkey) {
    let mut data = vec![0u8; MINT_SIZE];
    data[45] = 1; // is_initialized = true

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: SYSTEM_PROGRAM_ID, // Wrong owner
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*mint, account).unwrap();
}

/// Create a Mint account with wrong size (too small)
pub fn create_mint_wrong_size(svm: &mut LiteSVM, mint: &Pubkey) {
    // Only 40 bytes instead of 82
    let mut data = vec![0u8; 40];
    data[0..4].copy_from_slice(&1u32.to_le_bytes()); // tag = Some

    let account = Account {
        lamports: 1_000_000_000,
        data,
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*mint, account).unwrap();
}

/// Create an uninitialized Mint account (empty data)
pub fn create_mint_uninitialized(svm: &mut LiteSVM, mint: &Pubkey) {
    let account = Account {
        lamports: 1_000_000_000,
        data: vec![], // Empty
        owner: TOKEN_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(*mint, account).unwrap();
}
