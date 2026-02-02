//! Token pool errors.

use pinocchio::program_error::ProgramError;

/// Token pool error codes.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenPoolError {
    /// Pool is paused
    PoolPaused = 0,
    /// Insufficient vault balance for withdrawal
    InsufficientBalance = 1,
    /// Deposit exceeds maximum allowed
    DepositLimitExceeded = 2,
    /// Invalid instruction data
    InvalidInstructionData = 3,
    /// Invalid pool config account
    InvalidPoolConfig = 4,
    /// Invalid vault account
    InvalidVault = 5,
    /// Invalid token program
    InvalidTokenProgram = 6,
    /// Invalid hub caller
    InvalidHubCaller = 7,
    /// Arithmetic overflow
    ArithmeticOverflow = 8,
    /// Invalid account owner
    InvalidAccountOwner = 9,
    /// Relayer fee exceeds transfer amount
    RelayerFeeExceedsAmount = 10,
    /// Invalid hub authority
    InvalidHubAuthority = 11,
    /// Unauthorized - caller is not the authority
    Unauthorized = 12,
    /// Rewards not ready - not enough slots elapsed since last finalization
    RewardsNotReady = 13,
    /// Invalid amount (zero or out of range)
    InvalidAmount = 14,
    /// Invalid mint account
    InvalidMint = 15,
    /// Invalid system program
    InvalidSystemProgram = 16,
    /// Pool already initialized
    AlreadyInitialized = 17,
    /// Expected output doesn't match computed value
    ExpectedOutputMismatch = 18,
    /// Invalid fee rate (must be <= 10000 basis points)
    InvalidFeeRate = 19,
    /// Invalid vault PDA address
    InvalidVaultPda = 20,
    /// Invalid pool config PDA address
    InvalidPoolConfigPda = 21,
}

impl From<TokenPoolError> for ProgramError {
    fn from(e: TokenPoolError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
