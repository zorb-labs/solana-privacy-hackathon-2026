//! Zorb Pool Interface
//!
//! Shared types for communication between the shielded-pool hub and pool plugins.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    shielded-pool (Hub)                       │
//! │  • ZK proof verification                                     │
//! │  • Commitment/Receipt/Nullifier trees                        │
//! │  • Fee validation (hub-centric)                              │
//! │  • Orchestrates pool CPIs                                    │
//! └─────────────────────────────────────────────────────────────┘
//!               │                           │
//!               ▼                           ▼
//! ┌─────────────────────────┐   ┌─────────────────────────┐
//! │      token-pool         │   │    unified-sol-pool     │
//! │  • SPL token deposits   │   │  • Multi-LST support    │
//! │  • Token withdrawals    │   │  • Exchange rate mgmt   │
//! │  • Vault management     │   │  • Harvest appreciation │
//! └─────────────────────────┘   └─────────────────────────┘
//! ```
//!
//! # Fee Calculation
//!
//! The hub is responsible for all fee calculations using the universal formula:
//! ```text
//! fee = principal × rate / BASIS_POINTS
//! ```
//!
//! Pools expose their fee rates via their config accounts. The hub reads these
//! rates, computes expected fees, validates user-provided fees, and passes
//! pre-computed amounts to pools via CPI.
//!
//! # Modules
//!
//! - [`types`]: Core types (PoolType, PoolInfo, DepositParams, WithdrawParams)
//! - [`cpi`]: CPI instruction builders for invoking pool programs
//! - [`error`]: Pool error types
//! - [`program_ids`]: Pool program ID constants

#![no_std]

pub mod asset_ids;
pub mod authority;
mod cpi;
mod error;
mod program_ids;
mod types;

pub use cpi::*;
pub use error::*;
pub use program_ids::*;
pub use types::*;
