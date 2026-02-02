//! Initialize the shielded pool program.

use crate::{
    events::{PoolInitializedEvent, emit_event},
    indexed_merkle_tree::IndexedMerkleTree,
    merkle_tree::MerkleTree,
    pda::gen_global_config_seeds,
    state::{
        COMMITMENT_TREE_HEIGHT, CommitmentMerkleTree, GlobalConfig, NULLIFIER_TREE_HEIGHT,
        NullifierIndexedTree, RECEIPT_TREE_HEIGHT, ReceiptMerkleTree, ROOT_HISTORY_SIZE,
    },
};
use light_hasher::{Poseidon, Sha256};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    sysvars::{Sysvar, clock::Clock},
};

/// Accounts for the Initialize instruction.
///
/// Uses panchor's init pattern to automatically create PDAs with proper bumps.
#[derive(Accounts)]
pub struct InitializeAccounts<'info> {
    /// Commitment tree PDA ["commitment_tree"], created by this instruction
    #[account(init, payer = authority, pda = CommitmentTree)]
    pub commitment_tree: AccountLoader<'info, CommitmentMerkleTree>,

    /// Global config PDA ["global_config"], created by this instruction
    #[account(init, payer = authority, pda = GlobalConfig)]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Receipt tree PDA ["receipt_tree"], created by this instruction
    #[account(init, payer = authority, pda = ReceiptTree)]
    pub receipt_tree: AccountLoader<'info, ReceiptMerkleTree>,

    /// Nullifier tree PDA ["nullifier_tree"], created by this instruction
    #[account(init, payer = authority, pda = NullifierTree)]
    pub nullifier_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// Payer and future pool authority
    #[account(mut)]
    pub authority: Signer<'info>,

    /// System program for account creation
    pub system_program: Program<'info, System>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Initialize the shielded pool program.
///
/// Creates four PDA accounts that store the pool's global state:
/// - CommitmentMerkleTree: Stores note commitments for the shielded pool
/// - GlobalConfig: Stores pool-wide configuration (authority, pause state)
/// - ReceiptMerkleTree: Stores transaction receipts for compliance
/// - NullifierIndexedTree: Stores nullifiers for double-spend prevention
///
/// # Epoch Handling
///
/// The nullifier tree starts at epoch 0 with no epoch root accounts.
/// The first `advance_nullifier_epoch` call will create EpochRoot(0) and
/// advance to epoch 1. This follows the principle that genesis state is
/// implicit rather than explicitly stored.
///
/// # Notes
///
/// - The authority becomes the pool authority and can register assets and update config
/// - Token configurations must be added separately via the RegisterAsset instruction
/// - Account creation is handled automatically by panchor's init pattern
pub fn process_initialize(ctx: Context<InitializeAccounts>) -> ProgramResult {
    let InitializeAccounts {
        commitment_tree,
        global_config,
        receipt_tree,
        nullifier_tree,
        authority,
        system_program: _,
        shielded_pool_program,
    } = ctx.accounts;

    // Get bumps from context (populated by panchor's init)
    let bumps = ctx.bumps;

    // Get current slot for event
    let clock = Clock::get()?;

    // Initialize commitment merkle tree
    {
        let mut tree_data = commitment_tree.load_mut()?;
        tree_data.authority = *authority.key();
        tree_data.next_index = 0;
        tree_data.root_index = 0;
        tree_data.bump = bumps.commitment_tree;
        tree_data.height = COMMITMENT_TREE_HEIGHT;
        tree_data.root_history_size = ROOT_HISTORY_SIZE as u16;
        MerkleTree::initialize::<Poseidon>(&mut tree_data)?;
    }

    // Initialize global config
    {
        let mut config = global_config.load_mut()?;
        config.authority = *authority.key();
        config.is_paused = 0;
        config.bump = bumps.global_config;
    }

    // Initialize receipt merkle tree
    {
        let mut receipt_data = receipt_tree.load_mut()?;
        receipt_data.authority = *authority.key();
        receipt_data.next_index = 0;
        receipt_data.height = RECEIPT_TREE_HEIGHT;
        receipt_data.bump = bumps.receipt_tree;
        // AUDIT FIX (CRIT-01): Receipt tree uses SHA256 for leaf hashing and appends,
        // so initialization must also use SHA256 for consistent zero-level hashes.
        receipt_data.initialize::<Sha256>()?;
    }

    // Initialize nullifier indexed tree with genesis leaf
    //
    // The genesis leaf (value=0, next_value=0, next_index=0) occupies index 0 as a
    // sentinel in the sorted linked list. After initialization:
    // - next_index = 1 (genesis at 0, next insertion at 1)
    // - next_pending_index = 1 (first nullifier assigned gets index 1)
    // - current_epoch = 1 (epoch 0 is reserved as "not inserted" sentinel)
    // - earliest_provable_epoch = 1 (no epoch roots exist yet)
    // - last_finalized_index = 0 (no epoch snapshots taken yet)
    // - last_epoch_slot = 0 (allows immediate first epoch advance)
    //
    // Note: Epochs start at 1 (not 0) so that inserted_epoch = 0 can serve as
    // the sentinel for "nullifier not yet inserted" in Nullifier PDAs.
    //
    // Epoch roots are created lazily when advance_nullifier_epoch is called.
    {
        let mut tree = nullifier_tree.load_mut()?;
        tree.authority = *authority.key();
        tree.height = NULLIFIER_TREE_HEIGHT;
        tree.bump = bumps.nullifier_tree;
        tree.current_epoch = 1; // Start at 1, not 0 (0 = sentinel for "not inserted")
        tree.earliest_provable_epoch = 1;
        tree.last_finalized_index = 0;
        tree.last_epoch_slot = 0; // Allows immediate first epoch advance

        // Initialize with genesis leaf - sets next_index and next_pending_index to 1
        // Per Aztec spec: next_value=0 represents infinity (end of sorted list)
        IndexedMerkleTree::initialize::<Poseidon>(&mut tree)?;
    }

    // Emit genesis event
    let bump_bytes = [bumps.global_config];
    let seeds = gen_global_config_seeds(&bump_bytes);
    let signer = PinocchioSigner::from(&seeds);

    let event = PoolInitializedEvent {
        authority: *authority.key(),
        commitment_tree: *commitment_tree.account_info().key(),
        receipt_tree: *receipt_tree.account_info().key(),
        nullifier_tree: *nullifier_tree.account_info().key(),
        global_config: *global_config.account_info().key(),
        slot: clock.slot,
    };

    emit_event(
        global_config.account_info(),
        shielded_pool_program,
        signer,
        &event,
    )?;

    Ok(())
}
