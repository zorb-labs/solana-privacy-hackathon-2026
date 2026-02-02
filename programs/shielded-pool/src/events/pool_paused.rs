//! Pool pause changed event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when the pool's paused state is changed.
///
/// This event is emitted after a successful `set_pool_paused` instruction,
/// which enables or disables all pool operations. The event is emitted for
/// both pausing AND unpausing - check `is_paused` field for the new state.
///
/// # Security Considerations
///
/// This is a security-critical event that should be monitored for:
/// - Emergency pause activations
/// - Unexpected pause state changes
/// - Protocol availability tracking
///
/// # Usage by Indexers
///
/// 1. Track pool operational status
/// 2. Alert on pause state changes
/// 3. Calculate pool downtime metrics
/// 4. Audit trail for operational changes
#[event(EventType::PoolPauseChanged)]
#[repr(C)]
pub struct PoolPauseChangedEvent {
    /// Authority who changed the state.
    pub authority: Pubkey,
    /// New state: 1 = paused, 0 = active.
    pub is_paused: u8,
    /// Padding for 8-byte alignment.
    pub _padding: [u8; 7],
    /// Slot when state changed.
    pub slot: u64,
}
