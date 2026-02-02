//! Pool registered event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when a pool is registered with the hub.
///
/// This event is emitted after a successful `register_token_pool` or
/// `register_unified_sol_pool` instruction, which creates a `PoolConfig`
/// account linking the hub to a pool program.
///
/// # Usage by Indexers
///
/// 1. Track which pools are registered with the hub
/// 2. Map asset_ids to their corresponding pool programs
/// 3. Monitor pool registration activity for analytics
#[event(EventType::PoolRegistered)]
#[repr(C)]
pub struct PoolRegisteredEvent {
    /// The pool type (0 = Token, 1 = UnifiedSol)
    pub pool_type: u8,
    /// Padding for alignment
    pub _padding: [u8; 7],
    /// The asset ID for this pool
    pub asset_id: [u8; 32],
    /// The pool program ID
    pub pool_program: Pubkey,
}
