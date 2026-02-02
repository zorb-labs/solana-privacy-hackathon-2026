//! CPI instruction builders for invoking pool programs.
//!
//! The hub uses these builders to construct CPI calls to pool programs.
//! In the delegation model:
//! - Hub handles all token transfers
//! - Deposits: Hub transfers first, then CPIs to pool for validation/accounting
//! - Withdrawals: Hub CPIs to pool for approval, then executes transfers

use crate::{DepositParams, PoolInstruction, PoolType, WithdrawParams};

// ============================================================================
// Instruction Data Builders
// ============================================================================

/// Build instruction data for a deposit CPI.
///
/// Layout: [discriminator: u8, params: DepositParams (16 bytes)]
pub fn build_deposit_instruction_data(params: &DepositParams) -> [u8; 17] {
    let mut data = [0u8; 17];
    data[0] = PoolInstruction::Deposit.to_u8();
    data[1..17].copy_from_slice(&params.to_bytes());
    data
}

/// Build instruction data for a withdrawal CPI.
///
/// Layout: [discriminator: u8, params: WithdrawParams (16 bytes)]
pub fn build_withdraw_instruction_data(params: &WithdrawParams) -> [u8; 17] {
    let mut data = [0u8; 17];
    data[0] = PoolInstruction::Withdraw.to_u8();
    data[1..17].copy_from_slice(&params.to_bytes());
    data
}

/// Parse a pool instruction discriminator from instruction data.
pub fn parse_instruction_discriminator(data: &[u8]) -> Option<PoolInstruction> {
    if data.is_empty() {
        return None;
    }
    match data[0] {
        0 => Some(PoolInstruction::Deposit),
        1 => Some(PoolInstruction::Withdraw),
        2 => Some(PoolInstruction::GetInfo),
        _ => None,
    }
}

/// Parse deposit params from instruction data (after discriminator).
pub fn parse_deposit_params(data: &[u8]) -> Option<DepositParams> {
    if data.len() < 17 {
        return None;
    }
    DepositParams::from_bytes(&data[1..])
}

/// Parse withdraw params from instruction data (after discriminator).
pub fn parse_withdraw_params(data: &[u8]) -> Option<WithdrawParams> {
    if data.len() < 17 {
        return None;
    }
    WithdrawParams::from_bytes(&data[1..])
}

// ============================================================================
// Account Layout Constants
// ============================================================================

/// Pool account counts for hub asset_map loading.
///
/// These constants define how many accounts are consumed from remaining_accounts
/// when loading pool configuration for each unique asset_id in a transaction.
///
/// Note: These are different from the CPI account counts in `deposit_accounts`
/// and `withdraw_accounts`, which include token_program, hub_program, etc.
/// These pool_layout counts are for the hub's asset_map building only.
pub mod pool_layout {
    /// Number of pool accounts for Token pool per unique asset.
    /// Layout: (hub_pool_config, token_pool_config, vault)
    pub const TOKEN_POOL_ACCOUNTS: usize = 3;

    /// Number of pool accounts for Unified SOL pool per unique asset.
    /// Layout: (hub_pool_config, unified_sol_pool_config, lst_config, vault)
    pub const UNIFIED_SOL_POOL_ACCOUNTS: usize = 4;

    /// Number of user token accounts per public asset slot.
    /// Layout: (depositor_token, recipient_token, relayer_token)
    pub const USER_TOKEN_ACCOUNTS: usize = 3;
}

/// Account indices for deposit CPI.
///
/// Pool executes transfers from depositor to vault.
///
/// Account layout (5 accounts):
/// 0. pool_config (mut) - Pool state account
/// 1. vault (mut) - Vault token account
/// 2. depositor_token (mut) - Depositor's token account (source)
/// 3. depositor (signer) - Depositor authority (passed from hub)
/// 4. token_program - SPL Token program
/// 5. hub_program - Hub program (for CPI validation)
pub mod deposit_accounts {
    /// Pool config account (writable)
    pub const POOL_CONFIG: usize = 0;
    /// Vault token account (writable)
    pub const VAULT: usize = 1;
    /// Depositor's token account (writable, source)
    pub const DEPOSITOR_TOKEN: usize = 2;
    /// Depositor authority (signer, passed from hub)
    pub const DEPOSITOR: usize = 3;
    /// Token program
    pub const TOKEN_PROGRAM: usize = 4;
    /// Hub program (for CPI validation)
    pub const HUB_PROGRAM: usize = 5;
    /// Total number of accounts for token pool
    pub const COUNT: usize = 6;

