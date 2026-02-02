//! Unified SOL pool errors.

use pinocchio::program_error::ProgramError;

/// Unified SOL pool error codes.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnifiedSolPoolError {
    /// Pool is paused
    PoolPaused = 0,
    /// Insufficient vault balance for withdrawal
    InsufficientBalance = 1,
    /// Deposit exceeds maximum allowed
    DepositLimitExceeded = 2,
    /// Invalid instruction data
    InvalidInstructionData = 3,
    /// Invalid unified SOL pool config account
    InvalidUnifiedSolPoolConfig = 4,
    /// Invalid LST config account
    InvalidLstConfig = 5,
    /// Invalid vault account
    InvalidVault = 6,
    /// Invalid token program
    InvalidTokenProgram = 7,
    /// Invalid hub caller
    InvalidHubCaller = 8,
    /// Arithmetic overflow
    ArithmeticOverflow = 9,
    /// Invalid account owner
    InvalidAccountOwner = 10,
    /// Relayer fee exceeds transfer amount
    RelayerFeeExceedsAmount = 11,
    /// Exchange rate is stale - must harvest first
    StaleExchangeRate = 12,
    /// LST not harvested for current epoch
    LstNotHarvested = 13,
    /// Invalid exchange rate
    InvalidExchangeRate = 14,
    /// Insufficient liquidity in requested LST vault
    InsufficientLiquidity = 15,
    /// LST config not active
    LstNotActive = 16,
    /// Invalid hub authority
    InvalidHubAuthority = 17,
    /// Unauthorized operation
    Unauthorized = 18,
    /// Rewards not ready - not enough slots elapsed since last finalization
    RewardsNotReady = 19,
    /// Invalid system program
    InvalidSystemProgram = 20,
    /// Account already initialized
    AlreadyInitialized = 21,
    /// LST already registered
    LstAlreadyRegistered = 22,
    /// Invalid stake pool
    InvalidStakePool = 23,
    /// Invalid pool type
    InvalidPoolType = 24,
    /// Invalid fee rate (must be <= 10000 basis points)
    InvalidFeeRate = 25,
    /// Missing required LST config accounts
    MissingLstConfigs = 26,
    /// Duplicate LST config provided
    DuplicateLstConfig = 27,
    /// Expected output doesn't match computed value
    ExpectedOutputMismatch = 28,
    /// Maximum number of LST configs reached
    MaxLstConfigsReached = 29,
    /// Invalid LST vault PDA address
    InvalidLstVaultPda = 30,
    /// Invalid stake pool program (not in whitelist)
    InvalidStakePoolProgram = 31,
    /// Invalid hub authority PDA
    InvalidHubAuthorityPda = 32,
    /// Invalid unified config PDA
    InvalidUnifiedConfigPda = 33,
    /// Stake pool's pool_mint doesn't match provided lst_mint
    StakePoolMintMismatch = 34,
    /// Stake pool not updated in current epoch (stale rate data)
    StaleStakePoolRate = 35,
    /// WSOL withdrawal would violate minimum buffer requirement
    InsufficientBuffer = 36,
    /// Counter vault_token_balance doesn't match actual vault balance
    VaultBalanceMismatch = 37,
}

impl From<UnifiedSolPoolError> for ProgramError {
    fn from(e: UnifiedSolPoolError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
