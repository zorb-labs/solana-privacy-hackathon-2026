//! Core types for pool interface.

use bytemuck::{Pod, Zeroable};

/// Basis points precision (10000 = 100%)
pub const BASIS_POINTS: u64 = 10_000;

/// Exchange rate precision for LST pools (1e9)
pub const RATE_PRECISION: u128 = 1_000_000_000;

// ============================================================================
// Exchange Rate Conversion Functions (φ and φ⁻¹)
// ============================================================================

/// Convert LST tokens to virtual SOL using exchange rate.
///
/// Implements φ(e) = e × λ / ρ where:
/// - e = token amount
/// - λ = exchange_rate (1 LST = λ/ρ SOL)
/// - ρ = RATE_PRECISION (1e9)
///
/// # Example
/// ```
/// use zorb_pool_interface::tokens_to_virtual_sol;
///
/// // 100 LST at rate 1.05e9 (1 LST = 1.05 SOL)
/// let virtual_sol = tokens_to_virtual_sol(100_000_000_000, 1_050_000_000);
/// assert_eq!(virtual_sol, Some(105_000_000_000));
/// ```
///
/// # Returns
/// `None` on arithmetic overflow (should never happen with valid inputs)
#[inline]
pub fn tokens_to_virtual_sol(tokens: u64, exchange_rate: u64) -> Option<u128> {
    (tokens as u128)
        .checked_mul(exchange_rate as u128)?
        .checked_div(RATE_PRECISION)
}

/// Convert virtual SOL to LST tokens using exchange rate.
///
/// Implements φ⁻¹(s) = s × ρ / λ where:
/// - s = virtual SOL amount
/// - λ = exchange_rate (1 LST = λ/ρ SOL)
/// - ρ = RATE_PRECISION (1e9)
///
/// # Example
/// ```
/// use zorb_pool_interface::virtual_sol_to_tokens;
///
/// // 105 virtual SOL at rate 1.05e9 (1 LST = 1.05 SOL)
/// let tokens = virtual_sol_to_tokens(105_000_000_000, 1_050_000_000);
/// assert_eq!(tokens, Some(100_000_000_000));
/// ```
///
/// # Returns
/// `None` on arithmetic overflow or if exchange_rate is zero
///
/// # Exchange Rate Invariant
///
/// This function enforces `exchange_rate >= RATE_PRECISION (1e9)`.
/// Rates below 1.0 (1e9) would mean LST is worth less than its underlying SOL,
/// which should never happen in normal operation.
///
/// **Defense-in-depth**: Returns `None` for invalid rates, even though upstream
/// validation in `init_lst_config.rs` and `harvest_lst_appreciation.rs` should
/// prevent this state.
#[inline]
pub fn virtual_sol_to_tokens(virtual_sol: u64, exchange_rate: u64) -> Option<u64> {
    // Defense-in-depth: reject rates below minimum (1:1)
    // This prevents overflow in the u64 cast below
    if exchange_rate == 0 || (exchange_rate as u128) < RATE_PRECISION {
        return None;
    }
    let result = (virtual_sol as u128)
        .checked_mul(RATE_PRECISION)?
        .checked_div(exchange_rate as u128)?;
    // Safe cast: result <= virtual_sol since exchange_rate >= RATE_PRECISION
    Some(result as u64)
}

// ============================================================================
// Unified Fee Calculation Functions
// ============================================================================

