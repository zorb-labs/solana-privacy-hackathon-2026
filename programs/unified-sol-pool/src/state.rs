//! Unified SOL pool state accounts.
//!
//! This module contains the configuration accounts for the unified SOL pool:
//! - [`UnifiedSolPoolConfig`]: Master configuration for the unified SOL pool
//! - [`LstConfig`]: Per-LST configuration and state

use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;
use zorb_pool_interface::authority::HasAuthority;
use zorb_pool_interface::{BASIS_POINTS, tokens_to_virtual_sol, virtual_sol_to_tokens};

// ============================================================================
// Constants
// ============================================================================

/// Unified SOL asset ID - a protocol-defined constant with no preimage.
///
/// This is intentionally NOT a Poseidon hash of any token mint. Using a constant
/// with no preimage ensures:
/// 1. Clear distinction from token-derived asset IDs (which use poseidon(mint))
/// 2. No collision with any existing or future token's asset ID
/// 3. Obvious identification in logs/debugging (value 1 vs random-looking hash)
///
/// The value 1 (as a 256-bit big-endian integer) is used.
pub const UNIFIED_SOL_ASSET_ID: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];

// ============================================================================
// Account Type Enum
// ============================================================================

/// Account discriminators for the Unified SOL Pool program.
///
/// Each discriminator uniquely identifies an account type. The discriminator
/// is stored as the first 8 bytes of account data.
///
/// # Ranges (per discriminator-standard.md)
/// - **0-15**: Core accounts (config singletons)
/// - **16-31**: User accounts (reserved for future use)
/// - **32-63**: Tree accounts (reserved for future use)
/// - **64-127**: Ephemeral accounts (reserved for future use)
#[account_type]
pub enum UnifiedSolPoolAccount {
    // =========================================================================
    // Core Accounts (0-15) - Configuration accounts
    // =========================================================================
    /// Master configuration for the unified SOL pool
    UnifiedSolPoolConfig = 0,
    /// Per-LST configuration and state
    LstConfig = 1,
    // Reserved: 2-15

    // =========================================================================
    // User Accounts (16-31) - Reserved for future use
    // =========================================================================

    // =========================================================================
    // Tree Accounts (32-63) - Reserved for future use
    // =========================================================================

    // =========================================================================
    // Ephemeral Accounts (64-127) - Reserved for future use
    // =========================================================================
}

// ============================================================================
// Pool Type
// ============================================================================

/// Pool type for exchange rate parsing.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolType {
    /// Wrapped SOL (WSOL) - 1:1 rate
    Wsol = 0,
    /// SPL Stake Pool (Jito, Sanctum, etc.)
    SplStakePool = 1,
    /// Marinade Stake Pool
    Marinade = 2,
    /// Lido Stake Pool
    Lido = 3,
}

impl PoolType {
    /// Convert from u8 to PoolType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Wsol),
            1 => Some(Self::SplStakePool),
            2 => Some(Self::Marinade),
            3 => Some(Self::Lido),
            _ => None,
        }
    }
}

// ============================================================================
// UnifiedSolPoolConfig
// ============================================================================

/// Master configuration for unified SOL pool.
///
/// Seeds: ["unified_sol_pool"]
///
/// # Overview
///
/// This account enables users to deposit/withdraw any SOL-equivalent (WSOL, vSOL,
/// jitoSOL, etc.) while the protocol treats them as a single fungible asset.
/// The protocol manages rebalancing and converts LST appreciation into rewards.
///
/// # Asset ID
///
/// All SOL-equivalents share a single `asset_id = UNIFIED_SOL_ASSET_ID` in the circuit.
/// This is a protocol-defined constant (value 1), not derived from any mint.
/// This means users can deposit vSOL and later withdraw WSOL (subject to liquidity).
///
/// # Reward Accumulator Design
///
/// The accumulator tracks rewards per unit of virtual SOL. Rewards are finalized
/// every `UPDATE_SLOT_INTERVAL` slots (~5 minutes).
///
/// ## State Fields
///
/// - `finalized_balance`: Balance frozen at last finalization. NOT modified between finalizations.
/// - `pending_deposits`: New deposits since last finalization (additive only).
/// - `pending_withdrawals`: Withdrawals since last finalization (additive only).
/// - `pending_deposit_fees`: Deposit fees collected since last finalization.
/// - `pending_withdrawal_fees`: Withdrawal fees collected since last finalization.
/// - `pending_appreciation`: LST appreciation collected since last finalization.
/// - `reward_accumulator`: Cumulative rewards per unit, scaled by 1e18.
///
/// ## Key Properties
///
/// 1. **Free deposits/withdrawals**: Between finalizations, deposits and withdrawals only
///    modify `pending_deposits` and `pending_withdrawals`. They never touch
///    `finalized_balance`, so underflow is impossible during transactions.
///
/// 2. **Withdrawers receive rewards**: Since `finalized_balance` is frozen and includes
///    funds being withdrawn, withdrawers receive their proportional share of rewards.
///
/// 3. **Single validation point**: The invariant `pending_withdrawals <= finalized_balance + pending_deposits`
///    is checked only at finalization, not during individual transactions.
///
/// ## Finalization Flow
///
/// On `finalize_rewards(current_slot)`:
/// 1. Check enough slots elapsed since `last_finalized_slot`
/// 2. Calculate `total_pool = finalized_balance + pending_deposits - pending_withdrawals`
/// 3. If rewards exist: `accumulator += (pending_rewards * 1e18) / total_pool`
/// 4. Update: `finalized_balance = total_pool`
/// 5. Reset: `pending_deposits = 0`, `pending_withdrawals = 0`, `pending_rewards = 0`
///
/// ## Example
///
/// ```text
/// After finalization: finalized_balance = 1000e9 (1000 virtual SOL)
/// User A deposits 500 vSOL:  pending_deposits = 500e9
/// User B withdraws 200:      pending_withdrawals = 200e9
/// LST appreciation + fees:   pending_rewards = 50e9
///
/// On finalize_rewards:
///   total_pool = 1000e9 + 500e9 - 200e9 = 1300e9
///   reward_delta = 50e9 * 1e18 / 1300e9 ≈ 38,461,538,461,538,461
///   finalized_balance = 1300e9
/// ```
///
/// To compute a user's reward in circuit:
/// ```text
/// user_reward = user_amount * (current_accumulator - entry_accumulator) / 1e18
/// ```
#[account(UnifiedSolPoolAccount::UnifiedSolPoolConfig)]
#[repr(C)]
pub struct UnifiedSolPoolConfig {
    // === Identity ===
    /// The canonical asset ID - protocol-defined constant.
    /// Value: [0x00...0x01] (the number 1 as big-endian 256-bit integer)
    pub asset_id: [u8; 32],

