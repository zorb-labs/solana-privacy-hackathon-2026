//! Authority transfer initiated event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when an authority transfer is initiated.
///
/// This event is emitted after a successful `transfer_authority` instruction,
/// which sets the `pending_authority` field on the global config. The transfer
/// is not complete until `accept_authority` is called by the pending authority.
///
/// # Security Considerations
///
/// This is a security-critical event that should be monitored for:
/// - Unauthorized transfer attempts
/// - Unusual transfer patterns
/// - Monitoring for timely acceptance (or lack thereof)
///
/// # Usage by Indexers
///
/// 1. Track pending authority transfers
/// 2. Alert on authority transfer initiation
/// 3. Monitor for acceptance timeout (if applicable)
/// 4. Audit trail for governance changes
#[event(EventType::AuthorityTransferInitiated)]
#[repr(C)]
pub struct AuthorityTransferInitiatedEvent {
    /// Current authority initiating the transfer.
    pub current_authority: Pubkey,
    /// New authority who must accept.
    pub pending_authority: Pubkey,
    /// Slot when transfer was initiated.
    pub slot: u64,
}