/// Calculate deposit output for both pool types.
///
/// This function handles the fee calculation for deposits:
/// - Token pool (exchange_rate = None): principal = amount - fee
/// - Unified SOL pool: virtual_sol = φ(amount), principal = virtual_sol - fee
///
/// # Arguments
/// * `amount` - Token amount being deposited
/// * `fee_rate_bps` - Fee rate in basis points (e.g., 100 = 1%)
/// * `exchange_rate` - Exchange rate for unified SOL pool (None for token pool)
///
/// # Returns
/// `Some((principal, protocol_fee))` on success, `None` on arithmetic overflow
///
/// # Example
/// ```
/// use zorb_pool_interface::calculate_deposit_output;
///
/// // Token pool: 1000 tokens at 1% fee
/// let (principal, fee) = calculate_deposit_output(1000, 100, None).unwrap();
/// assert_eq!(fee, 10);
/// assert_eq!(principal, 990);
///
/// // Unified SOL: 1000 tokens at 1.05x rate, 1% fee
/// let (principal, fee) = calculate_deposit_output(1000, 100, Some(1_050_000_000)).unwrap();
/// // 1000 tokens → 1050 virtual SOL, fee = 10 (1% of 1050), principal = 1040
/// assert_eq!(principal, 1040);
/// ```
#[inline]
pub fn calculate_deposit_output(
    amount: u64,
    fee_rate_bps: u16,
    exchange_rate: Option<u64>,
) -> Option<(u64, u64)> {
    // Convert to working units (virtual SOL for unified, same for token)
    let working_units = match exchange_rate {
        Some(rate) => tokens_to_virtual_sol(amount, rate)? as u64,
        None => amount,
    };

    // Calculate fee: working_units × rate / BASIS_POINTS
    let fee = (working_units as u128)
        .checked_mul(fee_rate_bps as u128)?
        .checked_div(BASIS_POINTS as u128)? as u64;

    // Principal = working_units - fee
    let principal = working_units.checked_sub(fee)?;

    Some((principal, fee))
}

/// Calculate withdrawal output for both pool types.
///
/// This function handles the fee calculation for withdrawals:
/// - Token pool (exchange_rate = None): output = amount - fee
/// - Unified SOL pool: fee on virtual SOL, output = φ⁻¹(amount - fee)
///
/// # Arguments
/// * `amount` - Virtual SOL / pool units being withdrawn
/// * `fee_rate_bps` - Fee rate in basis points (e.g., 100 = 1%)
/// * `exchange_rate` - Exchange rate for unified SOL pool (None for token pool)
///
/// # Returns
/// `Some((output_tokens, protocol_fee))` on success, `None` on arithmetic overflow
///
/// # Example
/// ```
/// use zorb_pool_interface::calculate_withdrawal_output;
///
/// // Token pool: 1000 pool units at 0.5% fee
/// let (output, fee) = calculate_withdrawal_output(1000, 50, None).unwrap();
/// assert_eq!(fee, 5);
/// assert_eq!(output, 995);
///
/// // Unified SOL: 1050 virtual SOL at 1.05x rate, 0.5% fee
/// let (output, fee) = calculate_withdrawal_output(1050, 50, Some(1_050_000_000)).unwrap();
/// // fee = 5 (0.5% of 1050), net = 1045, output = φ⁻¹(1045) ≈ 995 tokens
/// ```
#[inline]
pub fn calculate_withdrawal_output(
    amount: u64,
    fee_rate_bps: u16,
    exchange_rate: Option<u64>,
) -> Option<(u64, u64)> {
    // Calculate fee: amount × rate / BASIS_POINTS (fee is in pool units)
    let fee = (amount as u128)
        .checked_mul(fee_rate_bps as u128)?
        .checked_div(BASIS_POINTS as u128)? as u64;

    // Net amount after fee
    let net_amount = amount.checked_sub(fee)?;

    // Convert to output tokens (φ⁻¹ for unified, same for token)
    let output_tokens = match exchange_rate {
        Some(rate) => virtual_sol_to_tokens(net_amount, rate)?,
        None => net_amount,
    };

    Some((output_tokens, fee))
}

// ============================================================================
// Pool Type Discriminator
// ============================================================================

/// Pool type discriminator for identifying pool programs.
///
/// Used by the hub to determine which pool program to invoke and how to
/// interpret exchange rates.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolType {
    /// Standard SPL token pool (φ = identity, exchange rate = 1:1)
    Token = 0,
    /// Unified SOL pool with LST support (φ = exchange rate conversion)
    UnifiedSol = 1,
}

impl PoolType {
    /// Convert from u8 to PoolType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PoolType::Token),
            1 => Some(PoolType::UnifiedSol),
            _ => None,
        }
    }

    /// Returns true if this pool type uses an exchange rate
    pub fn has_exchange_rate(&self) -> bool {
        matches!(self, PoolType::UnifiedSol)
    }
}

// ============================================================================
// CPI Instruction Discriminators
// ============================================================================