    /// Authority that can update config and perform admin operations
    pub authority: Pubkey,

    /// Pending authority for two-step transfer.
    /// Set by `transfer_authority`, must call `accept_authority` to complete.
    pub pending_authority: Pubkey,

    /// Current reward epoch. Increments on each `finalize_unified_rewards`.
    ///
    /// # AUDIT: Epoch Model
    ///
    /// **Initialization:** Starts at 1, not 0. Epoch 0 is reserved as "uninitialized".
    ///
    /// **Lifecycle per epoch N:**
    /// 1. `harvest_lst_appreciation`: sets `LstConfig::last_harvest_epoch = N`
    /// 2. `finalize_unified_rewards`: validates all LSTs have `last_harvest_epoch == N`,
    ///    freezes `harvested_exchange_rate`, then increments `reward_epoch` to N+1
    /// 3. `execute_transact`: uses `accumulator_epoch = N+1`, checks
    ///    `last_harvest_epoch == N+1 - 1 == N`
    ///
    /// **Why start at 1?** New LSTs have `last_harvest_epoch = 0`. If `reward_epoch`
    /// also started at 0, the finalize check `last_harvest_epoch == reward_epoch`
    /// would pass for unharvested LSTs (0 == 0), allowing dummy exchange rates.
    ///
    /// See: `init_unified_sol_pool_config.rs` for initialization
    pub reward_epoch: u64,

    /// Reserved for future use (using u64 array since bytemuck doesn't support [u8; 56])
    pub _reserved1: [u64; 7],

    // === Virtual SOL Tracking ===
    /// Total virtual SOL value across all LST vaults.
    ///
    /// **Units:** Lamports (1e9 per SOL)
    ///
    /// **Formula:** Sum of `vault_balance × harvested_exchange_rate / RATE_PRECISION` for all LSTs
    ///
    /// # Update Policy
    ///
    /// Updated incrementally during:
    /// - `deposit`: += virtual_sol (value of deposited tokens at harvested_exchange_rate)
    /// - `withdraw`: -= net_virtual_sol (value of tokens leaving vault)
    ///
    /// Recalibrated during:
    /// - `finalize_unified_rewards`: set to `Σ(LstConfig.total_virtual_sol)`
    ///
    /// # Important
    ///
    /// This value tracks pool value at the `harvested_exchange_rate` (frozen rate).
    /// It does NOT reflect real-time market value (which uses `exchange_rate`).
    /// Used for:
    /// - Buffer calculations (via `calculate_required_buffer()`)
    /// - Informational/display purposes
    pub total_virtual_sol: u128,

    // === Reward Accumulator ===
    /// Cumulative rewards per unit of deposit, scaled by 1e18.
    ///
    /// **Units:** Scaled fixed-point (rewards * 1e18 / deposits)
    ///
    /// **Updates on:** `finalize_rewards()` only
    ///
    /// To compute user rewards in circuit:
    /// `user_reward = user_amount * (current_acc - entry_acc) / 1e18`
    pub reward_accumulator: u128,

    /// Slot when rewards were last finalized.
    ///
    /// **Updates on:** `finalize_rewards()` only
    pub last_finalized_slot: u64,