    // Unified SOL pool extensions
    /// LST config account for unified SOL pool (writable)
    pub const LST_CONFIG: usize = 6;
    /// Total accounts for unified SOL pool
    pub const UNIFIED_COUNT: usize = 7;
}

/// Account indices for withdrawal CPI.
///
/// Pool executes transfers from vault to recipient, and optionally
/// approves hub_authority for relayer fee portion.
///
/// Account layout (6 accounts):
/// 0. pool_config (mut) - Pool state account, PDA signer
/// 1. vault (mut) - Vault token account
/// 2. recipient_token (mut) - Recipient's token account
/// 3. hub_authority - Hub authority PDA (for delegation of relayer fee)
/// 4. token_program - SPL Token program
/// 5. hub_program - Hub program (for CPI validation)
pub mod withdraw_accounts {
    /// Pool config account (writable, PDA signer)
    pub const POOL_CONFIG: usize = 0;
    /// Vault token account (writable)
    pub const VAULT: usize = 1;
    /// Recipient's token account (writable)
    pub const RECIPIENT_TOKEN: usize = 2;
    /// Hub authority PDA (for delegation)
    pub const HUB_AUTHORITY: usize = 3;
    /// Token program
    pub const TOKEN_PROGRAM: usize = 4;
    /// Hub program (for CPI validation)
    pub const HUB_PROGRAM: usize = 5;
    /// Total number of accounts for token pool
    pub const COUNT: usize = 6;

    // Unified SOL pool extensions
    /// LST config account for unified SOL pool (writable, PDA signer)
    pub const LST_CONFIG: usize = 6;
    /// Total accounts for unified SOL pool
    pub const UNIFIED_COUNT: usize = 7;
}

// ============================================================================
// Pool Operations Trait
// ============================================================================

/// Trait defining the interface that pool programs must implement.
///
/// This trait is used for compile-time abstraction and testing.
/// At runtime, the hub uses CPI to invoke pool programs.
pub trait PoolOperations {
    /// Get the pool type
    fn pool_type(&self) -> PoolType;

    /// Check if the pool is active
    fn is_active(&self) -> bool;

    /// Get deposit fee rate in basis points
    fn deposit_fee_rate(&self) -> u16;

    /// Get withdrawal fee rate in basis points
    fn withdrawal_fee_rate(&self) -> u16;

    /// Get exchange rate numerator
    fn exchange_rate_num(&self) -> u128;

    /// Get exchange rate denominator
    fn exchange_rate_denom(&self) -> u128;

    /// Validate a deposit operation
    fn validate_deposit(&self, amount: u64) -> bool;

    /// Validate a withdrawal operation
    fn validate_withdrawal(&self, amount: u64) -> bool;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deposit_instruction_data_layout() {
        let params = DepositParams {
            amount: 1000,
            expected_output: 995,
        };
        let data = build_deposit_instruction_data(&params);
        assert_eq!(data[0], PoolInstruction::Deposit.to_u8());
        assert_eq!(data.len(), 17);

        // Verify we can parse it back
        let discriminator = parse_instruction_discriminator(&data);
        assert_eq!(discriminator, Some(PoolInstruction::Deposit));

        let parsed = parse_deposit_params(&data).unwrap();
        assert_eq!(parsed.amount, 1000);
        assert_eq!(parsed.expected_output, 995);
    }

    #[test]
    fn test_withdraw_instruction_data_layout() {
        let params = WithdrawParams {
            amount: 1000,
            expected_output: 995,
        };
        let data = build_withdraw_instruction_data(&params);
        assert_eq!(data[0], PoolInstruction::Withdraw.to_u8());
        assert_eq!(data.len(), 17);

        // Verify we can parse it back
        let discriminator = parse_instruction_discriminator(&data);
        assert_eq!(discriminator, Some(PoolInstruction::Withdraw));

        let parsed = parse_withdraw_params(&data).unwrap();
        assert_eq!(parsed.amount, 1000);
        assert_eq!(parsed.expected_output, 995);
    }
}