/// Pool instruction discriminators for CPI calls.
///
/// These are the instruction discriminators that pool programs must implement
/// to be compatible with the hub.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolInstruction {
    /// Execute a deposit into the pool
    /// Accounts: [vault, user_token_account, pool_config, token_program, ...]
    Deposit = 0,

    /// Execute a withdrawal from the pool
    /// Accounts: [vault, recipient_token_account, pool_config, token_program, ...]
    Withdraw = 1,

    /// Get pool info (read-only query, not typically used via CPI)
    GetInfo = 2,
}

impl PoolInstruction {
    /// Convert to u8 discriminator
    pub const fn to_u8(self) -> u8 {
        self as u8
    }
}

// ============================================================================
// CPI Parameters
// ============================================================================

/// Parameters for a deposit CPI call from hub to pool.
///
/// Pool executes the transfer and validates the expected output.
///
/// # Token Flow (Deposit)
/// ```text
/// Hub validates proof, calculates amounts
/// Hub CPIs to pool: { amount, expected_output }
/// Pool: depositor ──(amount)──► vault
/// Pool: validates expected_output = amount - fee (with exchange rate if applicable)
/// Pool: updates state, returns { fee }
/// Hub: handles relayer_fee transfer separately
/// ```
///
/// # Responsibilities
/// - Pool: Transfer amount from depositor to vault
/// - Pool: Validate expected_output matches (amount - protocol_fee) with exchange rate
/// - Pool: Update accounting, return protocol_fee
/// - Hub: Handle relayer_fee transfer (not included in pool CPI)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct DepositParams {
    /// Total tokens to transfer from depositor to vault (principal + protocol_fee)
    pub amount: u64,
    /// Expected output in pool-native units (principal credited to shielded balance)
    /// For token pools: expected_output = amount - protocol_fee
    /// For unified SOL: expected_output = (amount - protocol_fee) * exchange_rate
    pub expected_output: u64,
}

impl DepositParams {
    /// Size in bytes
    pub const SIZE: usize = 16;

    /// Serialize to bytes for CPI instruction data
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..8].copy_from_slice(&self.amount.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.expected_output.to_le_bytes());
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            amount: u64::from_le_bytes(bytes[0..8].try_into().ok()?),
            expected_output: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
        })
    }
}

/// Parameters for a withdrawal CPI call from hub to pool.
///
/// Pool validates amounts and approves hub to transfer from vault.
/// Hub handles the actual distribution to recipient and relayer.
///
/// # Token Flow (Withdrawal)
/// ```text
/// Hub validates proof, calculates amounts
/// Hub CPIs to pool: { amount, expected_output }
/// Pool: validates expected_output = amount - protocol_fee
/// Pool: approves hub_authority for expected_output (total tokens to distribute)
/// Pool: updates state, returns { fee }
/// Hub: vault ──(expected_output - relayer_fee)──► recipient
/// Hub: vault ──(relayer_fee)──► relayer (using approval)
/// ```
///
/// # Responsibilities
/// - Pool: Validate expected_output matches (amount - protocol_fee) with exchange rate
/// - Pool: Approve hub_authority as delegate for expected_output
/// - Pool: Update accounting, return protocol_fee
/// - Hub: Transfer (expected_output - relayer_fee) from vault to recipient
/// - Hub: Transfer relayer_fee from vault to relayer
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct WithdrawParams {
    /// Principal amount in pool-native units (shielded value being spent)
    pub amount: u64,
    /// Expected tokens to distribute (after protocol_fee)
    /// For token pools: expected_output = amount - protocol_fee
    /// For unified SOL: expected_output = (amount - protocol_fee) / exchange_rate
    /// Hub will split this between recipient (expected_output - relayer_fee) and relayer (relayer_fee)
    pub expected_output: u64,
}

impl WithdrawParams {
    /// Size in bytes
    pub const SIZE: usize = 16;

    /// Serialize to bytes for CPI instruction data
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..8].copy_from_slice(&self.amount.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.expected_output.to_le_bytes());
        bytes
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            amount: u64::from_le_bytes(bytes[0..8].try_into().ok()?),
            expected_output: u64::from_le_bytes(bytes[8..16].try_into().ok()?),
        })
    }
}

/// Return data from pool operations.
///
/// Pool returns protocol fee collected via `set_return_data`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct PoolReturnData {
    /// Protocol fee collected by the pool
    pub fee: u64,
}

