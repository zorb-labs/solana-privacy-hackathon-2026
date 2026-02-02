//! Pool error types.

/// Pool error codes shared across all pool implementations.
///
/// These error codes are used by pool programs and can be matched by the hub
/// or clients to understand failure reasons.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolError {
    /// Pool is paused and not accepting operations
    PoolPaused = 0,

    /// Insufficient liquidity in vault for withdrawal
    InsufficientLiquidity = 1,

    /// Invalid deposit amount (zero or exceeds limit)
    InvalidDepositAmount = 2,

    /// Invalid withdrawal amount
    InvalidWithdrawalAmount = 3,

    /// Arithmetic overflow in computation
    ArithmeticOverflow = 4,

    /// Invalid hub authority (CPI caller is not authorized)
    InvalidHubAuthority = 5,

    /// Invalid account owner
    InvalidAccountOwner = 6,

    /// Invalid vault account
    InvalidVault = 7,

    /// Invalid token account mint
    InvalidMint = 8,

    /// Invalid pool config account
    InvalidPoolConfig = 9,

    /// Exchange rate is stale (needs harvest)
    StaleExchangeRate = 10,

    /// Deposit limit exceeded
    DepositLimitExceeded = 11,

    /// Relayer fee exceeds transfer amount
    RelayerFeeExceedsAmount = 12,

    /// Invalid instruction data
    InvalidInstructionData = 13,
}

impl PoolError {
    /// Convert to error code
    pub const fn to_u32(self) -> u32 {
        self as u32
    }

    /// Create from error code
    pub fn from_u32(code: u32) -> Option<Self> {
        match code {
            0 => Some(Self::PoolPaused),
            1 => Some(Self::InsufficientLiquidity),
            2 => Some(Self::InvalidDepositAmount),
            3 => Some(Self::InvalidWithdrawalAmount),
            4 => Some(Self::ArithmeticOverflow),
            5 => Some(Self::InvalidHubAuthority),
            6 => Some(Self::InvalidAccountOwner),
            7 => Some(Self::InvalidVault),
            8 => Some(Self::InvalidMint),
            9 => Some(Self::InvalidPoolConfig),
            10 => Some(Self::StaleExchangeRate),
            11 => Some(Self::DepositLimitExceeded),
            12 => Some(Self::RelayerFeeExceedsAmount),
            13 => Some(Self::InvalidInstructionData),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_roundtrip() {
        let error = PoolError::InsufficientLiquidity;
        let code = error.to_u32();
        assert_eq!(PoolError::from_u32(code), Some(error));
    }
}