    /// Pending deposit fees waiting to be distributed on next finalization.
    ///
    /// **Units:** Virtual SOL (lamports, 1e9 per SOL)
    ///
    /// **Important:** Fee rewards are denominated in virtual SOL, not LST tokens.
    /// When a user pays a 1% fee on a 100 virtual SOL deposit, 1 virtual SOL
    /// is added to `pending_deposit_fees`, regardless of the underlying LST type.
    ///
    /// **Updates on:**
    /// - Deposit fees: += fee_amount (virtual SOL)
    /// - `finalize_rewards()`: reset to 0 after distribution
    pub pending_deposit_fees: u64,

    /// Pending withdrawal fees waiting to be distributed on next finalization.
    ///
    /// **Units:** Virtual SOL (lamports, 1e9 per SOL)
    ///
    /// **Important:** Fee rewards are denominated in virtual SOL, not LST tokens.
    /// When a user pays a 1% fee on a 100 virtual SOL withdrawal, 1 virtual SOL
    /// is added to `pending_withdrawal_fees`, regardless of the underlying LST type.
    ///
    /// **Updates on:**
    /// - Withdrawal fees: += fee_amount (virtual SOL)
    /// - `finalize_rewards()`: reset to 0 after distribution
    ///
    /// **Note:** These fee fields (deposit + withdrawal) contain only transaction fees.
    /// LST appreciation is tracked separately in `pending_appreciation`. At finalization,
    /// all three are combined and distributed via the accumulator.
    pub pending_withdrawal_fees: u64,

    /// Pending LST appreciation rewards waiting to be distributed.
    ///
    /// **Units:** Virtual SOL (lamports, 1e9 per SOL)
    ///
    /// **Important:** Appreciation is denominated in virtual SOL. When an LST's
    /// exchange rate increases from 1.05 to 1.06, the virtual SOL value of the
    /// pool's holdings increases. This delta (valued at the `harvested_exchange_rate`)
    /// is added to `pending_appreciation`.
    ///
    /// **Updates on:**
    /// - `add_appreciation()`: += amount (from harvest_lst_appreciation)
    /// - `finalize_rewards()`: reset to 0 after distribution
    ///
    /// **Audit Note:** This separation enables the finalization event to report
    /// fee_rewards vs appreciation_rewards unambiguously, providing transparency
    /// on yield sources for indexers and users.
    pub pending_appreciation: u64,

    // Note: With 4 × u64 fields above (last_finalized_slot, pending_deposit_fees,
    // pending_withdrawal_fees, pending_appreciation) = 32 bytes, we're aligned for u128.
    // The previous _pad_appreciation was removed when pending_withdrawal_fees was added.

    // === Balance Tracking ===
    /// Balance frozen at last finalization (not modified between finalizations).
    ///
    /// **Units:** Lamports (1e9 per SOL) - virtual SOL value
    ///
    /// **Updates on:** `finalize_rewards()` only
    ///
    /// This is the denominator used to compute reward-per-unit during finalization.
    /// Between finalizations, only pending_deposits/pending_withdrawals change.
    pub finalized_balance: u128,

    /// Deposits since last finalization, for accumulator calculation only.
    ///
    /// **Units:** Lamports (1e9 per SOL) - virtual SOL value
    ///
    /// **Purpose:** Used solely in `finalize_rewards()` to compute the reward
    /// accumulator denominator: `total_pool = finalized_balance + pending_deposits - pending_withdrawals`.
    /// This is NOT the source of truth for pool balance (vault balances are).
    ///
    /// **Value tracked:** Principal (virtual_sol - fee). This equals the user's ZK
    /// commitment value. Fees are tracked separately in `pending_deposit_fees`.
    ///
    /// **Updates on:**
    /// - Deposit instruction: += principal
    /// - `finalize_rewards()`: reset to 0
    pub pending_deposits: u128,

    /// Withdrawals since last finalization, for accumulator calculation only.
    ///
    /// **Units:** Lamports (1e9 per SOL) - virtual SOL value
    ///
    /// **Purpose:** Used solely in `finalize_rewards()` to compute the reward
    /// accumulator denominator: `total_pool = finalized_balance + pending_deposits - pending_withdrawals`.
    /// This is NOT the source of truth for pool balance (vault balances are).
    ///
    /// **Value tracked:** Commitment value being burned (params.amount in virtual SOL).
    /// This is the full amount before withdrawal fee. Fees are tracked separately in
    /// `pending_withdrawal_fees`.
    ///
    /// **Updates on:**
    /// - Withdraw instruction: += virtual_sol
    /// - `finalize_rewards()`: reset to 0
    pub pending_withdrawals: u128,

    // === Fee Configuration ===
    /// Deposit fee rate in basis points (e.g., 100 = 1%)
    pub deposit_fee_rate: u16,

    /// Withdrawal fee rate in basis points (e.g., 100 = 1%)
    pub withdrawal_fee_rate: u16,

    // === Buffer Configuration ===
    /// Minimum WSOL buffer as basis points (e.g., 2000 = 20%)
    pub min_buffer_bps: u16,

