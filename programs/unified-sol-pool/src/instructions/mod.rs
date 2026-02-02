//! Unified SOL pool instruction handlers.
//!
//! Uses panchor's `#[instructions]` macro for automatic dispatch.

use panchor::prelude::*;

// Admin instructions (initialization, configuration, pausing)
pub mod admin;

// Pool operation modules
mod deposit;
mod withdraw;

// Permissionless operations
mod finalize_unified_rewards;
mod harvest_lst_appreciation;
mod log;

// Re-export admin accounts, data, and handlers
pub use admin::*;

// Re-export pool operation accounts and handlers
pub use deposit::{DepositAccounts, process_deposit};
pub use withdraw::{WithdrawAccounts, process_withdraw};

// Re-export permissionless operation accounts and handlers
pub use finalize_unified_rewards::{
    FinalizeUnifiedRewardsAccounts, process_finalize_unified_rewards,
};
pub use harvest_lst_appreciation::{
    HarvestLstAppreciationAccounts, process_harvest_lst_appreciation,
};
pub use log::{LogAccounts, process_log};

/// Unified SOL pool instruction set.
///
/// # Discriminator Ranges (per discriminator-standard.md)
/// - **0-31**: Pool operations (deposit, withdraw) - called via CPI from hub
/// - **64-127**: Config/admin operations (historical - ideally would be 192-255)
///
/// Note: Admin operations use the specialized range (64-127) for historical reasons.
/// New admin instructions should use 192-255 per discriminator-standard.md.
#[instructions]
pub enum UnifiedSolPoolInstruction {
    // =========================================================================
    // Pool Operations (0-31) - Called by hub via CPI
    // =========================================================================
    /// Process a deposit: transfer LST tokens from depositor to vault.
    ///
    /// # Accounts
    /// See `DepositAccounts` for the required accounts.
    #[handler(raw_data, accounts = DepositAccounts)]
    Deposit = 0,

    /// Process a withdrawal: transfer LST tokens from vault to recipient.
    ///
    /// # Accounts
    /// See `WithdrawAccounts` for the required accounts.
    #[handler(raw_data, accounts = WithdrawAccounts)]
    Withdraw = 1,
    // Reserved: 2-31

    // =========================================================================
    // Config/Admin Operations (64-127) - Historical range
    // Note: New admin instructions should use 192-255 per standard
    // =========================================================================
    /// Initialize the unified SOL pool configuration.
    ///
    /// Creates the UnifiedSolPoolConfig PDA.
    #[handler(data)]
    InitUnifiedSolPoolConfig = 64,

    /// Initialize a new LST configuration.
    ///
    /// Creates an LstConfig PDA and its associated vault.
    #[handler(data)]
    InitLstConfig = 65,

    /// Set the active state for the unified SOL pool config.
    #[handler(data)]
    SetUnifiedSolPoolConfigActive = 66,

    /// Set the active state for an LST config.
    #[handler(data)]
    SetLstConfigActive = 67,

    /// Set the fee rates for the unified SOL pool config.
    #[handler(data)]
    SetUnifiedSolPoolConfigFeeRates = 68,

    /// Finalize unified SOL rewards by updating the reward accumulator.
    ///
    /// Permissionless - anyone can call after UPDATE_SLOT_INTERVAL has passed.
    /// Freezes exchange rates atomically with accumulator update.
    FinalizeUnifiedRewards = 69,

    /// Harvest LST appreciation for a specific LST.
    ///
    /// Permissionless. Updates exchange rate and adds appreciation to pending rewards.
    HarvestLstAppreciation = 70,

    /// Log an event via CPI (internal use only).
    ///
    /// This instruction is invoked via CPI from within the program to emit events.
    /// It validates the caller is the program itself via PDA signer.
    #[handler(raw_data, accounts = LogAccounts)]
    Log = 71,
    // Reserved: 72-127

    // =========================================================================
    // Admin Operations (192-255) - For future admin instructions
    // =========================================================================
    /// Initiate two-step authority transfer by setting pending_authority.
    ///
    /// The new authority must call `accept_authority` to complete the transfer.
    TransferAuthority = 192,

    /// Complete two-step authority transfer by accepting pending_authority role.
    ///
    /// Must be called by the `pending_authority` address.
    AcceptAuthority = 193,
}
