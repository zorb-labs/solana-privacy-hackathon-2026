//! Pool config active changed event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when a pool config's active state is changed.
///
/// This event is emitted after a successful `set_pool_config_active` instruction,
/// which enables or disables pool routing for a specific asset.
///
/// # Usage by Indexers
///
/// 1. Track pool config state changes
/// 2. Update asset routing availability
/// 3. Alert on asset enable/disable
/// 4. Audit trail for asset management
#[event(EventType::PoolConfigActiveChanged)]
#[repr(C)]
pub struct PoolConfigActiveChangedEvent {
    /// Authority who changed the state.
    pub authority: Pubkey,
    /// Asset ID affected.
    pub asset_id: [u8; 32],
    /// New state: 1 = enabled, 0 = disabled.
    pub is_active: u8,
    /// Padding for 8-byte alignment.
    pub _padding: [u8; 7],
}
