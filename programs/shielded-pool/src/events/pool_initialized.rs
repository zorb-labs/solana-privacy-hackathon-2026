//! Pool initialized event definition.

use super::EventType;
use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;

/// Event emitted when the shielded pool is initialized.
///
/// This event is emitted after a successful `initialize` instruction,
/// which creates the four core PDAs (commitment tree, receipt tree,
/// nullifier tree, and global config).
///
/// # Historical Record
///
/// This event provides a genesis record for the protocol, enabling:
/// - Protocol start timestamp tracking
/// - Initial authority identification
/// - Core PDA address discovery
///
/// # Usage by Indexers
///
/// 1. Record protocol genesis block/slot
/// 2. Track initial authority
/// 3. Store core PDA addresses for reference
/// 4. Initialize indexer state from genesis
#[event(EventType::PoolInitialized)]
#[repr(C)]
pub struct PoolInitializedEvent {
    /// Initial protocol authority.
    pub authority: Pubkey,
    /// Commitment merkle tree PDA.
    pub commitment_tree: Pubkey,
    /// Receipt merkle tree PDA.
    pub receipt_tree: Pubkey,
    /// Nullifier indexed tree PDA.
    pub nullifier_tree: Pubkey,
    /// Global config PDA.
    pub global_config: Pubkey,
    /// Genesis slot.
    pub slot: u64,
}
