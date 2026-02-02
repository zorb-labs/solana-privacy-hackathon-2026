//! Session data loading and validation for execute_transact.
//!
//! This module provides helpers to parse the transact session account data
//! into its component parts: proof, params, nullifier proof, and encrypted outputs.
//!
//! Parsing is zero-copy for large data structures (proof ~800 bytes, params ~700 bytes).
//! Only the small session header (56 bytes) is copied.

use crate::{
    errors::ShieldedPoolError,
    instructions::types::{
        N_OUTS, NULLIFIER_NM_PROOF_SIZE, NullifierNonMembershipProofData, PROOF_SIZE,
        TRANSACT_PARAMS_SIZE, TransactParams, TransactProofData,
    },
    state::{TRANSACT_SESSION_HEADER_SIZE, TransactSession},
    utils,
};
use panchor::prelude::*; // For HasDiscriminator trait
use pinocchio::program_error::ProgramError;

/// Minimum data size for a valid transact session body.
/// Proof + TransactParams + NullifierNonMembershipProofData
pub const MIN_SESSION_DATA_SIZE: usize =
    PROOF_SIZE + TRANSACT_PARAMS_SIZE + NULLIFIER_NM_PROOF_SIZE;

/// Parsed session data with zero-copy references to the underlying account data.
///
/// This struct provides convenient access to all components stored in a transact session:
/// - The session header (authority, nonce, created_slot, etc.) - copied, 56 bytes
/// - The ZK proof for the transaction - zero-copy reference
/// - Transaction parameters (amounts, recipients, fees, etc.) - zero-copy reference
/// - Nullifier non-membership proof - zero-copy reference
/// - Encrypted output ciphertexts - zero-copy slices
///
/// The caller must keep the account data borrow alive for the references to remain valid.
pub struct SessionData<'a> {
    /// Session header (authority, nonce, created_slot, etc.) - small copy (56 bytes)
    pub header: TransactSession,
    /// Groth16 proof and public inputs - zero-copy reference (~800 bytes)
    pub proof: &'a TransactProofData,
    /// Transaction parameters bound by the proof - zero-copy reference (~700 bytes)
    pub params: &'a TransactParams,
    /// Nullifier non-membership proof for the indexed tree - zero-copy reference
    pub nullifier_nm_proof: &'a NullifierNonMembershipProofData,
    /// Encrypted output ciphertexts (one per output note) - zero-copy slices
    pub encrypted_outputs: [&'a [u8]; N_OUTS],
}

impl<'a> SessionData<'a> {
    /// Check if this transaction has a relayer (not system program).
    #[inline]
    pub fn has_relayer(&self) -> bool {
        self.params.relayer != pinocchio_system::ID
    }

    /// Check if this transaction has any deposits (positive ext_amounts).
    #[inline]
    pub fn has_deposits(&self) -> bool {
        self.params.ext_amounts.iter().any(|&a| a > 0)
    }

    /// Check if this transaction has any withdrawals (negative ext_amounts).
    #[inline]
    pub fn has_withdrawals(&self) -> bool {
        self.params.ext_amounts.iter().any(|&a| a < 0)
    }
}

