//! Single source of truth for all Zorb protocol program IDs.
//!
//! This crate defines program IDs as `&'static str` constants that can be used
//! at compile time by the panchor `program!` macro and other crates.
//!
//! # Feature Flags
//!
//! - `devnet` - Use devnet program IDs
//! - `mainnet` - Use mainnet program IDs (default)
//! - `localnet` - Use localnet program IDs (same as mainnet)
//!
//! # Usage
//!
//! ```rust,ignore
//! // In program lib.rs
//! panchor::program! {
//!     id = zorb_program_ids::SHIELDED_POOL_ID,
//!     instructions = MyInstruction,
//! }
//! ```

#![no_std]

// =============================================================================
// Shielded Pool (Hub) Program ID
// =============================================================================

/// Shielded Pool program ID (devnet).
///
/// Hub program for ZK shielded transactions with Groth16 verification.
#[cfg(feature = "devnet")]
pub const SHIELDED_POOL_ID: &str = "Ar4QfyyGcZENwwHcYA8d45XcnjtjcaWBSHzEzvyAP5dT";

/// Shielded Pool program ID (mainnet/localnet).
///
/// Hub program for ZK shielded transactions with Groth16 verification.
#[cfg(not(feature = "devnet"))]
pub const SHIELDED_POOL_ID: &str = "zrbus1K97oD9wzzygehPBZMh5EVXPturZNgbfoTig5Z";

// =============================================================================
// Token Pool Program ID
// =============================================================================

/// Token Pool program ID (devnet).
///
/// Handles SPL token deposits and withdrawals with 1:1 exchange rate.
#[cfg(feature = "devnet")]
pub const TOKEN_POOL_ID: &str = "GomZwW2f2AyqTMfqRhNxbuN8RyEbArD4r93wY8zaRQxw";

/// Token Pool program ID (mainnet/localnet).
///
/// Handles SPL token deposits and withdrawals with 1:1 exchange rate.
#[cfg(not(feature = "devnet"))]
pub const TOKEN_POOL_ID: &str = "tokucUdUVP8k9xMS98cnVFmy4Yg3zkKMjfmGuYma8ah";

// =============================================================================
// Unified SOL Pool Program ID
// =============================================================================

/// Unified SOL Pool program ID (devnet).
///
/// Handles LST deposits and withdrawals with exchange rate conversion.
#[cfg(feature = "devnet")]
pub const UNIFIED_SOL_POOL_ID: &str = "5RvgA1AKJSp9dgWMStgU7ud7WJvDEVx1ybU3du9BUCya";

/// Unified SOL Pool program ID (mainnet/localnet).
///
/// Handles LST deposits and withdrawals with exchange rate conversion.
#[cfg(not(feature = "devnet"))]
pub const UNIFIED_SOL_POOL_ID: &str = "unixG6MuVwukHrmCbn4oE8LAPYKDfDMyNtNuMSEYJmi";

// =============================================================================
// Convenience re-exports with alternative names
// =============================================================================

/// Hub program ID (alias for SHIELDED_POOL_ID).
pub const HUB_PROGRAM_ID: &str = SHIELDED_POOL_ID;
