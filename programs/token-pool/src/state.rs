//! Token pool state accounts.

use panchor::prelude::*;
use pinocchio::pubkey::Pubkey;
use zorb_pool_interface::authority::HasAuthority;

use crate::TokenPoolError;

/// Account discriminators for the Token Pool program.
///
/// Each discriminator uniquely identifies an account type. The discriminator
/// is stored as the first 8 bytes of account data.
///
/// # Ranges (per discriminator-standard.md)
/// - **0-15**: Core accounts (pool config)
/// - **16-31**: User accounts (reserved for future use)
/// - **32-63**: Tree accounts (reserved for future use)
/// - **64-127**: Ephemeral accounts (reserved for future use)
#[account_type]
pub enum TokenPoolAccount {
    // =========================================================================
    // Core Accounts (0-15) - Configuration accounts
    // =========================================================================
    /// Token pool configuration (per mint)
    TokenPoolConfig = 0,
    // Reserved: 1-15

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

/// Token pool configuration account.
///
/// Stores all state for a single token pool, including:
/// - Token identity (mint, vault, asset_id)
/// - Epoch-based balance tracking
/// - Fee configuration
/// - Statistics
///
/// # Reward Accumulator Design
///
/// The accumulator tracks rewards per unit of deposit. Rewards are finalized
/// every `UPDATE_SLOT_INTERVAL` slots (~5 minutes).
///
/// ## State Fields
///
/// - `finalized_balance`: Balance frozen at last finalization. NOT modified between finalizations.
/// - `pending_deposits`: New deposits since last finalization (additive only).
/// - `pending_withdrawals`: Withdrawals since last finalization (additive only).
/// - `pending_deposit_fees`: Deposit fees collected since last finalization.
/// - `pending_withdrawal_fees`: Withdrawal fees collected since last finalization.
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
/// After finalization: finalized_balance = 1000e9 (1000 tokens, 9 decimals)
/// User A deposits 500:  pending_deposits = 500e9
/// User B withdraws 200: pending_withdrawals = 200e9
/// Fees collected: pending_rewards = 50e9 (50 tokens)
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
#[account(TokenPoolAccount::TokenPoolConfig)]
#[repr(C)]
pub struct TokenPoolConfig {
    /// Authority that can update pool config
    pub authority: Pubkey,
    /// Pending authority for two-step transfer.
    /// Set by `transfer_authority`, must call `accept_authority` to complete.
    pub pending_authority: Pubkey,
    /// Token mint address
    pub mint: Pubkey,
    /// Vault token account PDA
    pub vault: Pubkey,
    /// Asset ID (Poseidon hash of mint)
    pub asset_id: [u8; 32],
    /// Balance frozen at last finalization (not modified between finalizations).
    ///
    /// **Units:** Token base units (e.g., for USDC with 6 decimals: 1e6 = 1 USDC)
    ///
    /// **Updates on:** `finalize_rewards()` only
    ///
    /// This is the denominator used to compute reward-per-unit during finalization.
    /// Between finalizations, only pending_deposits/pending_withdrawals change.
    pub finalized_balance: u128,

    /// Cumulative rewards per unit of deposit, scaled by 1e18.
    ///
    /// **Units:** Scaled fixed-point (rewards * 1e18 / deposits)
    ///
    /// **Updates on:** `finalize_rewards()` only
    ///
    /// To compute user rewards in circuit:
    /// `user_reward = user_amount * (current_acc - entry_acc) / 1e18`
    pub reward_accumulator: u128,

    /// Deposits since last finalization, for accumulator calculation only.
    ///
    /// **Units:** Token base units (matches token decimals)
    ///
    /// **Purpose:** Used solely in `finalize_rewards()` to compute the reward
    /// accumulator denominator: `total_pool = finalized_balance + pending_deposits - pending_withdrawals`.
    /// This is NOT the source of truth for pool balance (vault balance is).
    ///
    /// **Value tracked:** Principal (amount - fee). This equals the user's ZK
    /// commitment value. Fees are tracked separately in `pending_deposit_fees`.
    ///
    /// **Updates on:**
    /// - Deposit instruction: += principal
    /// - `finalize_rewards()`: reset to 0
    pub pending_deposits: u128,

    /// Withdrawals since last finalization, for accumulator calculation only.
    ///
    /// **Units:** Token base units (matches token decimals)
    ///
    /// **Purpose:** Used solely in `finalize_rewards()` to compute the reward
    /// accumulator denominator: `total_pool = finalized_balance + pending_deposits - pending_withdrawals`.
    /// This is NOT the source of truth for pool balance (vault balance is).
    ///
    /// **Value tracked:** Commitment value being burned (params.amount). This is
    /// the full amount before withdrawal fee. Fees are tracked separately in
    /// `pending_withdrawal_fees`.
    ///
    /// **Updates on:**
    /// - Withdraw instruction: += amount
    /// - `finalize_rewards()`: reset to 0
    pub pending_withdrawals: u128,

