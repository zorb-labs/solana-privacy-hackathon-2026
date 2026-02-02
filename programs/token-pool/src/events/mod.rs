//! Event definitions for the token pool program.
//!
//! Events are emitted via self-invocation of the Log instruction,
//! which allows event data to be recorded in transaction logs without truncation.
//!
//! # Event Types
//!
//! - [`TokenDepositEvent`] - Emitted when tokens are deposited
//! - [`TokenWithdrawalEvent`] - Emitted when tokens are withdrawn
//! - [`TokenRewardsFinalizedEvent`] - Emitted when rewards are finalized
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
use crate::instructions::TokenPoolInstruction;

/// Event type discriminators for identifying event types in logs.
///
/// Each event type has a unique u64 discriminator prepended to its serialized data.
/// This allows indexers to identify and parse different event types.
///
/// # Ranges (per discriminator-standard.md)
/// - **1-15**: Core events (deposit, withdrawal, rewards)
/// - **16-31**: Admin events (reserved for future use)
#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::IntoStaticStr)]
pub enum EventType {
    // =========================================================================
    // Core Events (1-15) - Fundamental protocol events
    // =========================================================================
    /// Token deposit event
    TokenDeposit = 1,
    /// Token withdrawal event
    TokenWithdrawal = 2,
    /// Rewards finalized event
    TokenRewardsFinalized = 3,
    /// Excess tokens swept into rewards
    SweepExcess = 4,
    // Reserved: 5-15

    // =========================================================================
    // Admin Events (16-31) - Reserved for future use
    // =========================================================================
}

/// Event emitted when tokens are deposited into the token pool.
#[event(EventType::TokenDeposit)]
#[repr(C)]
pub struct TokenDepositEvent {
    /// Token mint address
    pub mint: [u8; 32],
    /// Pool balance after deposit
    pub new_balance: u128,
    /// Gross amount deposited (in token base units)
    pub amount: u64,
    /// Protocol fee (in token base units)
    pub fee: u64,
    /// Net amount credited to shielded balance
    pub net_amount: u64,
    /// Solana slot when the deposit occurred
    pub slot: u64,
}

/// Event emitted when tokens are withdrawn from the token pool.
#[event(EventType::TokenWithdrawal)]
#[repr(C)]
pub struct TokenWithdrawalEvent {
    /// Token mint address
    pub mint: [u8; 32],
    /// Pool balance after withdrawal
    pub new_balance: u128,
    /// Gross amount withdrawn (in token base units)
    pub amount: u64,
    /// Protocol fee (in token base units)
    pub fee: u64,
    /// Solana slot when the withdrawal occurred
    pub slot: u64,
    /// Padding for 16-byte alignment
    pub _padding: u64,
}

/// Event emitted when rewards are finalized (accumulator updated).
///
/// **Audit Note:** This event provides unambiguous breakdown of reward sources:
/// - `deposit_fees`: Deposit fees collected since last finalization
/// - `withdrawal_fees`: Withdrawal fees collected since last finalization
/// - `funded_rewards`: External rewards funded via fund_rewards since last finalization
///
/// Indexers can verify: `deposit_fees + withdrawal_fees + funded_rewards` was distributed to the accumulator.
#[event(EventType::TokenRewardsFinalized)]
#[repr(C)]
pub struct TokenRewardsFinalizedEvent {
    /// Token mint address
    pub mint: [u8; 32],
    /// Total pool size at finalization (denominator for reward calculation)
    pub total_pool: u128,
    /// New accumulator value (compare across events for APY calculation)
    pub new_accumulator: u128,
    /// Deposit fees distributed (in token base units)
    pub deposit_fees: u64,
    /// Withdrawal fees distributed (in token base units)
    pub withdrawal_fees: u64,
    /// Funded rewards distributed (external funding, in token base units)
    pub funded_rewards: u64,
    /// Solana slot when finalization occurred
    pub slot: u64,
}

/// Event emitted when excess tokens are swept into pending rewards.
///
/// Excess tokens are tokens that arrived in the vault outside of normal
/// deposit/fund_rewards flows (e.g., direct transfers, airdrops).
#[event(EventType::SweepExcess)]
#[repr(C)]
pub struct SweepExcessEvent {
    /// Token mint address
    pub mint: [u8; 32],
    /// Amount of excess tokens swept into pending rewards
    pub amount: u64,
    /// Solana slot when the sweep occurred
    pub slot: u64,
}

/// Emit a panchor event via self-invocation of the Log instruction.
///
/// This function:
/// 1. Serializes the event using `EventBytes::to_event_bytes()` (Pod + discriminator)
/// 2. Builds an instruction to invoke the Log handler
/// 3. Invokes the Log instruction with the pool config PDA as signer
///
/// # Arguments
/// * `pool_config` - The pool config PDA account (used as signer)
/// * `token_pool_program` - The token-pool program account (required for self-CPI)
/// * `signer` - Signer seeds for the pool config PDA
/// * `event` - The event to emit (must implement EventBytes)
pub fn emit_event<T: EventBytes>(
    pool_config: &AccountInfo,
    token_pool_program: &AccountInfo,
    signer: Signer,
    event: &T,
) -> ProgramResult {
    // Serialize the event using Pod serialization with discriminator
    let event_data = event.to_event_bytes();

    // Build instruction data: [Log discriminator, length (4 bytes LE), data...]
    let log_discriminator = TokenPoolInstruction::Log as u8;
    let len = event_data.len() as u32;
    let mut instruction_data = Vec::with_capacity(1 + 4 + event_data.len());
    instruction_data.push(log_discriminator);
    instruction_data.extend_from_slice(&len.to_le_bytes());
    instruction_data.extend_from_slice(&event_data);

    // Build instruction for self-CPI to Log
    let instruction = Instruction {
        program_id: &ID,
        accounts: &[AccountMeta::readonly_signer(pool_config.key())],
        data: &instruction_data,
    };

    // Invoke the Log instruction with pool config PDA as signer
    // token_pool_program is included so the runtime can find the program executable for CPI
    invoke_signed(&instruction, &[pool_config, token_pool_program], &[signer])?;

    Ok(())
}
