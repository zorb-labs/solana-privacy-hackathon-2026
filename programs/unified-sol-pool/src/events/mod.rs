//! Event definitions for the unified SOL pool program.
//!
//! Events are emitted via self-invocation of the Log instruction,
//! which allows event data to be recorded in transaction logs without truncation.
//!
//! # Event Types
//!
//! - [`UnifiedSolDepositEvent`] - Emitted when SOL/LST is deposited
//! - [`UnifiedSolWithdrawalEvent`] - Emitted when SOL/LST is withdrawn
//! - [`AppreciationHarvestedEvent`] - Emitted when LST appreciation is harvested
//! - [`ExchangeRateUpdatedEvent`] - Emitted when exchange rate is updated
//! - [`UnifiedSolRewardsFinalizedEvent`] - Emitted when rewards are finalized
//!
//! # Event Pattern
//!
//! All events use the panchor `#[event]` macro which:
//! - Implements `Discriminator` trait with the event type discriminator
//! - Implements `Event` trait for event metadata
//! - Derives `Pod` and `Zeroable` for zero-copy serialization

use alloc::vec::Vec;
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    cpi::invoke_signed,
    instruction::{AccountMeta, Instruction, Signer},
};

use crate::ID;
use crate::instructions::UnifiedSolPoolInstruction;

/// Event type discriminators for identifying event types in logs.
///
/// Each event type has a unique u64 discriminator prepended to its serialized data.
/// This allows indexers to identify and parse different event types.
///
/// # Ranges (per discriminator-standard.md)
/// - **1-15**: Core events (deposit, withdrawal, rewards)
/// - **16-31**: LST events (appreciation, rate updates)
/// - **32-47**: Admin events (reserved for future use)
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::IntoStaticStr)]
pub enum EventType {
    // =========================================================================
    // Core Events (1-15) - Fundamental protocol events
    // =========================================================================
    /// SOL/LST deposit event
    UnifiedSolDeposit = 1,
    /// SOL/LST withdrawal event
    UnifiedSolWithdrawal = 2,
    /// Rewards finalized event
    UnifiedSolRewardsFinalized = 3,
    // Reserved: 4-15

    // =========================================================================
    // LST Events (16-31) - LST-specific events
    // =========================================================================
    /// LST appreciation harvested
    AppreciationHarvested = 16,
    /// Exchange rate updated
    ExchangeRateUpdated = 17,
    // Reserved: 18-31

    // =========================================================================
    // Admin Events (32-47) - Reserved for future use
    // =========================================================================
}

/// Event emitted when SOL/LST is deposited into the unified SOL pool.
#[event(EventType::UnifiedSolDeposit)]
#[repr(C)]
pub struct UnifiedSolDepositEvent {
    /// LST mint address (or WSOL marker)
    pub lst_mint: [u8; 32],
    /// LST tokens deposited (in token base units)
    pub lst_amount: u64,
    /// SOL-equivalent value (in lamports)
    pub sol_value: u64,
    /// Protocol fee (in virtual SOL / lamports)
    pub fee: u64,
    /// Exchange rate used (1 LST = rate/1e9 SOL)
    pub exchange_rate: u64,
    /// Solana slot when the deposit occurred
    pub slot: u64,
    /// Padding for 8-byte alignment
    pub _padding: u64,
}

/// Event emitted when SOL/LST is withdrawn from the unified SOL pool.
#[event(EventType::UnifiedSolWithdrawal)]
#[repr(C)]
pub struct UnifiedSolWithdrawalEvent {
    /// LST mint address
    pub lst_mint: [u8; 32],
    /// LST tokens withdrawn (in token base units)
    pub lst_amount: u64,
    /// SOL-equivalent value (in lamports)
    pub sol_value: u64,
    /// Protocol fee (in virtual SOL / lamports)
    pub fee: u64,
    /// Exchange rate used
    pub exchange_rate: u64,
    /// Solana slot when the withdrawal occurred
    pub slot: u64,
    /// Padding for alignment
    pub _padding: u64,
}

