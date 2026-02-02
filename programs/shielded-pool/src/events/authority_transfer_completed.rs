//! Authority transfer completed event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when an authority transfer is completed.
///
/// This event is emitted after a successful `accept_authority` instruction,
/// which transfers the authority role from the previous authority to the
/// new authority who accepted.
///
/// # Security Considerations
///
/// This is a security-critical event that should be monitored for:
/// - Confirmation of legitimate authority transfers
/// - Unauthorized authority changes
/// - Governance compliance tracking
///
/// # Usage by Indexers
///
/// 1. Update authority tracking records
/// 2. Clear pending transfer state
/// 3. Alert on authority change completion
/// 4. Audit trail for governance changes
#[event(EventType::AuthorityTransferCompleted)]
#[repr(C)]
pub struct AuthorityTransferCompletedEvent {
    /// Authority who initiated and relinquished control.
    pub previous_authority: Pubkey,
    /// Authority who accepted and now controls protocol.
    pub new_authority: Pubkey,
    /// Slot when transfer was completed.
    pub slot: u64,
}