impl PoolReturnData {
    /// Size in bytes
    pub const SIZE: usize = 8;

    /// Serialize to bytes
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.fee.to_le_bytes()
    }

    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            fee: u64::from_le_bytes(bytes[0..8].try_into().ok()?),
        })
    }
}

// ============================================================================
// Pool Info (for Hub Fee Calculation)
// ============================================================================

/// Pool configuration info read by the hub for fee calculation.
///
/// The hub reads this information directly from pool config accounts
/// (zero-copy) to compute and validate fees before calling pools.
///
/// # Exchange Rate
///
/// For Token pools: `exchange_rate_num = exchange_rate_denom = RATE_PRECISION` (1:1)
/// For Unified SOL: `exchange_rate = exchange_rate_num / exchange_rate_denom`
///
/// The exchange rate converts between tokens and pool-native units:
/// - Deposit: `pool_units = tokens × exchange_rate_num / exchange_rate_denom`
/// - Withdraw: `tokens = pool_units × exchange_rate_denom / exchange_rate_num`
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct PoolInfo {
    /// Deposit fee rate in basis points (e.g., 100 = 1%)
    pub deposit_fee_rate: u16,
    /// Withdrawal fee rate in basis points (e.g., 100 = 1%)
    pub withdrawal_fee_rate: u16,
    /// Padding for alignment (u128 requires 16-byte alignment)
    pub _padding: [u8; 12],
    /// Exchange rate numerator (RATE_PRECISION for 1:1)
    pub exchange_rate_num: u128,
    /// Exchange rate denominator (RATE_PRECISION for 1:1)
    pub exchange_rate_denom: u128,
    /// Whether the pool is paused (1 = paused, 0 = active)
    pub is_paused: u8,
    /// Pool type discriminator
    pub pool_type: u8,
    /// Padding for alignment (struct must be multiple of 16 for u128 alignment)
    pub _padding2: [u8; 14],
}

impl PoolInfo {
    /// Create a new PoolInfo for a token pool (1:1 exchange rate)
    pub const fn new_token_pool(deposit_fee_rate: u16, withdrawal_fee_rate: u16) -> Self {
        Self {
            deposit_fee_rate,
            withdrawal_fee_rate,
            _padding: [0; 12],
            exchange_rate_num: RATE_PRECISION,
            exchange_rate_denom: RATE_PRECISION,
            is_paused: 0,
            pool_type: PoolType::Token as u8,
            _padding2: [0; 14],
        }
    }

    /// Create a new PoolInfo for a unified SOL pool
    pub const fn new_unified_sol_pool(
        deposit_fee_rate: u16,
        withdrawal_fee_rate: u16,
        exchange_rate_num: u128,
        exchange_rate_denom: u128,
    ) -> Self {
        Self {
            deposit_fee_rate,
            withdrawal_fee_rate,
            _padding: [0; 12],
            exchange_rate_num,
            exchange_rate_denom,
            is_paused: 0,
            pool_type: PoolType::UnifiedSol as u8,
            _padding2: [0; 14],
        }
    }

    /// Returns true if the pool is paused
    pub const fn is_paused(&self) -> bool {
        self.is_paused != 0
    }

    /// Get the pool type
    pub fn pool_type(&self) -> Option<PoolType> {
        PoolType::from_u8(self.pool_type)
    }

    /// Convert token amount to pool-native units using exchange rate
    ///
    /// For deposits: tokens → pool_units
    pub fn tokens_to_pool_units(&self, tokens: u64) -> u64 {
        if self.exchange_rate_denom == 0 {
            return 0;
        }
        // pool_units = tokens × rate_num / rate_denom
        ((tokens as u128) * self.exchange_rate_num / self.exchange_rate_denom) as u64
    }

    /// Convert pool-native units to token amount using exchange rate
    ///
    /// For withdrawals: pool_units → tokens
    pub fn pool_units_to_tokens(&self, pool_units: u64) -> u64 {
        if self.exchange_rate_num == 0 {
            return 0;
        }
        // tokens = pool_units × rate_denom / rate_num
        ((pool_units as u128) * self.exchange_rate_denom / self.exchange_rate_num) as u64
    }