/// Parse session data from raw account bytes.
///
/// This function performs mostly zero-copy parsing:
/// 1. Validates discriminator
/// 2. Copies the small header (56 bytes)
/// 3. Returns zero-copy references to proof, params, nullifier proof (~1.5KB total)
/// 4. Parses encrypted outputs as zero-copy slices
/// 5. Validates encrypted output hashes match params
///
/// # Arguments
/// * `data` - Raw account data bytes (discriminator + header + body)
///
/// # Returns
/// * `SessionData` with references into the provided data
///
/// # Errors
/// * `InvalidDiscriminator` - Wrong account type
/// * `InvalidAccountData` - Data too small or malformed
/// * `InvalidEncryptedOutputHash` - Hash mismatch in encrypted outputs
pub fn parse_session_data(data: &[u8]) -> Result<SessionData<'_>, ProgramError> {
    if data.len() < TRANSACT_SESSION_HEADER_SIZE {
        return Err(ProgramError::InvalidAccountData);
    }

    // Validate discriminator
    let discriminator = u64::from_le_bytes(data[..8].try_into().unwrap());
    if discriminator != TransactSession::DISCRIMINATOR {
        return Err(ShieldedPoolError::InvalidDiscriminator.into());
    }

    // Copy header (56 bytes) - necessary for borrow checker, negligible cost
    let header: TransactSession = *bytemuck::from_bytes(&data[8..TRANSACT_SESSION_HEADER_SIZE]);

    // Body starts after header
    let body = &data[TRANSACT_SESSION_HEADER_SIZE..];

    // Verify minimum body size
    if body.len() < MIN_SESSION_DATA_SIZE {
        return Err(ProgramError::InvalidAccountData);
    }

    // Zero-copy parse proof (~800 bytes - no stack allocation)
    let proof: &TransactProofData = bytemuck::from_bytes(&body[..PROOF_SIZE]);

    // Zero-copy parse params (~700 bytes)
    let params: &TransactParams =
        bytemuck::from_bytes(&body[PROOF_SIZE..PROOF_SIZE + TRANSACT_PARAMS_SIZE]);

    // Zero-copy parse nullifier non-membership proof
    let nm_proof_start = PROOF_SIZE + TRANSACT_PARAMS_SIZE;
    let nullifier_nm_proof: &NullifierNonMembershipProofData =
        bytemuck::from_bytes(&body[nm_proof_start..nm_proof_start + NULLIFIER_NM_PROOF_SIZE]);

    // Parse encrypted outputs from remaining data (Borsh format: u32 length prefix + bytes)
    let encrypted_data_start = nm_proof_start + NULLIFIER_NM_PROOF_SIZE;
    let encrypted_data = &body[encrypted_data_start..];

    let encrypted_outputs = parse_encrypted_outputs(encrypted_data)?;

    // Validate encrypted output hashes match the hashes committed in params
    validate_encrypted_output_hashes(&encrypted_outputs, params)?;

    Ok(SessionData {
        header,
        proof,
        params,
        nullifier_nm_proof,
        encrypted_outputs,
    })
}

/// Parse N_OUTS encrypted outputs from Borsh-formatted data.
///
/// Format: For each output, a u32 length prefix followed by that many bytes.
fn parse_encrypted_outputs(data: &[u8]) -> Result<[&[u8]; N_OUTS], ProgramError> {
    let mut outputs: [&[u8]; N_OUTS] = [&[]; N_OUTS];
    let mut offset = 0;

    for i in 0..N_OUTS {
        // Read length prefix
        if data.len() < offset + 4 {
            return Err(ProgramError::InvalidAccountData);
        }
        let len = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        // Read output data
        if data.len() < offset + len {
            return Err(ProgramError::InvalidAccountData);
        }
        outputs[i] = &data[offset..offset + len];
        offset += len;
    }

    Ok(outputs)
}

/// Validate that encrypted output hashes match the hashes in params.
///
/// The params struct contains SHA256 hashes of each encrypted output,
/// which are bound by the ZK proof. This function verifies the actual
/// encrypted outputs match those committed hashes.
fn validate_encrypted_output_hashes(
    encrypted_outputs: &[&[u8]; N_OUTS],
    params: &TransactParams,
) -> Result<(), ProgramError> {
    for (output, expected_hash) in encrypted_outputs.iter().zip(&params.encrypted_output_hashes) {
        let computed_hash = utils::sha256(output);
        if computed_hash != *expected_hash {
            return Err(ShieldedPoolError::InvalidEncryptedOutputHash.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_session_data_size() {
        // Verify the constant matches the sum of component sizes
        assert_eq!(
            MIN_SESSION_DATA_SIZE,
            PROOF_SIZE + TRANSACT_PARAMS_SIZE + NULLIFIER_NM_PROOF_SIZE
        );
    }
}
