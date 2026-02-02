//! Admin instructions for unified SOL pool management.
//!
//! These instructions are admin-gated and manage pool configuration.

mod accept_authority;
mod init_lst_config;
mod init_unified_sol_pool_config;
mod set_lst_config_active;
mod set_unified_sol_pool_config_active;
mod set_unified_sol_pool_config_fee_rates;
mod transfer_authority;

pub use accept_authority::{AcceptAuthorityAccounts, process_accept_authority};
pub use init_lst_config::{InitLstConfigAccounts, InitLstConfigData, process_init_lst_config};
pub use init_unified_sol_pool_config::{
    InitUnifiedSolPoolConfigAccounts, InitUnifiedSolPoolConfigData,
    process_init_unified_sol_pool_config,
};
pub use set_lst_config_active::{
    SetLstConfigActiveAccounts, SetLstConfigActiveData, process_set_lst_config_active,
};
pub use set_unified_sol_pool_config_active::{
    SetUnifiedSolPoolConfigActiveAccounts, SetUnifiedSolPoolConfigActiveData,
    process_set_unified_sol_pool_config_active,
};
pub use set_unified_sol_pool_config_fee_rates::{
    SetUnifiedSolPoolConfigFeeRatesAccounts, SetUnifiedSolPoolConfigFeeRatesData,
    process_set_unified_sol_pool_config_fee_rates,
};
pub use transfer_authority::{TransferAuthorityAccounts, process_transfer_authority};
