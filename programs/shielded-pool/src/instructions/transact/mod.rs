//! Transact flow instructions (session-based).
//!
//! This module contains the instructions for the shielded transaction flow:
//! 1. init_transact_session - Create a session for uploading transaction data
//! 2. upload_transact_chunk - Upload transaction data in chunks
//! 3. execute_transact - Execute the shielded transaction
//! 4. close_transact_session - Close and reclaim session account
//!
//! # Module Organization
//!
//! ## Session Flow
//! Instructions for managing transact sessions (data upload):
//! - `session_data.rs` - Session account structure and parsing
//! - `init_transact_session.rs` - Create session account
//! - `upload_transact_chunk.rs` - Upload proof/params in chunks
//! - `close_transact_session.rs` - Close and reclaim lamports
//!
//! ## Execute Flow
//! Core execution and helpers are organized under `execute_transact/`:
//! ```text
//! execute_transact/
//! ├── mod.rs           - Main instruction handler (verifies proof, executes state changes)
//! ├── slot_validation  - Per-slot validation (amounts, fees, accumulators)
//! ├── nullifier        - Non-membership proof verification and PDA creation
//! ├── tree_updates     - Commitment and receipt tree state updates
//! ├── cpi_execution    - Pool CPI execution for deposits/withdrawals
//! ├── validators       - Accumulator validation for reward registry
//! ├── accounts         - Account structures (SlotAccounts, RewardConfig, etc.)
//! ├── pool_config      - Pool configuration abstraction (Token vs UnifiedSol)
//! ├── fee              - Fee calculation helpers (basis point arithmetic)
//! └── escrow           - Escrow verification for deposit authorization
//! ```
//!
//! # Dependency Graph
//!
//! ```text
//! execute_transact/mod
//!     ├── slot_validation ──┬── validators ──┬── pool_config
//!     │                     │                └── accounts
//!     │                     ├── fee
//!     │                     └── accounts
//!     ├── nullifier
//!     ├── tree_updates
//!     ├── cpi_execution ──┬── fee
//!     │                   ├── escrow
//!     │                   └── accounts
//!     └── validators
//! ```

// =============================================================================
// Session Flow
// =============================================================================
mod close_transact_session;
mod init_transact_session;
mod session_data;
mod upload_transact_chunk;

// =============================================================================
// Execute Transact
// =============================================================================
// Core entry point (helper modules are declared inside execute_transact/mod.rs)
mod execute_transact;

// Re-export all public items from instruction modules
pub use close_transact_session::*;
pub use execute_transact::{
    ExecuteTransactAccounts, ExecuteTransactData, process_execute_transact,
};
pub use init_transact_session::*;
pub use session_data::{MIN_SESSION_DATA_SIZE, SessionData, parse_session_data};
pub use upload_transact_chunk::*;

// Re-export helper structs from execute_transact module
pub use execute_transact::SlotPoolType;
