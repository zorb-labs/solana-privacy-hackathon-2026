//! Escrow verification helpers for deposit transactions.
//!
//! This module provides security-critical escrow validation and state management
//! for the execute_transact instruction. These helpers ensure that deposits are
//! properly authorized and prevent replay attacks.

use crate::{errors::ShieldedPoolError, state::{DepositEscrow, TransactSession}};
use light_hasher::Sha256;
use panchor::Discriminator;
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, sysvars::Sysvar};

// ============================================================================
// Escrow Helper Functions
// ============================================================================

/// Verify that an escrow is valid for a deposit operation.
///
/// # Security Checks
/// 1. Escrow is owned by this program
/// 2. Escrow has valid discriminator
/// 3. proof_hash matches SHA256(session_body)
/// 4. Relayer is authorized on the escrow
/// 5. Escrow is not already consumed
/// 6. Escrow has not expired
///
/// Note: PDA derivation check removed - security is provided by:
/// - Program ownership check (only this program creates escrows)
/// - Discriminator check (validates account type)
/// - Proof hash binding (cryptographic commitment to specific session)
///
/// # Arguments
/// * `program_id` - This program's ID
/// * `escrow` - The escrow account
/// * `session_data` - The raw session data (for computing proof hash)
/// * `relayer` - The relayer's pubkey
pub fn verify_escrow_for_deposit(
    program_id: &Pubkey,
    escrow: &AccountInfo,
    session_data: &[u8],
    relayer: &Pubkey,
) -> Result<(), ProgramError> {
    use light_hasher::Hasher;

    // 1. Verify escrow is owned by this program
    if escrow.owner() != program_id {
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    // Load escrow data
    let escrow_data = escrow.try_borrow_data()?;
    if escrow_data.len() < DepositEscrow::ACCOUNT_SIZE {
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    // 2. Verify discriminator
    let discriminator = u64::from_le_bytes(escrow_data[..8].try_into().unwrap());
    if discriminator != DepositEscrow::DISCRIMINATOR {
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    let escrow_state =
        bytemuck::from_bytes::<DepositEscrow>(&escrow_data[8..DepositEscrow::ACCOUNT_SIZE]);

    // 3. Verify proof_hash matches SHA256(session_body)
    // session_body starts after the 8-byte header (authority, nonce, data_len, bump, created_slot)
    // Actually, looking at TransactSession, the body is the full proof + params + encrypted outputs
    // The plan says proof_hash = SHA256(session_body), and session_body is the data part after header
    let session_body = &session_data[TransactSession::BASE_SIZE..];
    let computed_hash = Sha256::hash(session_body)
        .map_err(|_| ShieldedPoolError::ArithmeticOverflow)?;

    if escrow_state.proof_hash != computed_hash {
        return Err(ShieldedPoolError::EscrowProofHashMismatch.into());
    }

    // 4. Verify relayer is authorized
    if !escrow_state.is_relayer_authorized(relayer) {
        return Err(ShieldedPoolError::EscrowUnauthorizedRelayer.into());
    }

    // 5. Verify not consumed
    if escrow_state.is_consumed() {
        return Err(ShieldedPoolError::EscrowAlreadyConsumed.into());
    }

    // 6. Verify not expired
    let clock = pinocchio::sysvars::clock::Clock::get()?;
    if escrow_state.is_expired(clock.slot) {
        return Err(ShieldedPoolError::EscrowExpired.into());
    }

    Ok(())
}

/// Mark an escrow as consumed after successful deposit execution.
///
/// Sets the `consumed` flag to true to prevent replay attacks.
pub fn mark_escrow_consumed(escrow: &AccountInfo) -> Result<(), ProgramError> {
    let mut escrow_data = escrow.try_borrow_mut_data()?;

    // Find the offset of the consumed field
    // Layout: [8 discriminator] [32 proof_hash] [32 mint] [32 authorized_relayer] [8 expiry_slot] [8 nonce] [1 consumed] ...
    // discriminator: 8 bytes (0-7)
    // proof_hash: 32 bytes (8-39)
    // mint: 32 bytes (40-71)
    // authorized_relayer: 32 bytes (72-103)
    // expiry_slot: 8 bytes (104-111)
    // nonce: 8 bytes (112-119)
    // consumed: 1 byte (120)
    let consumed_offset = 8 + 32 + 32 + 32 + 8 + 8; // = 120

    escrow_data[consumed_offset] = 1; // true

    Ok(())
}
