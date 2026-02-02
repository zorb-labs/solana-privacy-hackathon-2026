//! Token pool instruction handlers.
//!
//! Uses panchor's `#[instructions]` macro for automatic dispatch.

use panchor::prelude::*;

// Admin instructions (initialization, configuration, pausing)
pub mod admin;

// Pool operation modules
mod deposit;
mod withdraw;

// Permissionless operations
mod finalize_rewards;
mod fund_rewards;
mod log;
mod sweep_excess;

// Re-export admin accounts, data, and handlers
pub use admin::*;

// Re-export pool operation accounts and handlers
pub use deposit::{DepositAccounts, process_deposit};
pub use withdraw::{WithdrawAccounts, process_withdraw};

// Re-export permissionless operation accounts and handlers
pub use finalize_rewards::{FinalizeRewardsAccounts, process_finalize_rewards};
pub use fund_rewards::{FundRewardsAccounts, FundRewardsData, process_fund_rewards};
pub use log::{LogAccounts, process_log};
pub use sweep_excess::{SweepExcessAccounts, process_sweep_excess};

/// Token pool instruction set.
///
/// # Discriminator Ranges (per discriminator-standard.md)
/// - **0-31**: Pool operations (deposit, withdraw) - called via CPI from hub
/// - **64-127**: Config/admin operations (historical - ideally would be 192-255)
///
/// Note: Admin operations use the specialized range (64-127) for historical reasons.
/// New admin instructions should use 192-255 per discriminator-standard.md.
#[instructions]
pub enum TokenPoolInstruction {
    // =========================================================================
    // Pool Operations (0-31) - Called by hub via CPI
    // =========================================================================
    /// Process a deposit: transfer tokens from depositor to vault.
    ///
    /// # Accounts
    /// See `DepositAccounts` for the required accounts.
    #[handler(raw_data, accounts = DepositAccounts)]
    Deposit = 0,

    /// Process a withdrawal: transfer tokens from vault to recipient.
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
    /// Initialize a new token pool.
    ///
    /// Creates pool config and vault accounts.
    #[handler(data)]
    InitPool = 64,

    /// Set the active state for a pool.
    #[handler(data)]
    SetPoolActive = 65,

    /// Set fee rates for a pool.
    #[handler(data)]
    SetFeeRates = 66,

    /// Finalize pending rewards by updating the reward accumulator.
    ///
    /// Permissionless - anyone can call after UPDATE_SLOT_INTERVAL has passed.
    FinalizeRewards = 67,

    /// Fund the reward pool with external tokens.
    ///
    /// Permissionless - anyone can fund rewards via token transfer.
    #[handler(data)]
    FundRewards = 68,

    /// Log an event via CPI (internal use only).
    ///
    /// This instruction is invoked via CPI from within the program to emit events.
    /// It validates the caller is the program itself via PDA signer.
    #[handler(raw_data, accounts = LogAccounts)]
    Log = 69,

    /// Sweep excess tokens from vault into pending rewards.
    ///
    /// Permissionless - anyone can call. Recovers tokens that arrived in the
    /// vault outside of normal deposit/fund_rewards flows (e.g., direct transfers).
    SweepExcess = 70,
    // Reserved: 71-127

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