    /// Explicit padding for u64 alignment
    pub _pad1: [u8; 2],

    /// Minimum absolute WSOL buffer amount in lamports
    pub min_buffer_amount: u64,

    // === Status ===
    /// Whether the unified pool is active (1 = active, 0 = paused)
    pub is_active: u8,

    /// PDA bump seed
    pub bump: u8,

    /// Explicit padding for u128 alignment
    pub _pad2: [u8; 14],

    // === Statistics ===
    /// Total deposits over lifetime (unified SOL)
    pub total_deposited: u128,

    /// Total withdrawals over lifetime (unified SOL)
    pub total_withdrawn: u128,

    /// Total rewards distributed over lifetime
    pub total_rewards_distributed: u128,

    /// Total deposit fees collected (in virtual SOL)
    pub total_deposit_fees: u128,

    /// Total withdrawal fees collected (in virtual SOL)
    pub total_withdrawal_fees: u128,

    /// Reserved for future use (maintains struct alignment)
    pub _reserved_stats: u128,

    /// Total LST appreciation harvested across all LSTs (in virtual SOL)
    pub total_appreciation: u128,

    /// Maximum deposit amount per transaction (in virtual SOL, 0 = no limit)
    pub max_deposit_amount: u64,

    /// Number of deposit transactions
    pub deposit_count: u64,

    /// Number of withdrawal transactions
    pub withdrawal_count: u64,

    /// Number of registered LST configs
    pub lst_count: u8,

    /// Reserved for future use (23 bytes for 16-byte struct alignment)
    /// Note: Increased from 15 to 23 bytes after removing transfer_count (u64 = 8 bytes)
    pub _reserved: [u8; 23],
}

impl UnifiedSolPoolConfig {
    /// Account size
    pub const SIZE: usize = core::mem::size_of::<Self>();

    /// Minimum number of slots between accumulator updates (~18 minutes at 400ms slots).
    ///
    /// This interval determines the "proof validity window" - the minimum time
    /// users have to submit a ZK proof after finalization before the accumulator
    /// can change again.
    ///
    /// **Gates**: In addition to the slot interval, finalization requires:
    /// - All registered LSTs must be harvested in the current epoch
    /// - Pool must be active (not paused)
    ///
    /// The actual interval may be longer if no one calls `finalize_unified_rewards`
    /// or if LSTs haven't been harvested. This instruction is permissionless.
    ///
    /// At 400ms/slot: 2700 slots ≈ 18 minutes ≈ ~3 finalizations per hour (max)
    pub const UPDATE_SLOT_INTERVAL: u64 = 2700;

    /// Precision multiplier for accumulator calculations (1e18)
    ///
    /// This scaling factor preserves precision when computing rewards per unit.
    /// Without scaling, `pending_rewards / total_pool` would truncate to 0
    /// whenever rewards < pool size (the common case).
    ///
    /// Uses 1e18 for compatibility with standard token decimals:
    /// - Fits cleanly in BN254 field element (~254 bits available)
    /// - pending_rewards (u64) * 1e18 fits in u128
    ///
    /// Circuit reward calculation:
    /// ```text
    /// user_reward = user_amount * (current_acc - entry_acc) / 1e18
    /// ```
    ///
    /// The circuit MUST use the same precision constant.
    pub const ACCUMULATOR_PRECISION: u128 = 1_000_000_000_000_000_000;

    /// Calculate the PDA address for unified SOL pool config (singleton)
    /// Seeds: ["unified_sol_pool"]
    ///
    /// Note: Ignores program_id parameter - uses crate::ID
    pub fn find_pda(_program_id: &Pubkey) -> (Pubkey, u8) {
        crate::find_unified_sol_pool_config_pda()
    }

    /// Check if the pool is active
    pub fn is_active(&self) -> bool {
        self.is_active != 0
    }

