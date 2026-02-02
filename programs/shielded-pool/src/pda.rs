//! Program Derived Address (PDA) helpers
//!
//! All PDAs are derived using standardized seeds for each account type.
//!
//! # Generated Functions
//!
//! The `#[pdas]` macro generates the following for each PDA variant:
//! - `X_SEED` - The seed constant as a byte string literal
//! - `find_x_pda(...)` - Derives the PDA address and bump
//! - `gen_x_seeds(...)` - Creates signer seeds for CPIs
//!
//! For unit variants (like GlobalConfig), it also generates:
//! - `X_ADDRESS` - Compile-time derived PDA address
//! - `X_BUMP` - Compile-time derived bump

use panchor::pdas;
use pinocchio::pubkey::Pubkey;

/// PDA variants for the Shielded Pool program
#[pdas]
pub enum ShieldedPoolPdas {
    /// Global config singleton
    #[seeds("global_config")]
    GlobalConfig,

    /// Commitment merkle tree singleton
    #[seeds("commitment_tree")]
    CommitmentTree,

    /// Receipt merkle tree singleton
    #[seeds("receipt_tree")]
    ReceiptTree,

    /// Nullifier PDA - per nullifier value
    #[seeds("nullifier")]
    Nullifier {
        /// The 32-byte nullifier value
        nullifier: [u8; 32],
    },

    /// Nullifier indexed tree singleton
    #[seeds("nullifier_tree")]
    NullifierTree,

    /// Transact session PDA - per user, per nonce
    #[seeds("transact_session")]
    TransactSession {
        /// The authority (user) pubkey
        authority: Pubkey,
        /// Session nonce for uniqueness
        nonce: u64,
    },

    /// Nullifier epoch root PDA - per nullifier epoch number
    #[seeds("nullifier_epoch_root")]
    NullifierEpochRoot {
        /// The nullifier epoch number
        nullifier_epoch: u64,
    },

    /// Hub authority singleton - delegate for pool vault withdrawals
    ///
    /// Used in the delegation model where pools approve this PDA as delegate
    /// for vault transfers, allowing the hub to execute all token movements.
    #[seeds("hub_authority")]
    HubAuthority,

    /// Pool config PDA - per asset_id
    ///
    /// Hub's routing configuration that maps asset_ids to pool programs.
    #[seeds("pool_config")]
    PoolConfig {
        /// The 32-byte asset ID
        asset_id: [u8; 32],
    },

    /// Deposit escrow PDA - per depositor, per nonce
    ///
    /// Holds deposited tokens bound to a specific proof hash for relayer-assisted deposits.
    #[seeds("deposit_escrow")]
    DepositEscrow {
        /// The depositor pubkey
        depositor: Pubkey,
        /// Escrow nonce for uniqueness
        nonce: u64,
    },

    /// Escrow vault authority PDA - per escrow
    ///
    /// Authority for the escrow's token vault ATA. Used to sign transfers
    /// from the escrow vault during execute_transact.
    #[seeds("escrow_vault_authority")]
    EscrowVaultAuthority {
        /// The escrow account pubkey
        escrow: Pubkey,
    },
}
