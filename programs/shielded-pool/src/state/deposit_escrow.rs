//! Deposit escrow state for relayer-assisted deposits.
//!
//! This module implements an escrow mechanism that enables single-transaction UX for deposits:
//! 1. User creates escrow (ONE on-chain tx) - transfers tokens to escrow vault
//! 2. User sends proof data to relayer (off-chain)
//! 3. Relayer handles all transact transactions (init_session, upload_chunks, execute_transact)
//!
//! # Security Model
//!
//! The escrow binds deposited tokens to a specific proof via `proof_hash = SHA256(session_body)`.
//! This creates a cryptographic commitment chain:
//! - `escrow.proof_hash` → `session_body` → `transactParamsHash` (ZK-bound)
//!
//! The relayer can only consume the escrow if they execute the exact proof the user authorized.
//! This prevents:
//! - Relayer substituting a different proof
//! - Relayer modifying the recipient/amounts
//! - Replay attacks (consumed flag + expiry)

use crate::state::ShieldedPoolAccount;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Deposit escrow account for relayer-assisted deposits.
///
/// This account holds tokens deposited by a user, bound to a specific proof hash.
/// A relayer can consume the escrow when executing the matching transact proof.
///
/// # Account Layout
/// `[8-byte discriminator][struct fields]`
///
/// # PDA Seeds
/// `["deposit_escrow", depositor, nonce]`
#[account(ShieldedPoolAccount::DepositEscrow)]
#[repr(C)]
pub struct DepositEscrow {
    /// SHA256 hash of the session_body that this escrow is bound to.
    /// Must match `SHA256(session.body)` when consuming the escrow.
    /// This binds the escrow to the exact proof parameters the user authorized.
    pub proof_hash: [u8; 32],

    /// The SPL token mint for this escrow's vault.
    /// Used to derive the escrow vault ATA.
    pub mint: Pubkey,

    /// Authorized relayer pubkey, or [0;32] to allow any relayer.
    /// If non-zero, only this relayer can consume the escrow.
    pub authorized_relayer: Pubkey,

    /// Slot after which the depositor can reclaim the escrow.
    /// Calculated as: created_slot + expiry_slots
    pub expiry_slot: u64,

    /// Unique nonce for this escrow (part of PDA derivation).
    /// Allows a depositor to have multiple concurrent escrows.
    pub nonce: u64,

    /// Whether this escrow has been consumed by a transact.
    /// Set to 1 (true) after successful escrow deposit execution, 0 (false) otherwise.
    /// Using u8 instead of bool for bytemuck::Pod compatibility.
    pub consumed: u8,

    /// PDA bump seed for this escrow account.
    pub bump: u8,

    /// Padding for 8-byte alignment.
    pub _padding: [u8; 6],
}

impl DepositEscrow {
    /// Size of the DepositEscrow struct (excluding discriminator).
    /// 32 (proof_hash) + 32 (mint) + 32 (authorized_relayer) + 8 (expiry_slot)
    /// + 8 (nonce) + 1 (consumed) + 1 (bump) + 6 (padding) = 120 bytes
    pub const SIZE: usize = 120;

    /// Total account size including 8-byte discriminator.
    pub const ACCOUNT_SIZE: usize = 8 + Self::SIZE;

    /// Check if the given relayer is authorized to consume this escrow.
    ///
    /// Returns true if:
    /// - authorized_relayer is zero (any relayer allowed), OR
    /// - relayer matches authorized_relayer
    #[inline]
    pub fn is_relayer_authorized(&self, relayer: &Pubkey) -> bool {
        self.authorized_relayer == Pubkey::default() || self.authorized_relayer == *relayer
    }

    /// Check if this escrow has expired.
    ///
    /// Returns true if current_slot > expiry_slot.
    #[inline]
    pub fn is_expired(&self, current_slot: u64) -> bool {
        current_slot > self.expiry_slot
    }

    /// Check if this escrow has been consumed.
    #[inline]
    pub fn is_consumed(&self) -> bool {
        self.consumed != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_relayer_authorized() {
        let mut escrow = DepositEscrow {
            proof_hash: [0u8; 32],
            mint: Pubkey::default(),
            authorized_relayer: Pubkey::default(),
            expiry_slot: 0,
            nonce: 0,
            consumed: 0,
            bump: 0,
            _padding: [0u8; 6],
        };

        let relayer1: Pubkey = [1u8; 32];
        let relayer2: Pubkey = [2u8; 32];

        // Zero authorized_relayer allows any relayer
        assert!(escrow.is_relayer_authorized(&relayer1));
        assert!(escrow.is_relayer_authorized(&relayer2));

        // Set specific authorized relayer
        escrow.authorized_relayer = relayer1;
        assert!(escrow.is_relayer_authorized(&relayer1));
        assert!(!escrow.is_relayer_authorized(&relayer2));
    }

    #[test]
    fn test_is_expired() {
        let escrow = DepositEscrow {
            proof_hash: [0u8; 32],
            mint: Pubkey::default(),
            authorized_relayer: Pubkey::default(),
            expiry_slot: 1000,
            nonce: 0,
            consumed: 0,
            bump: 0,
            _padding: [0u8; 6],
        };

        // Not expired at or before expiry_slot
        assert!(!escrow.is_expired(999));
        assert!(!escrow.is_expired(1000));

        // Expired after expiry_slot
        assert!(escrow.is_expired(1001));
    }

    #[test]
    fn test_struct_size() {
        // Verify the struct size matches our constant
        assert_eq!(
            core::mem::size_of::<DepositEscrow>(),
            DepositEscrow::SIZE,
            "DepositEscrow struct size mismatch"
        );
    }
}
