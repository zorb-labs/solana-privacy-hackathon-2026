use crate::errors::ShieldedPoolError;
use panchor::prelude::*;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey,
};
use pinocchio_contrib::AccountAssertions;

use crate::state::ShieldedPoolAccount;

/// Header size for TransactSession (on-chain)
/// discriminator(8) + authority(32) + nonce(8) + created_slot(8) + data_len(4) + bump(1) + padding(3) = 64 bytes
pub const TRANSACT_SESSION_HEADER_SIZE: usize = 64;

/// Duration in slots after which anyone can close a session
/// ~400ms per slot, so 24 hours ≈ 216,000 slots
pub const SESSION_EXPIRY_SLOTS: u64 = 216_000;

/// Maximum allowed data length for transact session body.
/// Limits proof + params + encrypted outputs to 4KB.
pub const MAX_SESSION_DATA_LEN: u32 = 4096;

/// Transact session account for splitting transact into multiple transactions.
///
/// This account stores raw transaction data uploaded in chunks.
/// The setup phase creates the account and uploads data in pieces.
/// The execute phase deserializes and validates all data, then executes.
///
/// After execution, the session should be closed via `close_transact_session`.
/// Re-execution is prevented by closing the account (reclaiming rent).
///
/// # Account Layout (on-chain)
/// `[8-byte discriminator][56-byte struct][variable-length data]`
///
/// Seeds: ["transact_session", authority, nonce]
#[account(ShieldedPoolAccount::TransactSession)]
#[repr(C)]
pub struct TransactSession {
    /// Authority that created this session (can upload chunks and close)
    pub authority: Pubkey,
    /// Nonce for multiple concurrent sessions
    pub nonce: u64,
    /// Slot when this session was created (for expiry tracking)
    pub created_slot: u64,
    /// Total expected data length (not including header)
    pub data_len: u32,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 3],
    // Data follows immediately after (variable length, borsh-serialized)
}

impl TransactSession {
    /// Base size of the header (same as TRANSACT_SESSION_HEADER_SIZE)
    pub const BASE_SIZE: usize = TRANSACT_SESSION_HEADER_SIZE;

    /// Calculate total account size for a given data length
    #[inline]
    pub const fn account_size(data_len: u32) -> usize {
        TRANSACT_SESSION_HEADER_SIZE + data_len as usize
    }

    /// Get the data slice from account data (after header)
    #[inline]
    pub fn get_data(account_data: &[u8]) -> &[u8] {
        &account_data[TRANSACT_SESSION_HEADER_SIZE..]
    }

    /// Get the mutable data slice from account data (after header)
    #[inline]
    pub fn get_data_mut(account_data: &mut [u8]) -> &mut [u8] {
        &mut account_data[TRANSACT_SESSION_HEADER_SIZE..]
    }

    /// Load account and return header + raw body bytes.
    ///
    /// Returns the header (copied, 56 bytes) and a Ref to the body data.
    /// The body contains: Proof + TransactParams + NullifierNMProof + encrypted_outputs.
    pub fn load_body<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<(Self, pinocchio::account_info::Ref<'a, [u8]>), ProgramError> {
        account.assert_owner(program_id)?;

        let data = account.try_borrow_data()?;
        if data.len() < TRANSACT_SESSION_HEADER_SIZE {
            return Err(ProgramError::InvalidAccountData);
        }

        // Check discriminator
        let discriminator = u64::from_le_bytes(data[..8].try_into().unwrap());
        if discriminator != Self::DISCRIMINATOR {
            return Err(ShieldedPoolError::InvalidDiscriminator.into());
        }

        // Copy header (56 bytes at offset 8)
        let header = *bytemuck::from_bytes::<Self>(&data[8..TRANSACT_SESSION_HEADER_SIZE]);

        // Return header and mapped Ref to body
        let body_ref =
            pinocchio::account_info::Ref::map(data, |d| &d[TRANSACT_SESSION_HEADER_SIZE..]);
        Ok((header, body_ref))
    }

    /// Load header only (for upload/close validation).
    ///
    /// Returns a Ref to the header for reading authority, nonce, etc.
    pub fn load_header<'a>(
        account: &'a AccountInfo,
        program_id: &Pubkey,
    ) -> Result<pinocchio::account_info::Ref<'a, Self>, ProgramError> {
        account.assert_owner(program_id)?;

        let data = account.try_borrow_data()?;
        if data.len() < TRANSACT_SESSION_HEADER_SIZE {
            return Err(ProgramError::InvalidAccountData);
        }

        let discriminator = u64::from_le_bytes(data[..8].try_into().unwrap());
        if discriminator != Self::DISCRIMINATOR {
            return Err(ShieldedPoolError::InvalidDiscriminator.into());
        }

        Ok(pinocchio::account_info::Ref::map(data, |d| {
            bytemuck::from_bytes::<Self>(&d[8..TRANSACT_SESSION_HEADER_SIZE])
        }))
    }

    /// Initialize account with header data.
    ///
    /// Writes discriminator and header fields. Body data should be
    /// written separately via upload_transact_chunk.
    pub fn init_account(
        account: &AccountInfo,
        authority: &Pubkey,
        nonce: u64,
        data_len: u32,
        bump: u8,
        created_slot: u64,
    ) -> ProgramResult {
        let mut data = account.try_borrow_mut_data()?;

        // Write discriminator
        data[..8].copy_from_slice(&Self::DISCRIMINATOR.to_le_bytes());

        // Write header at offset 8
        let header = bytemuck::from_bytes_mut::<Self>(&mut data[8..TRANSACT_SESSION_HEADER_SIZE]);
        header.authority = *authority;
        header.nonce = nonce;
        header.created_slot = created_slot;
        header.data_len = data_len;
        header.bump = bump;
        header._padding = [0u8; 3];

        Ok(())
    }
}