    /// Calculate the expected fee for a given principal amount
    ///
    /// Uses the universal formula: `fee = principal × rate / BASIS_POINTS`
    pub fn calculate_fee(&self, principal: u64, is_deposit: bool) -> u64 {
        let rate = if is_deposit {
            self.deposit_fee_rate
        } else {
            self.withdrawal_fee_rate
        };
        (principal as u128 * rate as u128 / BASIS_POINTS as u128) as u64
    }
}

// ============================================================================
// Hub → Pool CPI Helper Types
// ============================================================================

/// Result of hub fee validation and amount computation.
///
/// The hub computes all these values before calling a pool via CPI.
/// Pool executes transfers, hub handles relayer fee separately.
#[derive(Clone, Copy, Debug)]
pub struct ComputedPoolParams {
    /// For deposit: tokens to transfer to vault (principal + protocol_fee)
    /// For withdraw: pool units being spent (principal)
    pub amount: u64,
    /// Expected output from the operation:
    /// - Deposit: pool units credited (principal)
    /// - Withdraw: tokens to recipient (after protocol_fee)
    pub expected_output: u64,
    /// Protocol fee (returned by pool for verification)
    pub protocol_fee: u64,
    /// Relayer fee in tokens (hub handles transfer separately)
    pub relayer_fee_tokens: u64,
}

impl ComputedPoolParams {
    /// Convert to DepositParams for CPI
    pub const fn to_deposit_params(&self) -> DepositParams {
        DepositParams {
            amount: self.amount,
            expected_output: self.expected_output,
        }
    }