    /// Get current balance (finalized_balance + pending_deposits - pending_withdrawals)
    pub fn current_balance(&self) -> Result<u128, crate::UnifiedSolPoolError> {
        self.finalized_balance
            .checked_add(self.pending_deposits)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?
            .checked_sub(self.pending_withdrawals)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)
    }

    /// Record a deposit (in unified SOL terms).
    ///
    /// Note: Does NOT update `total_virtual_sol` - that is calculated at finalization
    /// by summing LstConfig.total_virtual_sol values across all LSTs.
    pub fn record_deposit(
        &mut self,
        unified_sol: u128,
    ) -> Result<(), crate::UnifiedSolPoolError> {
        self.pending_deposits = self
            .pending_deposits
            .checked_add(unified_sol)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        self.total_deposited = self
            .total_deposited
            .checked_add(unified_sol)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Record a withdrawal (in unified SOL terms).
    ///
    /// Note: Does NOT update `total_virtual_sol` - that is calculated at finalization
    /// by summing LstConfig.total_virtual_sol values across all LSTs.
    pub fn record_withdrawal(
        &mut self,
        unified_sol: u128,
    ) -> Result<(), crate::UnifiedSolPoolError> {
        self.pending_withdrawals = self
            .pending_withdrawals
            .checked_add(unified_sol)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        self.total_withdrawn = self
            .total_withdrawn
            .checked_add(unified_sol)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    }

    /// Calculate the required WSOL buffer.
    /// Returns max(percentage-based, absolute minimum).
    ///
    /// Uses `total_virtual_sol` which is tracked at `harvested_exchange_rate`.
    /// This is intentional - the buffer sees the same stale rate as deposits/withdrawals,
    /// maintaining consistency with the economic barrier design.
    pub fn calculate_required_buffer(&self) -> Result<u64, crate::UnifiedSolPoolError> {
        let percentage_buffer = self
            .total_virtual_sol
            .checked_mul(self.min_buffer_bps as u128)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?
            / BASIS_POINTS as u128;
        Ok(core::cmp::max(percentage_buffer as u64, self.min_buffer_amount))
    }

    /// Finalize pending rewards by updating the reward accumulator.
    ///
    /// This distributes pending rewards by updating the accumulator, then
    /// updates the finalized balance.
    ///
    /// # Returns
    /// - `Ok(true)` if rewards were finalized
    /// - `Ok(false)` if slot interval hasn't passed yet
    /// - `Err` if arithmetic overflow occurs
    ///
    /// # AUDIT: Reward Distribution Denominator (Harvest-Finalize Timing Safety)
    ///
    /// The denominator uses `current_balance()` which INCLUDES `pending_deposits`.
    /// This is intentional and correct. Using `finalized_balance` alone would cause
    /// over-distribution (insolvency):
    ///
    /// **Conservation property:** `total_claimed = delta × total_pool = total_pending`.
    /// If we used `finalized_balance` instead, `delta` would be larger, and
    /// `total_claimed = delta × (finalized_balance + pending_deposits) > total_pending`.
    ///
    /// **Why new deposits receiving the next delta is safe:**
    /// Deposits between harvest and finalize enter at the stale `harvested_exchange_rate`
    /// (frozen at previous finalization). The cost of entering at this stale rate
    /// exceeds the captured yield for any realistic per-epoch rate change:
    ///
    /// ```text
    /// Cost of stale entry:  tokens × (current_rate - harvested_rate) / RATE_PRECISION
    /// Captured yield:       virtual_sol × pending_appreciation / total_pool
    ///
    /// For rate_delta = d:
    ///   cost  = tokens × d / 1e9
    ///   yield = (tokens × harvested_rate / 1e9) × (vault × d / 1e9) / total_pool
    ///         ≈ tokens × d / 1e9 × (vault / total_pool)
    ///
    /// Since vault/total_pool ≤ 1, cost ≥ yield always holds.
    /// ```
    ///
    /// This is the same pattern as Compound V2's cToken model: the frozen exchange
    /// rate between accruals means no profitable timing attack exists.
    ///
    /// **Bounded by MAX_RATE_CHANGE_BPS (50 = 0.5%):** Even in the worst case,
    /// the maximum per-epoch rate change is 0.5%, making any residual MEV negligible.
    ///
    /// See also:
    /// - `deposit.rs`: uses `harvested_exchange_rate` (stale rate = entry cost)
    /// - `harvest_lst_appreciation.rs`: `validate_rate_change()` enforces MAX_RATE_CHANGE_BPS
    /// - `finalize_unified_rewards.rs`: atomically freezes rates with accumulator update
    pub fn finalize_rewards(
        &mut self,
        current_slot: u64,
    ) -> Result<bool, crate::UnifiedSolPoolError> {
        // Check if enough slots have passed
        if current_slot < self.last_finalized_slot + Self::UPDATE_SLOT_INTERVAL {
            return Ok(false);
        }

        // AUDIT: CONSERVATION - Uses current_balance() (includes pending_deposits) as
        // denominator. This ensures: delta × total_pool == total_pending (zero-sum).
        // New deposits dilute the per-unit reward but total distribution is conserved.
        // The stale harvested_exchange_rate provides the economic barrier against
        // timing attacks (see method-level doc comment above).
        let total_pool = self.current_balance()?;

        // Combine deposit fees, withdrawal fees, and appreciation rewards
        let total_pending = (self.pending_deposit_fees as u128)
            .checked_add(self.pending_withdrawal_fees as u128)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?
            .checked_add(self.pending_appreciation as u128)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;

        // Update accumulator if there are deposits and pending rewards
        if total_pool > 0 && total_pending > 0 {
            // Delta = total_pending * 1e18 / total_pool
            let delta = total_pending
                .checked_mul(Self::ACCUMULATOR_PRECISION)
                .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?
                .checked_div(total_pool)
                .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;

            self.reward_accumulator = self
                .reward_accumulator
                .checked_add(delta)
                .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;

            self.total_rewards_distributed = self
                .total_rewards_distributed
                .checked_add(total_pending)
                .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;

            // Reset all reward tracking fields
            self.pending_deposit_fees = 0;
            self.pending_withdrawal_fees = 0;
            self.pending_appreciation = 0;
        }

        // Update finalized balance
        self.finalized_balance = self.current_balance()?;
        self.pending_deposits = 0;
        self.pending_withdrawals = 0;
        self.last_finalized_slot = current_slot;
        self.reward_epoch = self
            .reward_epoch
            .checked_add(1)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;

        Ok(true)
    }

    /// Add appreciation to pending appreciation rewards and track total appreciation.
    ///
    /// This is called by `harvest_lst_appreciation` when LST exchange rates increase.
    /// The appreciation is tracked separately from fees to enable transparent
    /// reporting in finalization events.
    pub fn add_appreciation(&mut self, amount: u64) -> Result<(), crate::UnifiedSolPoolError> {
        self.pending_appreciation = self
            .pending_appreciation
            .checked_add(amount)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        self.total_appreciation = self
            .total_appreciation
            .checked_add(amount as u128)
            .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        Ok(())
    }
}