/// Event emitted when LST appreciation is harvested.
#[event(EventType::AppreciationHarvested)]
#[repr(C)]
pub struct AppreciationHarvestedEvent {
    /// LST mint address
    pub lst_mint: [u8; 32],
    /// Previous exchange rate (before harvest)
    pub previous_rate: u64,
    /// Current exchange rate (after harvest)
    pub current_rate: u64,
    /// Appreciation amount (in virtual SOL / lamports)
    pub appreciation_amount: u64,
    /// Epoch when harvested
    pub epoch: u64,
    /// Solana slot when harvested
    pub slot: u64,
}

/// Event emitted when exchange rate is updated.
#[event(EventType::ExchangeRateUpdated)]
#[repr(C)]
pub struct ExchangeRateUpdatedEvent {
    /// LST mint address
    pub lst_mint: [u8; 32],
    /// Previous exchange rate
    pub previous_rate: u64,
    /// Current exchange rate
    pub current_rate: u64,
    /// Solana slot when updated
    pub slot: u64,
}

/// Event emitted when rewards are finalized (accumulator updated).
///
/// **Audit Note:** This event provides unambiguous breakdown of reward sources:
/// - `deposit_fees`: Deposit fees collected since last finalization
/// - `withdrawal_fees`: Withdrawal fees collected since last finalization
/// - `appreciation_rewards`: LST appreciation harvested since last finalization
/// - `lst_count`: Confirms all registered LSTs were included in this finalization
///
/// Indexers can verify: `deposit_fees + withdrawal_fees + appreciation_rewards` was distributed to the accumulator.
#[event(EventType::UnifiedSolRewardsFinalized)]
#[repr(C)]
pub struct UnifiedSolRewardsFinalizedEvent {
    /// Total virtual SOL at finalization (denominator for reward calculation)
    pub total_virtual_sol: u128,
    /// New accumulator value (compare across events for APY calculation)
    pub new_accumulator: u128,
    /// Deposit fees distributed (in lamports)
    pub deposit_fees: u64,
    /// Withdrawal fees distributed (in lamports)
    pub withdrawal_fees: u64,
    /// Appreciation rewards distributed (LST yield, in lamports)
    pub appreciation_rewards: u64,
    /// Reward epoch number (increments after each finalization)
    pub epoch: u64,
    /// Solana slot when finalization occurred
    pub slot: u64,
    /// Number of LST configs that were validated and finalized.
    /// Matches UnifiedSolPoolConfig.lst_count - confirms all LSTs were included.
    pub lst_count: u8,
    /// Padding for 16-byte alignment (struct total: 88 bytes)
    pub _padding: [u8; 7],
}

/// Emit a panchor event via self-invocation of the Log instruction.
///
/// This function:
/// 1. Serializes the event using `EventBytes::to_event_bytes()` (Pod + discriminator)
/// 2. Builds an instruction to invoke the Log handler
/// 3. Invokes the Log instruction with the unified config PDA as signer
///
/// # Arguments
/// * `unified_config` - The unified config PDA account (used as signer)
/// * `unified_sol_program` - The unified-sol-pool program account (required for self-CPI)
/// * `signer` - Signer seeds for the unified config PDA
/// * `event` - The event to emit (must implement EventBytes)
pub fn emit_event<T: EventBytes>(
    unified_config: &AccountInfo,
    unified_sol_program: &AccountInfo,
    signer: Signer,
    event: &T,
) -> ProgramResult {
    // Serialize the event using Pod serialization with discriminator
    let event_data = event.to_event_bytes();

    // Build instruction data: [Log discriminator, length (4 bytes LE), data...]
    let log_discriminator = UnifiedSolPoolInstruction::Log as u8;
    let len = event_data.len() as u32;
    let mut instruction_data = Vec::with_capacity(1 + 4 + event_data.len());
    instruction_data.push(log_discriminator);
    instruction_data.extend_from_slice(&len.to_le_bytes());
    instruction_data.extend_from_slice(&event_data);

    // Build instruction for self-CPI to Log
    let instruction = Instruction {
        program_id: &ID,
        accounts: &[AccountMeta::readonly_signer(unified_config.key())],
        data: &instruction_data,
    };

    // Invoke the Log instruction with unified config PDA as signer
    // unified_sol_program is included so the runtime can find the program executable for CPI
    invoke_signed(&instruction, &[unified_config, unified_sol_program], &[signer])?;

    Ok(())
}