    /// Pending deposit fees waiting to be distributed on next finalization.
    ///
    /// **Units:** Token base units (matches token decimals)
    ///
    /// **Updates on:**
    /// - Deposit fees: += fee_amount
    /// - `finalize_rewards()`: reset to 0 after distribution
    pub pending_deposit_fees: u64,

    /// Pending withdrawal fees waiting to be distributed on next finalization.
    ///
    /// **Units:** Token base units (matches token decimals)
    ///
    /// **Updates on:**
    /// - Withdrawal fees: += fee_amount
    /// - `finalize_rewards()`: reset to 0 after distribution
    ///
    /// **Note:** These fee fields (deposit + withdrawal) contain only transaction fees.
    /// External funding is tracked separately in `pending_funded_rewards`. At
    /// finalization, all three are combined and distributed via the accumulator.
    pub pending_withdrawal_fees: u64,

    /// Pending externally funded rewards waiting to be distributed.
    ///
    /// **Units:** Token base units (matches token decimals)
    ///
    /// **Updates on:**
    /// - `fund_rewards()`: += funded_amount
    /// - `finalize_rewards()`: reset to 0 after distribution
    ///
    /// **Audit Note:** This separation enables the finalization event to report
    /// fee_rewards vs funded_rewards unambiguously, providing transparency
    /// on reward sources for indexers and users.
    pub pending_funded_rewards: u64,

    /// Padding for u128 alignment (3 × u64 = 24 bytes, need 32 for u128 alignment)
    pub _pad_fees: u64,

    /// Cumulative total deposits (in token base units)
    pub total_deposited: u128,
    /// Cumulative total withdrawals (in token base units)
    pub total_withdrawn: u128,
    /// Total rewards distributed over lifetime (in token base units)
    ///
    /// **Updates on:** `finalize_rewards()` after distributing rewards
    ///
    /// This equals the sum of all (fee_rewards + funded_rewards) distributed
    /// through finalization events over the pool's lifetime.
    pub total_rewards_distributed: u128,
    /// Total deposit fees collected (in token base units)
    pub total_deposit_fees: u128,
    /// Total withdrawal fees collected (in token base units)
    pub total_withdrawal_fees: u128,
    /// Total rewards funded via fund_rewards instruction (in token base units)
    pub total_funded_rewards: u128,
    /// Reserved for future use (maintains struct alignment)
    pub _reserved_stats: u128,
    /// Maximum deposit amount per transaction
    pub max_deposit_amount: u64,
    /// Number of deposit transactions
    pub deposit_count: u64,
    /// Number of withdrawal transactions
    pub withdrawal_count: u64,

    /// Slot when rewards were last finalized.
    /// Moved after withdrawal_count (transfer_count was removed)
    ///
    /// **Updates on:** `finalize_rewards()` only
    pub last_finalized_slot: u64,
    /// Deposit fee rate in basis points
    pub deposit_fee_rate: u16,
    /// Withdrawal fee rate in basis points
    pub withdrawal_fee_rate: u16,
    /// Token decimals
    pub decimals: u8,
    /// Whether this config is active
    pub is_active: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for struct alignment (9 bytes to reach 16-byte alignment)
    pub _padding: [u8; 9],
}

impl TokenPoolConfig {
    /// Account size
    pub const SIZE: usize = core::mem::size_of::<Self>();

    /// Minimum number of slots between accumulator updates (~18 minutes at 400ms slots).
    ///
    /// This interval determines the "proof validity window" - the minimum time
    /// users have to submit a ZK proof after finalization before the accumulator
    /// can change again. Longer intervals improve UX (more time for proof
    /// generation and submission) at the cost of less frequent reward distribution.
    ///
    /// The actual interval may be longer if no one calls `finalize_rewards` after
    /// the minimum has passed. This is permissionless - anyone can call it.
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

    /// Calculate the PDA address for a token pool config
    /// Seeds: ["token_pool", mint]
    ///
    /// Note: Ignores program_id parameter - uses crate::ID
    pub fn find_pda(_program_id: &Pubkey, mint: &Pubkey) -> (Pubkey, u8) {
        crate::find_token_pool_config_pda(mint)
    }

    /// Check if the pool is active
    pub fn is_active(&self) -> bool {
        self.is_active != 0
    }