impl HasAuthority for UnifiedSolPoolConfig {
    fn authority(&self) -> &Pubkey {
        &self.authority
    }
    fn authority_mut(&mut self) -> &mut Pubkey {
        &mut self.authority
    }
    fn pending_authority(&self) -> &Pubkey {
        &self.pending_authority
    }
    fn pending_authority_mut(&mut self) -> &mut Pubkey {
        &mut self.pending_authority
    }
}

// ============================================================================
// LstConfig
// ============================================================================

/// Per-LST configuration and state.
///
/// Seeds: ["lst_config", lst_mint]
///
/// # Overview
///
/// Each LstConfig tracks a single LST type within the unified SOL pool.
/// It manages exchange rate tracking and virtual SOL value calculation.
///
/// Note: UnifiedSolPoolConfig is a singleton, so it's not stored here. The relationship
/// is implicit - all LstConfigs belong to the single UnifiedSolPoolConfig.
///
/// # Exchange Rate
///
/// The exchange rate is stored as `1 LST = exchange_rate/1e9 SOL`.
/// For example:
/// - Rate of 1_050_000_000 means 1 vSOL = 1.05 SOL
/// - Rate of 980_000_000 means 1 LST = 0.98 SOL (discount)
///
/// # Balance Derivation
///
/// The LST balance is NOT stored in this account. It's always derived by reading
/// the `lst_vault` token account balance. This ensures consistency and avoids
/// the possibility of balance desync.
///
/// # Appreciation
///
/// When the exchange rate increases:
/// ```text
/// old_value = vault_balance * old_rate / 1e9
/// new_value = vault_balance * new_rate / 1e9
/// appreciation = new_value - old_value
/// ```
/// This appreciation is added to UnifiedSolPoolConfig.pending_rewards via harvest_lst_appreciation.
#[account(UnifiedSolPoolAccount::LstConfig)]
#[repr(C)]
pub struct LstConfig {
    // =========================================================================
    // COMMON FIELDS (all pool types) - 232 bytes
    // =========================================================================
    // These fields are used by both WSOL and stake pool configs.
    // Variable decoders can read this prefix and stop here for WSOL.

    // === Header (8 bytes) ===
    /// Pool type discriminator (0 = Wsol, 1 = SplStakePool, 2 = Marinade, 3 = Lido)
    /// FIRST field for instant pool type discrimination in variable decoders.
    pub pool_type: u8,

    /// Whether this LST is enabled for operations (1 = active)
    pub is_active: u8,

    /// PDA bump seed
    pub bump: u8,

    /// Padding for 8-byte alignment
    pub _header_pad: [u8; 5],

    // === Common References (64 bytes) ===
    /// LST token mint (e.g., WSOL, jitoSOL)
    pub lst_mint: Pubkey,

    /// PDA token account holding this LST
    /// Seeds: ["lst_vault", lst_config]
    pub lst_vault: Pubkey,

    // === Exchange Rate State (40 bytes) ===
    /// Current exchange rate: 1 LST = exchange_rate/1e9 SOL
    /// For WSOL, always RATE_PRECISION (1:1).
    pub exchange_rate: u64,

    /// Exchange rate used for deposits/withdrawals (frozen at finalize time).
    ///
    /// Updated by `finalize_unified_rewards` to equal `exchange_rate` at that moment.
    /// This "freezes" the rate so ZK proofs can use a consistent value.
    ///
    /// # AUDIT: Dual Role as Economic Barrier
    ///
    /// Between harvest and finalize, this rate is STALE (lower than `exchange_rate`).
    /// This means depositors receive fewer virtual SOL than their tokens' market value.
    /// This stale-rate cost is the natural economic barrier that prevents profitable
    /// harvest-finalize timing attacks on the reward accumulator.
    ///
    /// DO NOT update this field during harvest — it must only change at finalize
    /// (atomically with the accumulator update) per INV-8.
    pub harvested_exchange_rate: u64,

    /// Slot when exchange rate was last updated
    pub last_rate_update_slot: u64,

