//! Tests for account validation constraints

use crate::helpers::*;
use solana_sdk::{
    instruction::InstructionError,
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};

/// Helper to extract `InstructionError` from transaction failure
#[track_caller]
fn expect_instruction_error(
    result: Result<litesvm::types::TransactionMetadata, litesvm::types::FailedTransactionMetadata>,
    expected: &InstructionError,
) {
    let err = result.expect_err("Expected transaction to fail");
    match err.err {
        TransactionError::InstructionError(_, actual) => {
            assert_eq!(&actual, expected, "Expected {expected:?}, got {actual:?}");
        }
        other => panic!("Expected InstructionError, got {other:?}"),
    }
}

/// Test #[account(signer)] constraint - valid signer
#[test]
fn test_signer_valid() {
    let mut svm = create_svm();

    let authority = Keypair::new();
    airdrop(&mut svm, &authority.pubkey(), 10 * SOL);

    let ix = test_signer(&authority.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid signer should succeed: {:?}",
        result.err()
    );
}

/// Test #[account(signer)] constraint - missing signature
#[test]
fn test_signer_missing() {
    let mut svm = create_svm();

    let authority = Keypair::new();
    let other = Keypair::new();
    airdrop(&mut svm, &other.pubkey(), 10 * SOL);

    // Use invalid instruction that marks account as non-signer
    let ix = test_signer_invalid(&authority.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&other.pubkey()),
        &[&other],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::MissingRequiredSignature);
}

/// Test #[account(mut)] constraint - valid writable
#[test]
fn test_mutable_valid() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let target = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);
    airdrop(&mut svm, &target.pubkey(), 1 * SOL);

    let ix = test_mutable(&target.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid writable should succeed: {:?}",
        result.err()
    );
}

/// Test #[account(mut)] constraint - account not writable
#[test]
fn test_mutable_readonly() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let target = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);
    airdrop(&mut svm, &target.pubkey(), 1 * SOL);

    // Use invalid instruction that marks account as readonly
    let ix = test_mutable_invalid(&target.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::Immutable);
}

/// Test Program<T> constraint - valid system program
#[test]
fn test_program_valid() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    let ix = test_program(&SYSTEM_PROGRAM_ID);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid program should succeed: {:?}",
        result.err()
    );
}

/// Test Program<T> constraint - wrong program ID (non-executable account)
#[test]
fn test_program_wrong_id() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let wrong_program = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Pass a non-program account instead of system program
    // Key check fails first, so we get IncorrectProgramId
    let ix = test_program(&wrong_program.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::IncorrectProgramId);
}

/// Test Program<T> constraint - wrong program ID (but IS executable)
#[test]
fn test_program_wrong_executable() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Pass the validation-test program itself - it's executable but wrong ID
    // Should fail with IncorrectProgramId since we expect System Program
    let ix = test_program(&PROGRAM_ID);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::IncorrectProgramId);
}

/// Test #[account(address = expr)] constraint - valid address
#[test]
fn test_address_valid() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // The test expects system program ID
    let ix = test_address(&SYSTEM_PROGRAM_ID);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid address should succeed: {:?}",
        result.err()
    );
}

/// Test #[account(address = expr)] constraint - wrong address
#[test]
fn test_address_wrong() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let wrong = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Pass wrong address
    let ix = test_address(&wrong.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::InvalidAccountData);
}

/// Test #[account(init, seeds = [...], payer = ...)] constraint
#[test]
fn test_init_creates_account() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    let (test_account, _) = find_test_account_pda(&payer.pubkey());

    // Verify account doesn't exist
    assert!(
        svm.get_account(&test_account).is_none(),
        "Account should not exist before init"
    );

    let ix = test_init(&payer.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(result.is_ok(), "Init should succeed: {:?}", result.err());

    // Verify account was created
    let account = svm.get_account(&test_account);
    assert!(account.is_some(), "Account should exist after init");
}

/// Test init with already initialized account
#[test]
fn test_init_already_initialized() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // First init should succeed
    let ix1 = test_init(&payer.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix1],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).expect("First init should succeed");

    // Expire recent blockhash to get a fresh one
    svm.expire_blockhash();

    // Second init should fail with AccountAlreadyInitialized
    let ix2 = test_init(&payer.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix2],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );
    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::AccountAlreadyInitialized);
}

/// Test #[account(owner = expr)] constraint - valid owner (system program)
#[test]
fn test_owner_constraint_valid() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // payer is owned by system program, so this should succeed
    let ix = test_owner_constraint(&payer.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid owner should succeed: {:?}",
        result.err()
    );
}

/// Test #[account(owner = expr)] constraint - wrong owner
#[test]
fn test_owner_constraint_wrong() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Use the test program itself as the target - it's owned by BPF Loader, not system program
    let ix = test_owner_constraint(&PROGRAM_ID);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::IllegalOwner);
}

