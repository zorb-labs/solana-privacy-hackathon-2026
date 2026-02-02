//! Admin instructions for token pool management.
//!
//! These instructions are admin-gated and manage pool configuration.

mod accept_authority;
mod init_pool;
mod set_fee_rates;
mod set_pool_active;
mod transfer_authority;

pub use accept_authority::{AcceptAuthorityAccounts, process_accept_authority};
pub use init_pool::{InitPoolAccounts, InitPoolData, process_init_pool};
pub use set_fee_rates::{SetFeeRatesAccounts, SetFeeRatesData, process_set_fee_rates};
pub use set_pool_active::{SetPoolActiveAccounts, SetPoolActiveData, process_set_pool_active};
pub use transfer_authority::{TransferAuthorityAccounts, process_transfer_authority};