    /// Reward epoch when this LST was last harvested.
    ///
    /// # AUDIT: Epoch Model
    ///
    /// **Initialization:** Starts at 0, meaning "never harvested".
    ///
    /// **Updates:** `harvest_lst_appreciation` sets this to `reward_epoch`.
    ///
    /// **Validation points:**
    /// - `finalize_unified_rewards`: requires `last_harvest_epoch == reward_epoch`
    /// - `execute_transact` (shielded-pool): requires `last_harvest_epoch == accumulator_epoch - 1`
    ///
    /// **Why 0 means "never harvested"?** Since `reward_epoch` starts at 1, a newly
    /// initialized LST with `last_harvest_epoch = 0` will fail the finalize check
    /// (0 != 1) until it's actually harvested.
    pub last_harvest_epoch: u64,

    /// Padding to align `total_virtual_sol` (u128) to 16-byte boundary.
    pub _pad_for_u128: u64,

    // === Value Tracking (40 bytes) ===
    /// Total virtual SOL value of this LST's vault.
    ///
    /// **Units:** Lamports (1e9 per SOL)
    ///
    /// **Formula:** `vault_token_balance × harvested_exchange_rate / RATE_PRECISION`
    ///
    /// Updated incrementally during deposit/withdraw, recalculated atomically at finalize.
    pub total_virtual_sol: u128,

    /// Token balance counter for this LST's vault.
    ///
    /// - **Deposit**: += token_amount
    /// - **Withdraw**: -= output_tokens
    /// - **Finalize**: used to calculate total_virtual_sol = counter × exchange_rate
    ///
    /// Note: External transfers directly to vault are not tracked (free equity to pool).
    pub vault_token_balance: u64,

    /// Padding for u128 alignment
    pub _value_pad: u64,

    // === Statistics (80 bytes) ===
    /// Total LST tokens deposited (in token base units)
    pub total_deposited: u128,

    /// Total LST tokens withdrawn (in token base units)
    pub total_withdrawn: u128,

    /// Total appreciation harvested from this LST (0 for WSOL)
    pub total_appreciation_harvested: u64,

    /// Number of deposits into this LST
    pub deposit_count: u64,

    /// Number of withdrawals from this LST
    pub withdrawal_count: u64,

    /// Padding for alignment
    pub _stat_pad: u64,

    // =========================================================================
    // STAKE POOL SPECIFIC FIELDS - 72 bytes
    // =========================================================================
    // These fields are only used by stake pool configs (SplStakePool, Marinade, Lido).
    // For WSOL, these are zeroed and can be skipped by variable decoders.

    /// Stake pool address for this LST (for rate queries)
    /// Zeroed for WSOL.
    pub stake_pool: Pubkey,

    /// Program ID for the stake pool
    /// Zeroed for WSOL.
    pub stake_pool_program: Pubkey,

    /// Previous exchange rate (for appreciation calculation)
    /// Zeroed for WSOL (no appreciation).
    pub previous_exchange_rate: u64,

    // =========================================================================
    // RESERVED - 8 bytes
    // =========================================================================
    /// Reserved for future use
    pub _reserved: u64,
}

impl LstConfig {
    /// Account size
    pub const SIZE: usize = core::mem::size_of::<Self>();

    /// Exchange rate precision (1e9)
    pub const RATE_PRECISION: u64 = 1_000_000_000;

    /// Calculate the PDA address for an LST config
    /// Seeds: ["lst_config", lst_mint]
    ///
    /// Note: Ignores program_id parameter - uses crate::ID
    pub fn find_pda(_program_id: &Pubkey, lst_mint: &Pubkey) -> (Pubkey, u8) {
        crate::find_lst_config_pda(lst_mint)
    }

    /// Check if the LST is active
    pub fn is_active(&self) -> bool {
        self.is_active != 0
    }

    /// Calculate the current SOL value of a given LST balance.
    /// Uses the harvested_exchange_rate for consistency.
    /// Implements φ(e) = e × λ / ρ
    pub fn calculate_virtual_sol(&self, lst_balance: u64) -> u128 {
        tokens_to_virtual_sol(lst_balance, self.harvested_exchange_rate).unwrap_or(0)
    }

    /// Calculate LST tokens from virtual SOL amount.
    /// Uses the harvested_exchange_rate for consistency.
    /// Implements φ⁻¹(s) = s × ρ / λ
    pub fn calculate_lst_tokens(&self, virtual_sol: u64) -> u64 {
        virtual_sol_to_tokens(virtual_sol, self.harvested_exchange_rate).unwrap_or(0)
    }

    /// Maximum rate change allowed per harvest (50 basis points = 0.5%).
    ///
    /// # AUDIT: Arbitrage Bound
    ///
    /// This constant bounds the maximum harvest-finalize timing arbitrage window.
    /// Any deposit between harvest and finalize captures at most:
    ///   `virtual_sol × (appreciation / total_pool)`
    /// where appreciation ≤ `vault_balance × MAX_RATE_CHANGE_BPS / 10000`.
    ///
    /// Since the depositor pays the stale-rate cost of:
    ///   `tokens × rate_delta / RATE_PRECISION`
    /// and this cost ≥ captured yield (see `finalize_rewards` doc), the
    /// attack is unprofitable. This constant ensures the theoretical maximum
    /// MEV per epoch is bounded to 0.5% of vault value, which combined with
    /// the stale-rate cost, remains economically irrelevant.
    pub const MAX_RATE_CHANGE_BPS: u64 = 50;

