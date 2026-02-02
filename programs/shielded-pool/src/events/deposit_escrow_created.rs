//! Deposit escrow created event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when a deposit escrow is created.
///
/// This event is emitted after a successful `init_deposit_escrow` instruction,
/// which creates an escrow account and transfers tokens to a vault for
/// relayer-assisted deposits.
///
/// # Usage by Indexers
///
/// 1. Track pending deposit escrows
/// 2. Monitor escrow expiry for user notifications
/// 3. Match escrows to transact executions
/// 4. Calculate deposit volume metrics
#[event(EventType::DepositEscrowCreated)]
#[repr(C)]
pub struct DepositEscrowCreatedEvent {
    /// User who created the escrow.
    pub depositor: Pubkey,
    /// Escrow PDA address.
    pub escrow: Pubkey,
    /// Token mint of escrowed tokens.
    pub mint: Pubkey,
    /// SHA256 of transact session (binds escrow to proof).
    pub proof_hash: [u8; 32],
    /// Authorized relayer (zero = any relayer allowed).
    pub authorized_relayer: Pubkey,
    /// Slot after which depositor can reclaim tokens.
    pub expiry_slot: u64,
    /// Escrow nonce (for multiple concurrent escrows).
    pub nonce: u64,
    /// Amount of tokens escrowed.
    pub amount: u64,
}
