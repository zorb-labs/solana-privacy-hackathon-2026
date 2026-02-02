//! Escrow instructions for relayer-assisted deposits.
//!
//! This module provides instructions for creating and managing deposit escrows.
//! Escrows enable single-transaction UX for deposits by allowing users to
//! pre-commit tokens that a relayer can later use during execute_transact.

mod init_deposit_escrow;
mod close_deposit_escrow;

pub use init_deposit_escrow::*;
pub use close_deposit_escrow::*;