    /// Validate that a new exchange rate is within acceptable bounds.
    pub fn validate_rate_change(&self, new_rate: u64) -> Result<(), crate::UnifiedSolPoolError> {
        if self.exchange_rate == 0 {
            return Ok(()); // First rate update, allow anything
        }

        // Calculate maximum allowed change (0.5% = 50 bps)
        let max_change = self.exchange_rate / 200; // 0.5%

        let diff = new_rate.abs_diff(self.exchange_rate);

        if diff > max_change {
            return Err(crate::UnifiedSolPoolError::InvalidExchangeRate);
        }

        Ok(())
    }

    /// Update the exchange rate and calculate appreciation.
    ///
    /// Returns the appreciation amount (in virtual SOL).
    pub fn update_exchange_rate(
        &mut self,
        vault_balance: u64,
        new_rate: u64,
        current_slot: u64,
    ) -> Result<u64, crate::UnifiedSolPoolError> {
        let old_rate = self.exchange_rate;
        self.previous_exchange_rate = old_rate;
        self.exchange_rate = new_rate;
        self.last_rate_update_slot = current_slot;

        // Calculate appreciation directly from rate delta
        // appreciation = vault_balance * (new_rate - old_rate) / 1e9
        let appreciation = if new_rate > old_rate {
            ((vault_balance as u128 * (new_rate - old_rate) as u128)
                / Self::RATE_PRECISION as u128) as u64
        } else {
            0
        };

        if appreciation > 0 {
            self.total_appreciation_harvested = self
                .total_appreciation_harvested
                .checked_add(appreciation)
                .ok_or(crate::UnifiedSolPoolError::ArithmeticOverflow)?;
        }

        Ok(appreciation)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_sol_pool_config_size() {
        // Verify size is reasonable for on-chain account
        assert!(UnifiedSolPoolConfig::SIZE < 1024);
    }

    #[test]
    fn test_lst_config_size() {
        // Verify size is reasonable for on-chain account
        assert!(LstConfig::SIZE < 512);
    }

    #[test]
    fn test_calculate_virtual_sol() {
        let config = LstConfig {
            // Header
            pool_type: 1, // SplStakePool
            is_active: 1,
            bump: 255,
            _header_pad: [0u8; 5],
            // Common References
            lst_mint: [0u8; 32],
            lst_vault: [0u8; 32],
            // Exchange Rate State
            exchange_rate: 1_050_000_000, // 1.05x
            harvested_exchange_rate: 1_050_000_000,
            last_rate_update_slot: 0,
            last_harvest_epoch: 0,
            _pad_for_u128: 0,
            // Value Tracking
            total_virtual_sol: 0,
            vault_token_balance: 0,
            _value_pad: 0,
            // Statistics
            total_deposited: 0,
            total_withdrawn: 0,
            total_appreciation_harvested: 0,
            deposit_count: 0,
            withdrawal_count: 0,
            _stat_pad: 0,
            // Stake Pool Specific
            stake_pool: [0u8; 32],
            stake_pool_program: [0u8; 32],
            previous_exchange_rate: 1_000_000_000,
            // Reserved
            _reserved: 0,
        };

        // 100 LST at 1.05x = 105 SOL
        let virtual_sol = config.calculate_virtual_sol(100_000_000_000);
        assert_eq!(virtual_sol, 105_000_000_000);
    }

    #[test]
    fn test_calculate_lst_tokens() {
        let config = LstConfig {
            // Header
            pool_type: 1, // SplStakePool
            is_active: 1,
            bump: 255,
            _header_pad: [0u8; 5],
            // Common References
            lst_mint: [0u8; 32],
            lst_vault: [0u8; 32],
            // Exchange Rate State
            exchange_rate: 1_050_000_000, // 1.05x
            harvested_exchange_rate: 1_050_000_000,
            last_rate_update_slot: 0,
            last_harvest_epoch: 0,
            _pad_for_u128: 0,
            // Value Tracking
            total_virtual_sol: 0,
            vault_token_balance: 0,
            _value_pad: 0,
            // Statistics
            total_deposited: 0,
            total_withdrawn: 0,
            total_appreciation_harvested: 0,
            deposit_count: 0,
            withdrawal_count: 0,
            _stat_pad: 0,
            // Stake Pool Specific
            stake_pool: [0u8; 32],
            stake_pool_program: [0u8; 32],
            previous_exchange_rate: 1_000_000_000,
            // Reserved
            _reserved: 0,
        };

        // 105 virtual SOL at 1.05x = 100 LST
        let lst_tokens = config.calculate_lst_tokens(105_000_000_000);
        assert_eq!(lst_tokens, 100_000_000_000);
    }
}
