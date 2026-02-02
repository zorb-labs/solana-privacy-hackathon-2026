//! Deposit escrow closed event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when a deposit escrow is closed.
///
/// This event is emitted after a successful `close_deposit_escrow` instruction,
/// which transfers tokens back to the depositor and closes the escrow account.
///
/// # Closure Conditions
///
/// A deposit escrow can be closed when:
/// - The escrow has expired (current slot > expiry_slot)
/// - The escrow has not been consumed by a transact execution
///
/// # Usage by Indexers
///
/// 1. Remove escrow from active tracking
/// 2. Track escrow cancellation rate
/// 3. Calculate returned token volume
/// 4. Audit trail for escrow lifecycle
#[event(EventType::DepositEscrowClosed)]
#[repr(C)]
pub struct DepositEscrowClosedEvent {
    /// User who closed the escrow (original depositor).
    pub depositor: Pubkey,
    /// Escrow PDA address.
    pub escrow: Pubkey,
    /// Token mint.
    pub mint: Pubkey,
    /// Amount of tokens returned to depositor.
    pub amount_returned: u64,
    /// Escrow nonce (for correlation with creation event).
    pub nonce: u64,
    /// Rent lamports reclaimed.
    pub lamports_reclaimed: u64,
}