    /// Convert to WithdrawParams for CPI
    pub const fn to_withdraw_params(&self) -> WithdrawParams {
        WithdrawParams {
            amount: self.amount,
            expected_output: self.expected_output,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_type_from_u8() {
        assert_eq!(PoolType::from_u8(0), Some(PoolType::Token));
        assert_eq!(PoolType::from_u8(1), Some(PoolType::UnifiedSol));
        assert_eq!(PoolType::from_u8(2), None);
    }

    #[test]
    fn test_pool_info_token_pool() {
        let info = PoolInfo::new_token_pool(100, 50);
        assert_eq!(info.deposit_fee_rate, 100);
        assert_eq!(info.withdrawal_fee_rate, 50);
        assert_eq!(info.tokens_to_pool_units(1000), 1000); // 1:1
        assert_eq!(info.pool_units_to_tokens(1000), 1000); // 1:1
    }

    #[test]
    fn test_pool_info_unified_sol() {
        // Exchange rate: 1.1 (LST worth more than SOL)
        let info = PoolInfo::new_unified_sol_pool(
            100,
            50,
            RATE_PRECISION * 11 / 10, // 1.1
            RATE_PRECISION,
        );

        // 1000 tokens at 1.1x = 1100 pool units
        assert_eq!(info.tokens_to_pool_units(1000), 1100);
        // 1100 pool units at 1.1x = 1000 tokens
        assert_eq!(info.pool_units_to_tokens(1100), 1000);
    }

    #[test]
    fn test_fee_calculation() {
        let info = PoolInfo::new_token_pool(100, 50); // 1% deposit, 0.5% withdraw

        // 1% of 10000 = 100
        assert_eq!(info.calculate_fee(10000, true), 100);
        // 0.5% of 10000 = 50
        assert_eq!(info.calculate_fee(10000, false), 50);
    }

    #[test]
    fn test_deposit_params_size() {
        // amount: 8 + expected_output: 8 = 16
        assert_eq!(core::mem::size_of::<DepositParams>(), 16);
    }

    #[test]
    fn test_withdraw_params_size() {
        // amount: 8 + expected_output: 8 = 16
        assert_eq!(core::mem::size_of::<WithdrawParams>(), 16);
    }

    #[test]
    fn test_pool_return_data_size() {
        // fee: 8 = 8
        assert_eq!(core::mem::size_of::<PoolReturnData>(), 8);
    }

    #[test]
    fn test_pool_info_size() {
        // 2 + 2 + 12 (padding) + 16 + 16 + 1 + 1 + 14 (padding) = 64
        // Struct is 16-byte aligned due to u128 fields
        assert_eq!(core::mem::size_of::<PoolInfo>(), 64);
    }

    #[test]
    fn test_deposit_params_serialization() {
        let params = DepositParams {
            amount: 1000,
            expected_output: 995,
        };
        let bytes = params.to_bytes();
        let restored = DepositParams::from_bytes(&bytes).unwrap();
        assert_eq!(params.amount, restored.amount);
        assert_eq!(params.expected_output, restored.expected_output);
    }

    #[test]
    fn test_withdraw_params_serialization() {
        let params = WithdrawParams {
            amount: 1000,
            expected_output: 995,
        };
        let bytes = params.to_bytes();
        let restored = WithdrawParams::from_bytes(&bytes).unwrap();
        assert_eq!(params.amount, restored.amount);
        assert_eq!(params.expected_output, restored.expected_output);
    }

    #[test]
    fn test_pool_return_data_serialization() {
        let data = PoolReturnData { fee: 5 };
        let bytes = data.to_bytes();
        let restored = PoolReturnData::from_bytes(&bytes).unwrap();
        assert_eq!(data.fee, restored.fee);
    }

    // ========================================================================
    // Exchange Rate Conversion Function Tests
    // ========================================================================

    #[test]
    fn test_tokens_to_virtual_sol_at_1x() {
        // Rate = 1.0 (1e9): 100 tokens = 100 virtual SOL
        let result = super::tokens_to_virtual_sol(100_000_000_000, 1_000_000_000);
        assert_eq!(result, Some(100_000_000_000));
    }

    #[test]
    fn test_tokens_to_virtual_sol_at_1_05x() {
        // Rate = 1.05 (1.05e9): 100 tokens = 105 virtual SOL
        let result = super::tokens_to_virtual_sol(100_000_000_000, 1_050_000_000);
        assert_eq!(result, Some(105_000_000_000));
    }

    #[test]
    fn test_tokens_to_virtual_sol_at_1_10x() {
        // Rate = 1.10 (1.1e9): 100 tokens = 110 virtual SOL
        let result = super::tokens_to_virtual_sol(100_000_000_000, 1_100_000_000);
        assert_eq!(result, Some(110_000_000_000));
    }

    #[test]
    fn test_virtual_sol_to_tokens_at_1x() {
        // Rate = 1.0 (1e9): 100 virtual SOL = 100 tokens
        let result = super::virtual_sol_to_tokens(100_000_000_000, 1_000_000_000);
        assert_eq!(result, Some(100_000_000_000));
    }

    #[test]
    fn test_virtual_sol_to_tokens_at_1_05x() {
        // Rate = 1.05 (1.05e9): 105 virtual SOL = 100 tokens
        let result = super::virtual_sol_to_tokens(105_000_000_000, 1_050_000_000);
        assert_eq!(result, Some(100_000_000_000));
    }

    #[test]
    fn test_virtual_sol_to_tokens_at_1_10x() {
        // Rate = 1.10 (1.1e9): 110 virtual SOL = 100 tokens
        let result = super::virtual_sol_to_tokens(110_000_000_000, 1_100_000_000);
        assert_eq!(result, Some(100_000_000_000));
    }

    #[test]
    fn test_roundtrip_tokens_virtual_sol() {
        // Roundtrip: tokens -> virtual_sol -> tokens
        let original_tokens = 100_000_000_000u64;
        let rate = 1_050_000_000u64;

        let virtual_sol = super::tokens_to_virtual_sol(original_tokens, rate).unwrap();
        assert_eq!(virtual_sol, 105_000_000_000);

        let recovered_tokens = super::virtual_sol_to_tokens(virtual_sol as u64, rate).unwrap();
        assert_eq!(recovered_tokens, original_tokens);
    }

    #[test]
    fn test_virtual_sol_to_tokens_zero_rate() {
        // Zero exchange rate should return None
        let result = super::virtual_sol_to_tokens(100_000_000_000, 0);
        assert_eq!(result, None);
    }

    #[test]
    fn test_virtual_sol_to_tokens_below_rate_precision() {
        // Rate = 0.9 (below 1:1) should return None
        // This is a defense-in-depth check - rates below RATE_PRECISION
        // would cause the result to exceed u64::MAX for large inputs
        let result = super::virtual_sol_to_tokens(100_000_000_000, 900_000_000);
        assert_eq!(result, None);

        // Rate = 0.5 (50%) should also return None
        let result = super::virtual_sol_to_tokens(100_000_000_000, 500_000_000);
        assert_eq!(result, None);

        // Rate = 0.999999999 (just below 1:1) should return None
        let result = super::virtual_sol_to_tokens(100_000_000_000, 999_999_999);
        assert_eq!(result, None);

        // Rate = 1.0 (exactly RATE_PRECISION) should succeed
        let result = super::virtual_sol_to_tokens(100_000_000_000, 1_000_000_000);
        assert_eq!(result, Some(100_000_000_000));
    }

    // ========================================================================
    // Unified Fee Calculation Function Tests
    // ========================================================================

    #[test]
    fn test_calculate_deposit_output_token_pool() {
        // Token pool: 1000 tokens at 1% fee
        let (principal, fee) = super::calculate_deposit_output(1000, 100, None).unwrap();
        assert_eq!(fee, 10);
        assert_eq!(principal, 990);
    }

    #[test]
    fn test_calculate_deposit_output_token_pool_zero_fee() {
        // Token pool: 1000 tokens at 0% fee
        let (principal, fee) = super::calculate_deposit_output(1000, 0, None).unwrap();
        assert_eq!(fee, 0);
        assert_eq!(principal, 1000);
    }

    #[test]
    fn test_calculate_deposit_output_unified_sol() {
        // Unified SOL: 1000 tokens at 1.05x rate, 1% fee
        // 1000 tokens → 1050 virtual SOL
        // fee = 1% of 1050 = 10 (integer division)
        // principal = 1050 - 10 = 1040
        let (principal, fee) =
            super::calculate_deposit_output(1000, 100, Some(1_050_000_000)).unwrap();
        assert_eq!(fee, 10);
        assert_eq!(principal, 1040);
    }

    #[test]
    fn test_calculate_deposit_output_unified_sol_at_1x() {
        // Unified SOL at 1.0x rate should behave like token pool
        let (principal, fee) =
            super::calculate_deposit_output(1000, 100, Some(1_000_000_000)).unwrap();
        assert_eq!(fee, 10);
        assert_eq!(principal, 990);
    }

    #[test]
    fn test_calculate_withdrawal_output_token_pool() {
        // Token pool: 1000 pool units at 0.5% fee
        let (output, fee) = super::calculate_withdrawal_output(1000, 50, None).unwrap();
        assert_eq!(fee, 5);
        assert_eq!(output, 995);
    }

    #[test]
    fn test_calculate_withdrawal_output_token_pool_zero_fee() {
        // Token pool: 1000 pool units at 0% fee
        let (output, fee) = super::calculate_withdrawal_output(1000, 0, None).unwrap();
        assert_eq!(fee, 0);
        assert_eq!(output, 1000);
    }

    #[test]
    fn test_calculate_withdrawal_output_unified_sol() {
        // Unified SOL: 1050 virtual SOL at 1.05x rate, 0.5% fee
        // fee = 0.5% of 1050 = 5
        // net = 1050 - 5 = 1045
        // output = φ⁻¹(1045) = 1045 * 1e9 / 1.05e9 = 995 tokens
        let (output, fee) =
            super::calculate_withdrawal_output(1050, 50, Some(1_050_000_000)).unwrap();
        assert_eq!(fee, 5);
        assert_eq!(output, 995);
    }

    #[test]
    fn test_calculate_withdrawal_output_unified_sol_at_1x() {
        // Unified SOL at 1.0x rate should behave like token pool
        let (output, fee) =
            super::calculate_withdrawal_output(1000, 50, Some(1_000_000_000)).unwrap();
        assert_eq!(fee, 5);
        assert_eq!(output, 995);
    }

    #[test]
    fn test_calculate_deposit_output_overflow_protection() {
        // Fee rate that would cause subtraction underflow
        // 100% fee on 100 tokens = 100, principal = 0
        let result = super::calculate_deposit_output(100, 10000, None);
        assert_eq!(result, Some((0, 100)));
    }

    #[test]
    fn test_calculate_withdrawal_output_zero_rate() {
        // Zero exchange rate should return None
        let result = super::calculate_withdrawal_output(1000, 50, Some(0));
        assert_eq!(result, None);
    }
}
