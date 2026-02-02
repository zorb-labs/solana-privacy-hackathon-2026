//! Admin/setup instructions for the shielded pool.
//!
//! This module contains instructions for pool initialization and configuration.

mod accept_authority;
mod initialize;
mod register_token_pool;
mod register_unified_sol_pool;
mod set_pool_config_active;
mod set_pool_paused;
mod transfer_authority;

// Re-export Accounts structs
pub use accept_authority::AcceptAuthorityAccounts;
pub use initialize::InitializeAccounts;
pub use register_token_pool::RegisterTokenPoolAccounts;
pub use register_unified_sol_pool::RegisterUnifiedSolPoolAccounts;
pub use set_pool_config_active::{SetPoolConfigActiveAccounts, SetPoolConfigActiveData};
pub use set_pool_paused::{SetPoolPausedAccounts, SetPoolPausedData};
pub use transfer_authority::TransferAuthorityAccounts;

// Re-export handlers (called by #[instructions] macro generated dispatch)
pub use accept_authority::process_accept_authority;
pub use initialize::process_initialize;
pub use register_token_pool::process_register_token_pool;
pub use register_unified_sol_pool::process_register_unified_sol_pool;
pub use set_pool_config_active::process_set_pool_config_active;
pub use set_pool_paused::process_set_pool_paused;
pub use transfer_authority::process_transfer_authority;
