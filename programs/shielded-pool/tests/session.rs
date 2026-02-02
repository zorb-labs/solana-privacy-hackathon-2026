//! Transact session lifecycle tests.
//!
//! Tests for InitTransactSession, UploadTransactChunk, and CloseTransactSession.

mod common;

use common::*;
use litesvm::LiteSVM;
use solana_keypair::Keypair;
use solana_signer::Signer;

// ============================================================================
// Session Lifecycle Tests
// ============================================================================

/// Test the full session lifecycle: init, upload, close.
#[test]
fn test_session_lifecycle() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Init session
    let nonce = 1u64;
    let data_len = 1024u32;
    let session_result = init_transact_session(&mut svm, &program_id, &authority, nonce, data_len);
    assert!(session_result.is_ok(), "init_transact_session failed");

    let session = session_result.unwrap();

    // Verify session account exists
    let account = svm.get_account(&session).expect("session should exist");
    assert_eq!(
        account.owner, program_id,
        "session should be owned by program"
    );

    // Upload a chunk
    let chunk_data = vec![0u8; 100];
    let upload_result =
        upload_transact_chunk(&mut svm, &program_id, &session, &authority, 0, chunk_data);
    assert!(upload_result.is_ok(), "upload_transact_chunk failed");

    // Close session
    let close_result = close_transact_session(&mut svm, &program_id, &session, &authority);
    assert!(close_result.is_ok(), "close_transact_session failed");

    // Verify session account is closed
    let closed_account = svm.get_account(&session);
    assert!(
        closed_account.is_none() || closed_account.unwrap().lamports == 0,
        "session should be closed"
    );
}

// ============================================================================
// Chunked Upload Tests
// ============================================================================

/// Test uploading data in multiple chunks.
#[test]
fn test_chunked_upload() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Init session with larger data buffer
    let nonce = 2u64;
    let data_len = 2000u32;
    let session = init_transact_session(&mut svm, &program_id, &authority, nonce, data_len)
        .expect("init_transact_session should succeed");

    // Upload in chunks of 900 bytes
    let chunk1 = vec![1u8; 900];
    let chunk2 = vec![2u8; 900];
    let chunk3 = vec![3u8; 200];

    let upload1 = upload_transact_chunk(&mut svm, &program_id, &session, &authority, 0, chunk1);
    assert!(upload1.is_ok(), "upload chunk 1 failed");

    let upload2 = upload_transact_chunk(&mut svm, &program_id, &session, &authority, 900, chunk2);
    assert!(upload2.is_ok(), "upload chunk 2 failed");

    let upload3 = upload_transact_chunk(&mut svm, &program_id, &session, &authority, 1800, chunk3);
    assert!(upload3.is_ok(), "upload chunk 3 failed");

    // Close session
    let close_result = close_transact_session(&mut svm, &program_id, &session, &authority);
    assert!(close_result.is_ok(), "close_transact_session failed");
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

/// Test that uploading beyond the allocated data length fails.
#[test]
fn test_upload_beyond_data_len() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Init session with small data buffer (100 bytes)
    let nonce = 3u64;
    let data_len = 100u32;
    let session = init_transact_session(&mut svm, &program_id, &authority, nonce, data_len)
        .expect("init_transact_session should succeed");

    // Try to upload chunk that would exceed the buffer
    // offset 50 + 100 bytes = 150 bytes, which exceeds data_len of 100
    let chunk_data = vec![0u8; 100];
    let upload_result =
        upload_transact_chunk(&mut svm, &program_id, &session, &authority, 50, chunk_data);
    assert!(
        upload_result.is_err(),
        "upload should fail when chunk would exceed data_len"
    );

    // Note: Uploading with offset beyond buffer may succeed if data fits within
    // the allocated account space (which includes header bytes). The bounds check
    // is on offset + data.len() <= total account data size, not data_len.

    // Clean up
    close_transact_session(&mut svm, &program_id, &session, &authority)
        .expect("close should succeed");
}

// ============================================================================
// Authorization Tests
// ============================================================================

/// Test that uploading to a session with wrong authority fails.
#[test]
fn test_upload_wrong_authority() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Init session
    let nonce = 4u64;
    let data_len = 1024u32;
    let session = init_transact_session(&mut svm, &program_id, &authority, nonce, data_len)
        .expect("init_transact_session should succeed");

    // Create a different keypair (unauthorized)
    let other_user = Keypair::new();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    // Try to upload with wrong authority
    let chunk_data = vec![0u8; 100];
    let upload_result =
        upload_transact_chunk(&mut svm, &program_id, &session, &other_user, 0, chunk_data);
    assert!(
        upload_result.is_err(),
        "upload should fail with wrong authority"
    );

    // Clean up with correct authority
    close_transact_session(&mut svm, &program_id, &session, &authority)
        .expect("close should succeed with correct authority");
}

/// Test that closing a session with wrong authority fails.
#[test]
fn test_close_session_wrong_authority() {
    let mut svm = LiteSVM::new();
    let program_id = deploy_program(&mut svm);

    // Initialize shielded pool
    let (_, _, _, _, authority) = initialize_shielded_pool(&mut svm, &program_id);

    // Init session
    let nonce = 5u64;
    let data_len = 1024u32;
    let session = init_transact_session(&mut svm, &program_id, &authority, nonce, data_len)
        .expect("init_transact_session should succeed");

    // Create a different keypair (unauthorized)
    let other_user = Keypair::new();
    svm.airdrop(&other_user.pubkey(), 10_000_000_000).unwrap();

    // Try to close with wrong authority
    let close_result = close_transact_session(&mut svm, &program_id, &session, &other_user);
    assert!(
        close_result.is_err(),
        "close should fail with wrong authority"
    );

    // Verify session still exists
    let account = svm
        .get_account(&session)
        .expect("session should still exist");
    assert!(account.lamports > 0, "session should not have been closed");

    // Clean up with correct authority
    close_transact_session(&mut svm, &program_id, &session, &authority)
        .expect("close should succeed with correct authority");
}
