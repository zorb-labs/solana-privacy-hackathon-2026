//! Event definitions for the shielded pool program.
//!
//! Events are emitted via self-invocation of the Log instruction,
//! which allows event data to be recorded in transaction logs without truncation.
//!
//! # Event Types
//!
//! ## Core Events (1-15)
//! - [`NewCommitmentEvent`] - Emitted when a new commitment is added to the tree
//! - [`NewNullifierEvent`] - Emitted when a nullifier is created (spent note)
//! - [`NewReceiptEvent`] - Emitted when a transaction receipt is recorded
//! - [`NullifierBatchInsertedEvent`] - Emitted when nullifiers are batch inserted into indexed tree
//! - [`NullifierLeafInsertedEvent`] - Emitted per-nullifier with full leaf data during batch insert
//! - [`NullifierPdaClosedEvent`] - Emitted when a nullifier PDA is closed (GC)
//! - [`NullifierEpochAdvancedEvent`] - Emitted when nullifier epoch is advanced
//! - [`NullifierEarliestEpochAdvancedEvent`] - Emitted when earliest provable epoch changes
//! - [`NullifierEpochRootClosedEvent`] - Emitted when a nullifier epoch root PDA is closed (GC)
//!
//! ## Transfer/Escrow Events (16-31)
//! - [`DepositEscrowCreatedEvent`] - Emitted when a deposit escrow is created
//! - [`DepositEscrowClosedEvent`] - Emitted when a deposit escrow is closed
//!
//! ## State Change Events (32-47)
//! - Reserved for future use (e.g., TransactSession events)
//!
//! ## Admin Events (48-63)
//! - [`PoolRegisteredEvent`] - Emitted when a pool is registered with the hub
//! - [`AuthorityTransferInitiatedEvent`] - Emitted when authority transfer begins
//! - [`AuthorityTransferCompletedEvent`] - Emitted when authority transfer completes
//! - [`PoolPauseChangedEvent`] - Emitted when pool paused state changes
//! - [`PoolConfigActiveChangedEvent`] - Emitted when pool config active state changes
//! - [`PoolInitializedEvent`] - Emitted when pool is initialized
//!
//! # Event Pattern
//!
//! All events use the panchor `#[event]` macro which:
//! - Implements `Discriminator` trait with the event type discriminator
//! - Implements `Event` trait for event metadata
//! - Derives `Pod` and `Zeroable` for zero-copy serialization
//!
//! Events are emitted via CPI to the Log instruction with a PDA as signer to
//! ensure only valid program invocations can emit events.

// Core events
mod new_commitment;
mod new_nullifier;
mod new_receipt;
mod nullifier_batch_inserted;
mod nullifier_earliest_epoch_advanced;
mod nullifier_epoch_advanced;
mod nullifier_pda_closed;
mod nullifier_epoch_root_closed;
mod nullifier_leaf_inserted;

// Transfer/Escrow events
mod deposit_escrow_closed;
mod deposit_escrow_created;

// Admin events
mod authority_transfer_completed;
mod authority_transfer_initiated;
mod pool_config_active_changed;
mod pool_initialized;
mod pool_paused;
mod pool_registered;

pub use authority_transfer_completed::*;
pub use authority_transfer_initiated::*;
pub use deposit_escrow_closed::*;
pub use deposit_escrow_created::*;
pub use new_commitment::*;
pub use new_nullifier::*;
pub use new_receipt::*;
pub use nullifier_batch_inserted::*;
pub use nullifier_earliest_epoch_advanced::*;
pub use nullifier_epoch_advanced::*;
pub use nullifier_pda_closed::*;
pub use nullifier_epoch_root_closed::*;
pub use nullifier_leaf_inserted::*;
pub use pool_config_active_changed::*;
pub use pool_initialized::*;
pub use pool_paused::*;
pub use pool_registered::*;

use alloc::vec::Vec;
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    cpi::invoke_signed,
    instruction::{AccountMeta, Instruction, Signer},
};

use crate::ID;
use crate::instructions::ShieldedPoolInstruction;