/// Test Signer<'info> wrapper - valid signer
#[test]
fn test_signer_wrapper_valid() {
    let mut svm = create_svm();

    let authority = Keypair::new();
    airdrop(&mut svm, &authority.pubkey(), 10 * SOL);

    let ix = test_signer_wrapper(&authority.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[&authority],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid signer wrapper should succeed: {:?}",
        result.err()
    );
}

/// Test Signer<'info> wrapper - missing signer
#[test]
fn test_signer_wrapper_missing() {
    let mut svm = create_svm();

    let authority = Keypair::new();
    let other = Keypair::new();
    airdrop(&mut svm, &other.pubkey(), 10 * SOL);

    // Use invalid instruction that marks account as non-signer
    let ix = test_signer_wrapper_invalid(&authority.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&other.pubkey()),
        &[&other],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::MissingRequiredSignature);
}

// ============================================================================
// AccountLoader<T> tests (test_owner instruction)
// Tests owner, discriminator, and size validation
// ============================================================================

/// Test AccountLoader<T> - valid account with correct owner, discriminator, size
#[test]
fn test_account_loader_valid() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let test_account = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create a valid test account
    create_valid_test_account(&mut svm, &test_account.pubkey(), &payer.pubkey());

    let ix = test_owner(&test_account.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid AccountLoader should succeed: {:?}",
        result.err()
    );
}

/// Test AccountLoader<T> - wrong owner (system program instead of validation program)
#[test]
fn test_account_loader_wrong_owner() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let test_account = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create account with wrong owner
    create_test_account_wrong_owner(&mut svm, &test_account.pubkey());

    let ix = test_owner(&test_account.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // AccountLoader uses AccountOwnerValidate which returns IllegalOwner
    expect_instruction_error(result, &InstructionError::IllegalOwner);
}

/// Test AccountLoader<T> - wrong discriminator
#[test]
fn test_account_loader_wrong_discriminator() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let test_account = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create account with wrong discriminator
    create_test_account_wrong_discriminator(&mut svm, &test_account.pubkey());

    let ix = test_owner(&test_account.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::InvalidAccountData);
}

/// Test AccountLoader<T> - wrong size (too small)
#[test]
fn test_account_loader_wrong_size() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let test_account = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create account with wrong size
    create_test_account_wrong_size(&mut svm, &test_account.pubkey());

    let ix = test_owner(&test_account.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::AccountDataTooSmall);
}

/// Test AccountLoader<T> - uninitialized (empty data)
#[test]
fn test_account_loader_uninitialized() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let test_account = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create uninitialized account
    create_test_account_uninitialized(&mut svm, &test_account.pubkey());

    let ix = test_owner(&test_account.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // Uninitialized account has zero-length data, which is too small
    expect_instruction_error(result, &InstructionError::AccountDataTooSmall);
}

// ============================================================================
// LazyAccount<'info, Mint> tests (test_lazy_mint instruction)
// Tests Token Program owner and 82-byte size validation
// ============================================================================

/// Test LazyAccount<Mint> - valid mint with correct owner (Token Program) and size (82 bytes)
#[test]
fn test_lazy_mint_valid() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let mint = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create a valid mint account
    create_valid_mint(&mut svm, &mint.pubkey(), &payer.pubkey(), 6);

    let ix = test_lazy_mint(&mint.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    assert!(
        result.is_ok(),
        "Valid LazyAccount<Mint> should succeed: {:?}",
        result.err()
    );
}

/// Test LazyAccount<Mint> - wrong owner (system program instead of Token Program)
#[test]
fn test_lazy_mint_wrong_owner() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let mint = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create mint with wrong owner
    create_mint_wrong_owner(&mut svm, &mint.pubkey());

    let ix = test_lazy_mint(&mint.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    expect_instruction_error(result, &InstructionError::IllegalOwner);
}

/// Test LazyAccount<Mint> - wrong size (too small, 40 bytes instead of 82)
#[test]
fn test_lazy_mint_wrong_size() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let mint = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create mint with wrong size
    create_mint_wrong_size(&mut svm, &mint.pubkey());

    let ix = test_lazy_mint(&mint.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // LazyAccount returns InvalidAccountData for size mismatch
    expect_instruction_error(result, &InstructionError::InvalidAccountData);
}

/// Test LazyAccount<Mint> - uninitialized (empty data)
#[test]
fn test_lazy_mint_uninitialized() {
    let mut svm = create_svm();

    let payer = Keypair::new();
    let mint = Keypair::new();
    airdrop(&mut svm, &payer.pubkey(), 10 * SOL);

    // Create uninitialized mint
    create_mint_uninitialized(&mut svm, &mint.pubkey());

    let ix = test_lazy_mint(&mint.pubkey());
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer],
        svm.latest_blockhash(),
    );

    let result = svm.send_transaction(tx);
    // LazyAccount returns InvalidAccountData for uninitialized/empty accounts
    expect_instruction_error(result, &InstructionError::InvalidAccountData);
}