    /// Check if pool is active, returning error if paused.
    #[inline]
    pub fn require_active(&self) -> Result<(), TokenPoolError> {
        if !self.is_active() {
            return Err(TokenPoolError::PoolPaused);
        }
        Ok(())
    }

    /// Check if signer matches authority, returning error if unauthorized.
    #[inline]
    pub fn require_authority(&self, signer: &Pubkey) -> Result<(), TokenPoolError> {
        if self.authority != *signer {
            return Err(TokenPoolError::Unauthorized);
        }
        Ok(())
    }

    /// Validate pool_config key matches canonical PDA for mint.
    #[inline]
    pub fn validate_pda(pool_config_key: &Pubkey, mint: &Pubkey) -> Result<(), TokenPoolError> {
        let (expected_pda, _) = crate::find_token_pool_config_pda(mint);
        if *pool_config_key != expected_pda {
            return Err(TokenPoolError::InvalidPoolConfigPda);
        }
        Ok(())
    }

    /// Get current balance (finalized_balance + pending_deposits - pending_withdrawals)
    pub fn current_balance(&self) -> Result<u128, TokenPoolError> {
        self.finalized_balance
            .checked_add(self.pending_deposits)
            .ok_or(TokenPoolError::ArithmeticOverflow)?
            .checked_sub(self.pending_withdrawals)
            .ok_or(TokenPoolError::ArithmeticOverflow)
    }

    /// Finalize pending rewards by updating the reward accumulator.
    ///
    /// This function always advances monotonically when the slot interval passes:
    /// - Updates `finalized_balance = finalized_balance + pending_deposits - pending_withdrawals`
    /// - Resets `pending_deposits` and `pending_withdrawals` to 0
    /// - Updates `last_finalized_slot` to `current_slot`
    ///
    /// Accumulator update (conditional on `total_pool > 0 && total_pending > 0`):
    /// - Calculates `reward_delta = pending_rewards * 1e18 / total_pool`
    /// - Updates `reward_accumulator += reward_delta`
    /// - Resets `pending_deposit_fees`, `pending_withdrawal_fees`, `pending_funded_rewards` to 0
    ///
    /// When `total_pool = 0`, pending reward fields are preserved until depositors arrive.
    ///
    /// Returns `Err(RewardsNotReady)` if `UPDATE_SLOT_INTERVAL` slots
    /// have not passed since `last_finalized_slot`.
    pub fn finalize_rewards(
        &mut self,
        current_slot: u64,
    ) -> Result<(), pinocchio::program_error::ProgramError> {
        // Check if enough slots have passed
        let slots_elapsed = current_slot.saturating_sub(self.last_finalized_slot);
        if slots_elapsed < Self::UPDATE_SLOT_INTERVAL {
            return Err(TokenPoolError::RewardsNotReady.into());
        }

        // Calculate total pool for reward distribution
        let total_pool = self
            .finalized_balance
            .checked_add(self.pending_deposits)
            .ok_or(TokenPoolError::ArithmeticOverflow)?
            .checked_sub(self.pending_withdrawals)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Combine deposit fees, withdrawal fees, and funded rewards for distribution
        let total_pending = (self.pending_deposit_fees as u128)
            .checked_add(self.pending_withdrawal_fees as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?
            .checked_add(self.pending_funded_rewards as u128)
            .ok_or(TokenPoolError::ArithmeticOverflow)?;

        // Update accumulator only if there are deposits AND pending rewards
        // When total_pool = 0, rewards are preserved until depositors arrive
        if total_pool > 0 && total_pending > 0 {
            // Calculate reward delta with precision scaling
            let scaled_rewards = total_pending
                .checked_mul(Self::ACCUMULATOR_PRECISION)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;

            let reward_delta = scaled_rewards
                .checked_div(total_pool)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;

            // Update the accumulator
            self.reward_accumulator = self
                .reward_accumulator
                .checked_add(reward_delta)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;

            // Track total rewards distributed
            self.total_rewards_distributed = self
                .total_rewards_distributed
                .checked_add(total_pending)
                .ok_or(TokenPoolError::ArithmeticOverflow)?;

            // Reset pending reward fields only when distributed
            self.pending_deposit_fees = 0;
            self.pending_withdrawal_fees = 0;
            self.pending_funded_rewards = 0;
        }

        // Always update finalized balance and advance slot
        self.finalized_balance = total_pool;
        self.pending_deposits = 0;
        self.pending_withdrawals = 0;
        self.last_finalized_slot = current_slot;

        Ok(())
    }
}

impl HasAuthority for TokenPoolConfig {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_pool_config_size() {
        // Verify size is reasonable for on-chain account
        assert!(TokenPoolConfig::SIZE < 1024);
    }
}