/// Event type discriminators for identifying event types in logs.
///
/// Each event type has a unique u64 discriminator prepended to its serialized data.
/// This allows indexers to identify and parse different event types.
///
/// # Ranges (per discriminator-standard.md)
/// - **1-15**: Core events (commitment, nullifier, receipt)
/// - **16-31**: Transfer events (escrow operations)
/// - **32-47**: State change events (session management)
/// - **48-63**: Admin events (protocol administration)
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::IntoStaticStr)]
pub enum EventType {
    // =========================================================================
    // Core Events (1-15) - Fundamental protocol events
    // =========================================================================
    /// New commitment added to the commitment merkle tree
    NewCommitment = 1,
    /// New nullifier created (input note spent)
    NewNullifier = 2,
    /// New receipt added to the receipt merkle tree
    NewReceipt = 3,
    /// Nullifiers batch inserted into indexed merkle tree (ZK proof verified)
    NullifierBatchInserted = 4,
    // Reserved: 5 (was NullifierInserted, removed - do not reuse)
    /// Nullifier epoch advanced and root snapshot created
    NullifierEpochAdvanced = 6,
    /// Nullifier tree's earliest provable epoch updated (enables garbage collection)
    NullifierEarliestEpochAdvanced = 7,
    /// Per-nullifier leaf data emitted during ZK batch insertion
    NullifierLeafInserted = 8,
    /// Nullifier PDA closed and rent reclaimed (GC)
    NullifierPdaClosed = 9,
    /// Nullifier epoch root PDA closed and rent reclaimed (GC)
    NullifierEpochRootClosed = 10,
    // Reserved: 11-15

    // =========================================================================
    // Transfer Events (16-31) - Escrow operations
    // =========================================================================
    /// Deposit escrow created for relayer-assisted deposits
    DepositEscrowCreated = 16,
    /// Deposit escrow closed and tokens returned
    DepositEscrowClosed = 17,
    // Reserved: 18-31

    // =========================================================================
    // State Change Events (32-47) - Session management
    // =========================================================================
    // Reserved: 32-47 (TransactSession events if needed)

    // =========================================================================
    // Admin Events (48-63) - Protocol administration events
    // =========================================================================
    /// Pool registered with the hub
    PoolRegistered = 48,
    /// Authority transfer initiated (two-step process)
    AuthorityTransferInitiated = 49,
    /// Authority transfer completed (new authority accepted)
    AuthorityTransferCompleted = 50,
    /// Pool paused state changed (emitted for both pause and unpause)
    PoolPauseChanged = 51,
    /// Pool config active state changed for an asset
    PoolConfigActiveChanged = 52,
    /// Pool initialized (genesis event)
    PoolInitialized = 53,
    // Reserved: 54-63
}

/// Emit a panchor event via self-invocation of the Log instruction.
///
/// This function:
/// 1. Serializes the event using `EventBytes::to_event_bytes()` (Pod + discriminator)
/// 2. Builds an instruction to invoke the Log handler
/// 3. Invokes the Log instruction with the global config PDA as signer
///
/// # Arguments
/// * `global_config` - The global config PDA account (used as signer)
/// * `shielded_pool_program` - The shielded pool program account (required for self-CPI)
/// * `signer` - Signer seeds for the global config PDA
/// * `event` - The event to emit (must implement EventBytes)
///
/// # Note
/// The global config PDA signs the log instruction to ensure only valid program
/// invocations can emit events.
pub fn emit_event<T: EventBytes>(
    global_config: &AccountInfo,
    shielded_pool_program: &AccountInfo,
    signer: Signer,
    event: &T,
) -> ProgramResult {
    use borsh::BorshSerialize;

    // Serialize the event using Pod serialization with discriminator
    let event_data = event.to_event_bytes();

    // Build instruction data: [Log discriminator (33), Borsh-serialized data length + data]
    // Format: [discriminator, data_length (u32 le), data...]
    let log_discriminator = ShieldedPoolInstruction::Log as u8;
    let mut instruction_data = Vec::with_capacity(1 + 4 + event_data.len());
    instruction_data.push(log_discriminator);
    // Borsh-serialize the Vec<u8> data (length-prefixed)
    event_data
        .serialize(&mut instruction_data)
        .map_err(|_| pinocchio::program_error::ProgramError::InvalidInstructionData)?;

    // Build instruction for self-CPI to Log
    let instruction = Instruction {
        program_id: &ID,
        accounts: &[AccountMeta::readonly_signer(global_config.key())],
        data: &instruction_data,
    };

    // Invoke the Log instruction with the global config PDA as signer
    // shielded_pool_program is included so the runtime can find the program executable for CPI
    invoke_signed(&instruction, &[global_config, shielded_pool_program], &[signer])?;

    Ok(())
}
