//! Execute Transact Instruction
//!
//! This is the core entry point for shielded transactions in the Zorb privacy protocol.
//! It verifies a Groth16 zero-knowledge proof and executes state changes for private
//! value transfers.
//!
//! # Circuit Architecture (Three-Tier Routing)
//!
//! The transaction circuit uses three tiers for privacy-preserving value routing:
//!
//! ```text
//! [PUBLIC TIER]
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │ Reward Registry (8 lines)              Public Deltas (2 lines)          │
//! │ (rewardAssetId, rewardAcc) pairs       (publicAssetId, publicAmount)    │
//! │ On-chain yield accumulators            Visible deposits/withdrawals     │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                       │                              │
//!                       │ rosterRewardLineSel          │ publicLineSlotSel
//!                       │ (one-hot private selector)   │ (one-hot private selector)
//!                       ▼                              ▼
//! [PRIVATE ROUTING TIER]
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │                        Roster (4 slots)                                  │
//! │     Private (rosterAssetId, rosterEnabled) slots for routing values     │
//! │     Each slot: Σ(input values) + publicDelta = Σ(output amounts)        │
//! └─────────────────────────────────────────────────────────────────────────┘
//!                       ▲                              ▲
//!                       │ inNoteSlotSel                │ outNoteSlotSel
//!                       │                              │
//! [NOTE TIER]
//! ┌─────────────────────────────────────────────────────────────────────────┐
//! │ Input Notes (4)                        Output Notes (4)                  │
//! │ Notes being spent (nullifiers)         Notes being created (commitments)│
//! └─────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Reward Registry: Privacy Over Transacted Assets
//!
//! The 8 reward registry lines (`rewardAssetId`, `rewardAcc`) are **NOT** for multiple
//! notes. They provide **privacy over which assets are actually being transacted**.
//!
//! ## How It Works (transaction.circom Section 3):
//!
//! 1. Client provides 8 (assetId, accumulator) pairs as public inputs
//! 2. Each **roster slot** (private) uses a one-hot selector `rosterRewardLineSel[j][k]`
//!    to privately select which reward line provides its accumulator
//! 3. The circuit enforces: `selectedAssetId == rosterAssetId[j]` via dot product
//! 4. An observer cannot determine which reward lines are actually used
//!
//! ```text
//! Reward Registry (PUBLIC)           Roster Slot j (PRIVATE)
//! ────────────────────────           ─────────────────────────
//! rewardAssetId[0..8]  ────────┐
//! rewardAcc[0..8]      ────────┤     rosterRewardLineSel[j][k] (one-hot)
//!                              │     selects k where rewardAssetId[k] == rosterAssetId[j]
//!                              └───► rosterGlobalAcc[j] = rewardAcc[k]
//! ```
//!
//! ## Contract Obligation:
//!
//! The contract MUST validate ALL 8 non-zero `rewardAssetId` lines against on-chain
//! pools, even though only a subset are privately selected. This ensures:
//! - All potential accumulators are fresh (not stale)
//! - Privacy is preserved (observer cannot determine which are used)
//!
//! # Public Inputs Summary
//!
//! | Signal | Count | Description |
//! |--------|-------|-------------|
//! | `commitmentRoot` | 1 | Merkle root (proves input notes exist) |
//! | `transactParamsHash` | 1 | SHA256 of bound TransactParams |
//! | `publicAssetId[i]` | 2 | Asset IDs for public deposit/withdrawal |
//! | `publicAmount[i]` | 2 | Pool boundary deltas (+deposit, -withdrawal) |
//! | `nullifiers[i]` | 4 | Nullifier hashes for spent notes |
//! | `commitments[i]` | 4 | Commitment hashes for new notes |
//! | `rewardAcc[i]` | 8 | Reward accumulators (privacy set) |
//! | `rewardAssetId[i]` | 8 | Asset IDs for reward registry (privacy set) |
//!
//! # Pool Loading Requirements
//!
//! `unique_asset_count` must include pools for:
//! 1. **Public Assets**: Non-zero `publicAssetId[i]` where `ext_amounts[i] != 0`
//! 2. **Reward Assets**: ALL non-zero `rewardAssetId[i]` (for privacy set validation)
//!
//! # Validation & Execution Flow
//!
//! ```text
//! process_execute_transact(instruction_data, accounts)
//! │
//! ├─── VALIDATION PHASE ───────────────────────────────────────────────────────
//! │
//! ├──► 1. build_asset_map(remaining_accounts, unique_asset_count)
//! │        FOR i IN 0..unique_asset_count:
//! │            Load PoolConfig → get pool_type
//! │            Token: +2 accounts (token_pool_config, vault)
//! │            UnifiedSol: +3 accounts (unified_sol_pool_config, lst_config, vault)
//! │        RETURN: asset_map[asset_id] → PoolAccounts
//! │
//! ├──► 2. validate_reward_accumulators (privacy set)
//! │        FOR i IN 0..8:
//! │            IF rewardAssetId[i] != 0:
//! │                REQUIRE asset_map.contains(rewardAssetId[i])
//! │                REQUIRE pool.is_active
//! │                REQUIRE rewardAcc[i] == pool.reward_accumulator
//! │
//! ├──► 3. validate_transact_proof
//! │        REQUIRE commitment_root ∈ tree.root_history
//! │        REQUIRE SHA256(params) mod Fr == transactParamsHash
//! │        REQUIRE Groth16.verify(proof, TRANSACT_VK, public_inputs)
//! │
//! ├──► 4. validate_public_slots (i IN 0..2)
//! │        IF publicAssetId[i] != 0 AND ext_amount[i] != 0:
//! │            REQUIRE params.asset_ids[i] == proof.publicAssetId[i]
//! │            REQUIRE publicAmount[i] == computed:
//! │                Token:      ext_amount - fee
//! │                UnifiedSol: φ(ext_amount) - fee, φ(e) = e * rate / 1e9
//! │            REQUIRE fee >= min_fee (pool.fee_rate * amount / 10000)
//! │            VALIDATE token accounts (owner, mint)
//! │
//! ├──► 5. validate_nullifier_non_membership
//! │        REQUIRE nullifier_root ∈ {current_root, epoch_roots}
//! │        REQUIRE Groth16.verify(nm_proof, NULLIFIER_NM_VK, [root, nullifiers])
//! │
//! ├─── EXECUTION PHASE ────────────────────────────────────────────────────────
//! │
//! ├──► 6. create_nullifier_pdas (double-spend prevention)
//! │        FOR i IN 0..4:
//! │            IF nullifiers[i] != 0:
//! │                CREATE PDA["nullifier", nullifiers[i]]
//! │                EMIT NewNullifierEvent { nullifier, index }
//! │
//! ├──► 7. execute_public_slots (i IN 0..2)
//! │        IF ext_amount[i] > 0 (deposit):
//! │            CPI pool.deposit(depositor_token → vault, ext_amount)
//! │            Pool updates: pending_deposits += net, pending_rewards += fee
//! │        IF ext_amount[i] < 0 (withdrawal):
//! │            CPI pool.withdraw(vault → recipient_token, |ext_amount| - relayer_fee)
//! │            CPI transfer(vault → relayer_token, relayer_fee) if relayer_fee > 0
//! │            Pool updates: pending_withdrawals += gross, pending_rewards += fee
//! │
//! ├──► 8. append_commitments
//! │        FOR i IN 0..4:
//! │            commitment_tree.append(commitments[i])
//! │            EMIT NewCommitmentEvent { index, commitment, encrypted_output }
//! │
//! └──► 9. append_receipt
//!          receipt_hash = SHA256(tx_type, slot, epoch, commitments, nullifiers, ...)
//!          receipt_tree.append(receipt_hash)
//!          EMIT NewReceiptEvent { index, hash, full_receipt_data }
//! ```

// =============================================================================
// Submodules (execute_transact helpers)
// =============================================================================
mod accounts;
mod public_slots;
mod deposit_escrow;
mod fee;
mod nullifier;
mod pool_config;
mod slot_validation;
mod tree_updates;
mod validators;

// Re-export types needed by parent module
pub use accounts::SlotPoolType;

use crate::{
    CommitmentMerkleTree,
    errors::ShieldedPoolError,
    instructions::types::{N_INS, N_OUTS, N_PUBLIC_LINES, N_REWARD_LINES},
    merkle_tree::MerkleTree,
    pda::{HUB_AUTHORITY_ADDRESS, find_nullifier_pda},
    state::{
        GlobalConfig, LstConfig, MAX_SESSION_DATA_LEN, NullifierIndexedTree, ReceiptMerkleTree,
        TokenPoolConfig, TransactSession, UnifiedSolPoolConfig,
        find_lst_config_pda, find_token_pool_config_pda, find_unified_sol_pool_config_pda,
    },
    utils::{self, compute_unified_sol_asset_id, verify_proof},
    validation::{require_token_account_owner, require_valid_token_account},
    verifying_keys::TRANSACT_VK,
};
use zorb_pool_interface::{TOKEN_POOL_PROGRAM_ID, UNIFIED_SOL_POOL_PROGRAM_ID};

use ark_bn254::Fr;
use ark_ff::PrimeField;
use light_hasher::Sha256;
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    pubkey::Pubkey,
    sysvars::Sysvar,
};
use pinocchio_contrib::AccountAssertions;

// Session data parsing (from parent transact module)
use super::session_data::parse_session_data;

// Local submodule imports
use accounts::{
    SlotAccounts, TokenSlotAccounts, UnifiedSolSlotAccounts,
    build_reward_config_map, require_reward_config,
};
use public_slots::execute_public_slots;
use nullifier::{verify_and_create_nullifier, verify_nullifier_non_membership_proof};
use slot_validation::validate_public_slots;
use tree_updates::{append_commitment, compute_receipt_and_hash, emit_receipt_event};
use validators::{validate_token_accumulator, validate_unified_sol_accumulator};

// ============================================================================
// Panchor Accounts Wrapper and Handler
// ============================================================================

/// Wrapper accounts struct for ExecuteTransact instruction.
///
/// Fixed accounts (15 total) are defined in this struct. Dynamic pool accounts
/// are loaded from remaining_accounts based on `unique_asset_count` in instruction data.
///
/// # Account Layout
/// ## Fixed Accounts (15 in struct)
/// - Core PDAs: transact_session, commitment_tree, receipt_tree, nullifier_indexed_tree, epoch_root_pda, global_config
/// - Nullifiers: nullifier_0..3 (N_INS = 4)
/// - Signers: relayer, payer (depositor signature no longer required with escrow flow)
/// - Programs: token_program, system_program
///
/// ## Dynamic Accounts (remaining_accounts)
/// Pool accounts loaded based on unique_asset_count instruction data.
/// Each unique asset contributes 3-4 accounts based on pool_type:
/// - Token pool (3): pool_config, token_pool_config, vault
/// - UnifiedSol pool (4): pool_config, unified_sol_pool_config, lst_config, vault
#[derive(Accounts)]
pub struct ExecuteTransactAccounts<'info> {
    /// Transact session PDA containing proof data
    #[account(mut)]
    pub transact_session: AccountLoader<'info, TransactSession>,

    /// Commitment merkle tree PDA
    #[account(mut)]
    pub commitment_tree: AccountLoader<'info, CommitmentMerkleTree>,

    /// Receipt merkle tree PDA
    #[account(mut)]
    pub receipt_tree: AccountLoader<'info, ReceiptMerkleTree>,

    /// Nullifier indexed merkle tree PDA
    #[account(mut)]
    pub nullifier_indexed_tree: AccountLoader<'info, NullifierIndexedTree>,

    /// Epoch root PDA for historical nullifier roots (optional, pass system_program if unused)
    ///
    /// Uses raw AccountInfo since this account is optional - callers pass the system program
    /// as a placeholder when not using historical roots. Owner validation is performed manually
    /// in the handler when the account is actually used.
    pub epoch_root_pda: &'info AccountInfo,

    /// Global pool configuration
    pub global_config: AccountLoader<'info, GlobalConfig>,

    /// Nullifier PDA 0 (initialized during execution)
    ///
    /// Uses raw AccountInfo since nullifier accounts are created/initialized during
    /// transaction execution. Owner validation happens after account creation.
    #[account(mut)]
    pub nullifier_0: &'info AccountInfo,

    /// Nullifier PDA 1 (initialized during execution)
    #[account(mut)]
    pub nullifier_1: &'info AccountInfo,

    /// Nullifier PDA 2 (initialized during execution)
    #[account(mut)]
    pub nullifier_2: &'info AccountInfo,

    /// Nullifier PDA 3 (initialized during execution)
    #[account(mut)]
    pub nullifier_3: &'info AccountInfo,

    /// Relayer account (conditional signer for relayed transactions)
    pub relayer: Signer<'info>,

    /// SPL Token program
    pub token_program: Program<'info, Token>,

    /// System program
    pub system_program: Program<'info, System>,

    /// Rent payer (signer)
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Shielded pool program account (required for self-CPI event emission)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
    // Remaining accounts contain dynamic pool accounts:
    // - Load `unique_asset_count` pool account groups from remaining_accounts
    // - Each group starts with PoolConfig which determines pool_type
    // - Token pool: 3 accounts (pool_config, token_pool_config, vault)
    // - UnifiedSol pool: 4 accounts (pool_config, unified_sol_pool_config, lst_config, vault)
}

/// Instruction data for ExecuteTransact.
///
/// # Account Loading Strategy
///
/// remaining_accounts is organized into sections:
/// 1. Reward configs (2 accounts each, keyed by asset_id)
/// 2. Slot accounts (8 or 9 accounts each, indexed by slot, includes escrow accounts)
/// 3. Hub authority (1 account)
///
/// This separation allows:
/// - Reward-only assets to skip vault loading (saves accounts)
/// - Unified SOL to properly handle multiple LSTs (slot-indexed)
/// - Escrow deposits for relayer-assisted single-tx UX (escrow accounts per slot)
///
/// # Slot Pool Type Values
///
/// The `slot_pool_type` field uses [`SlotPoolType`] discriminant values:
/// - `0` = None (inactive slot, 0 accounts)
/// - `1` = Token (8 accounts: pool_config, token_pool_config, vault + 3 escrow + 2 user tokens)
/// - `2` = UnifiedSol (9 accounts: pool_config, unified_sol_pool_config, lst_config, vault + 3 escrow + 2 user tokens)
///
/// Stored as `[u8; 2]` for bytemuck Pod compatibility. Clients should use
/// the `SlotPoolType` enum for type-safe construction.
///
/// # Per-Slot Escrow
///
/// Each slot has its own escrow accounts (escrow, escrow_vault_authority, escrow_token).
/// The nonce for escrow PDA derivation is read from the escrow account itself.
/// For withdrawals/transfers (ext_amount <= 0), escrow accounts are present but unused.
/// This keeps the account layout consistent across all slots.
///
/// For deposits (ext_amount > 0):
/// - Escrow PDA: ["deposit_escrow", depositor, escrow.nonce]
/// - Escrow vault authority: ["escrow_vault_authority", escrow]
/// - Escrow token: ATA of escrow_vault_authority
#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable, InstructionArgs, IdlType)]
pub struct ExecuteTransactData {
    /// Count of unique reward asset_ids to load (max 8).
    /// Each loads 2 accounts: pool_config + pool-specific config.
    pub unique_reward_config_count: u8,
    /// Pool type for each public slot (N_PUBLIC_LINES = 2).
    /// Uses [`SlotPoolType`] discriminant values (0=None, 1=Token, 2=UnifiedSol).
    /// Determines account count per slot: None=0, Token=9, UnifiedSol=10.
    pub slot_pool_type: [u8; N_PUBLIC_LINES],
    /// Padding for 8-byte alignment.
    pub _padding: [u8; 5],
}

/// Handler for ExecuteTransact instruction.
///
/// This is a shielded transaction that:
/// - Verifies a Groth16 ZK proof
/// - Updates commitment and receipt merkle trees
/// - Creates nullifier PDAs for double-spend prevention
/// - Executes deposits/withdrawals via pool program CPIs
///
/// # Execution Order (Audit Guide)
///
/// The handler is organized by logical dependencies - what MUST happen before what:
///
/// ```text
/// SETUP PHASE (zero-cost binding)
/// ├─ P1: Extract fixed account references from panchor
/// │
/// FAIL-FAST PHASE (cheap checks before expensive work)
/// ├─ P2: Parse session data (needed for all subsequent checks)
/// ├─ P3: Validate program account, data length, expiry, pause state
/// │
/// ACCOUNT LOADING PHASE (expensive, only after passing cheap checks)
/// ├─ P4: Load remaining_accounts (reward configs, slot accounts, hub authority)
/// │
/// RELAYER VALIDATION PHASE
/// ├─ P5: Validate relayer pubkey match, signer, token accounts
/// │
/// PROOF-INDEPENDENT VALIDATION PHASE
/// ├─ P6: Validate reward accumulators (privacy set)
/// ├─ P7: Validate commitment root is known
/// ├─ P8: Validate transact params hash
/// │
/// ZK PROOF VERIFICATION PHASE
/// ├─ P9: Validate nullifier PDA keys (fail-fast before expensive proof)
/// ├─ P10: Verify Groth16 transact proof
/// │
/// PER-SLOT VALIDATION PHASE
/// ├─ P11: Validate public slots (amounts, fees, token accounts, escrow)
/// │
/// NULLIFIER VALIDATION PHASE
/// ├─ P12: Verify nullifier non-membership ZK proof
/// │
/// === EXECUTION PHASE (state changes begin) ===
/// │
/// ├─ E1: Create nullifier PDAs (double-spend prevention)
/// ├─ E2: Execute pool CPIs (deposits/withdrawals)
/// ├─ E3: Append commitments to tree
/// └─ E4: Append receipt to tree and emit event
/// ```
#[inline(never)]
pub fn process_execute_transact(
    panchor_ctx: Context<ExecuteTransactAccounts>,
    data: ExecuteTransactData,
) -> ProgramResult {
    let accounts = panchor_ctx.accounts;
    let program_id = &crate::ID;

    // ========================================================================
    // P1: SETUP - Extract fixed account references
    // ========================================================================
    // Zero-cost binding of panchor-validated accounts to local variables.
    // Panchor has already validated ownership and deserialization.

    let transact_session = accounts.transact_session.account_info();
    let commitment_tree = accounts.commitment_tree.account_info();
    let epoch_root_pda = accounts.epoch_root_pda;
    let global_config = accounts.global_config.account_info();
    let relayer = accounts.relayer.account_info();
    let token_program = accounts.token_program.account_info();
    let system_program = accounts.system_program.account_info();
    let payer = accounts.payer.account_info();
    let shielded_pool_program = accounts.shielded_pool_program;
    let nullifiers = [
        accounts.nullifier_0,
        accounts.nullifier_1,
        accounts.nullifier_2,
        accounts.nullifier_3,
    ];

    // ========================================================================
    // P2: PARSE SESSION - Extract all data needed for validation
    // ========================================================================
    // Session parsing is required for ALL subsequent validation, so do it early.
    // The borrow must be kept alive for zero-copy references in session.

    let session_data_ref = transact_session.try_borrow_data()?;
    let session = parse_session_data(&session_data_ref)?;

    // Extract session fields into local bindings (used throughout handler)
    let proof = session.proof;
    let transact_params = session.params;
    let nullifier_nm_proof = session.nullifier_nm_proof;
    let encrypted_outputs = &session.encrypted_outputs;
    let session_data_len = session.header.data_len;
    let has_relayer = session.has_relayer();

    // ========================================================================
    // P3: FAIL-FAST CHECKS - Cheap validation before expensive operations
    // ========================================================================
    // These checks are O(1) and should reject invalid transactions early,
    // before we spend compute on account loading or proof verification.

    // P3.1: Validate session data length is within bounds
    if session_data_len > MAX_SESSION_DATA_LEN {
        return Err(ShieldedPoolError::ProofPayloadOverflow.into());
    }

    // P3.3: Validate transaction not expired (load Clock once, reuse later)
    let clock = pinocchio::sysvars::clock::Clock::get()?;
    if transact_params.slot_expiry > 0 && clock.slot > transact_params.slot_expiry {
        return Err(ShieldedPoolError::TransactionExpired.into());
    }

    // P3.4: Validate relayer pubkey matches (simple equality, no account loading)
    // Defense-in-depth: ensures passed relayer account matches ZK-bound parameter
    if *relayer.key() != transact_params.relayer {
        return Err(ShieldedPoolError::Unauthorized.into());
    }

    // P3.5: Load global config and check pause state
    let global_config_data = accounts.global_config.load()?;
    if global_config_data.paused() {
        return Err(ShieldedPoolError::PoolPaused.into());
    }
    let global_config_bump = global_config_data.bump;

    // ========================================================================
    // P4: ACCOUNT LOADING - Parse remaining_accounts (expensive)
    // ========================================================================
    // Only performed after passing all cheap fail-fast checks.
    // Layout: [Reward Configs] [Slot Accounts] [Hub Authority]

    // Account layout in remaining_accounts (deterministic order):
    //   [0..R]     = Reward config accounts (R = unique_reward_config_count * 2)
    //   [R..S0]    = Slot 0 accounts (if slot_pool_type[0] != None)
    //   [S0..S1]   = Slot 1 accounts (if slot_pool_type[1] != None)
    //   [last]     = Hub authority
    let (reward_config_map, slot_accounts, hub_authority) = {
        let mut remaining_idx = 0;

        // Section 1: Build reward config map (for accumulator validation)
        let reward_result = build_reward_config_map(
            program_id,
            &panchor_ctx.remaining_accounts[remaining_idx..],
            data.unique_reward_config_count as usize,
        )?;
        let reward_config_map = reward_result.reward_config_map;
        remaining_idx += reward_result.accounts_consumed;

        // Section 2: Load slot accounts (inlined per-slot for auditability)
        //
        // Account counts per slot type:
        //   - None (0): 0 accounts
        //   - Token (1): 9 accounts  [pool_config, token_pool_config, vault, escrow, escrow_vault_auth, escrow_token, recipient_token, relayer_token, pool_program]
        //   - UnifiedSol (2): 10 accounts [pool_config, unified_sol_pool_config, lst_config, vault, escrow, escrow_vault_auth, escrow_token, recipient_token, relayer_token, pool_program]
        let mut slot_accounts: [Option<SlotAccounts>; N_PUBLIC_LINES] = [None; N_PUBLIC_LINES];
        let remaining = &panchor_ctx.remaining_accounts[remaining_idx..];
        // Reset remaining_idx to 0 since 'remaining' is already sliced from the reward config offset.
        // All subsequent indexing into 'remaining' should start from 0.
        remaining_idx = 0;

        // ════════════════════════════════════════════════════════════════════
        // SLOT 0
        // ════════════════════════════════════════════════════════════════════
        match SlotPoolType::from_u8(data.slot_pool_type[0]) {
            Some(SlotPoolType::Token) => {
                // Token: 9 accounts
                if remaining.len() < remaining_idx + 9 {
                    return Err(ShieldedPoolError::MissingAccounts.into());
                }
                let r = &remaining[remaining_idx..];

                // [1] token_pool_config: owner + PDA validation
                r[1].assert_owner(&TOKEN_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidAccountOwner)?;
                let token_config = AccountLoader::<TokenPoolConfig>::new(&r[1])
                    .map_err(|_| ShieldedPoolError::InvalidTokenConfig)?
                    .load()
                    .map_err(|_| ShieldedPoolError::InvalidTokenConfig)?;
                let (expected_pda, _) = find_token_pool_config_pda(&token_config.mint);
                r[1].assert_key(&expected_pda)
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfigPda)?;

                // [2] vault: valid token account
                require_valid_token_account(&r[2])?;

                // [8] pool_program: key validation
                r[8].assert_key(&TOKEN_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidProgramAccount)?;

                slot_accounts[0] = Some(SlotAccounts::Token(TokenSlotAccounts {
                    pool_config: &r[0],
                    token_pool_config: &r[1],
                    vault: &r[2],
                    escrow: &r[3],
                    escrow_vault_authority: &r[4],
                    escrow_token: &r[5],
                    recipient_token: &r[6],
                    relayer_token: &r[7],
                    pool_program: &r[8],
                }));
                remaining_idx += 9;
            }
            Some(SlotPoolType::UnifiedSol) => {
                // UnifiedSol: 10 accounts
                if remaining.len() < remaining_idx + 10 {
                    return Err(ShieldedPoolError::MissingAccounts.into());
                }
                let r = &remaining[remaining_idx..];

                // [1] unified_sol_pool_config: owner + PDA validation
                r[1].assert_owner(&UNIFIED_SOL_POOL_PROGRAM_ID)
                    .map_err(|_| {
                        ShieldedPoolError::InvalidAccountOwner
                    })?;
                let _unified_config = AccountLoader::<UnifiedSolPoolConfig>::new(&r[1])
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?
                    .load()
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?;
                let (expected_unified_pda, _) = find_unified_sol_pool_config_pda();
                r[1].assert_key(&expected_unified_pda)
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfigPda)?;

                // [2] lst_config: owner + PDA validation (CRITICAL for exchange rate)
                r[2].assert_owner(&UNIFIED_SOL_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidAccountOwner)?;
                let lst_config = AccountLoader::<LstConfig>::new(&r[2])
                    .map_err(|_| ShieldedPoolError::InvalidLstConfig)?
                    .load()
                    .map_err(|_| ShieldedPoolError::InvalidLstConfig)?;
                let (expected_lst_pda, _) = find_lst_config_pda(&lst_config.lst_mint);
                r[2].assert_key(&expected_lst_pda)
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfigPda)?;

                // [3] vault: valid token account
                require_valid_token_account(&r[3])?;

                // [9] pool_program: key validation
                r[9].assert_key(&UNIFIED_SOL_POOL_PROGRAM_ID)
                    .map_err(|_| {
                        ShieldedPoolError::InvalidProgramAccount
                    })?;

                slot_accounts[0] = Some(SlotAccounts::UnifiedSol(UnifiedSolSlotAccounts {
                    pool_config: &r[0],
                    unified_sol_pool_config: &r[1],
                    lst_config: &r[2],
                    vault: &r[3],
                    escrow: &r[4],
                    escrow_vault_authority: &r[5],
                    escrow_token: &r[6],
                    recipient_token: &r[7],
                    relayer_token: &r[8],
                    pool_program: &r[9],
                }));
                remaining_idx += 10;
            }
            Some(SlotPoolType::None) => {
                // Inactive slot - no accounts to load
            }
            None => return Err(ShieldedPoolError::InvalidPoolConfig.into()),
        }

        // ════════════════════════════════════════════════════════════════════
        // SLOT 1
        // ════════════════════════════════════════════════════════════════════
        match SlotPoolType::from_u8(data.slot_pool_type[1]) {
            Some(SlotPoolType::Token) => {
                // Token: 9 accounts
                if remaining.len() < remaining_idx + 9 {
                    return Err(ShieldedPoolError::MissingAccounts.into());
                }
                let r = &remaining[remaining_idx..];

                // [1] token_pool_config: owner + PDA validation
                r[1].assert_owner(&TOKEN_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidAccountOwner)?;
                let token_config = AccountLoader::<TokenPoolConfig>::new(&r[1])
                    .map_err(|_| ShieldedPoolError::InvalidTokenConfig)?
                    .load()
                    .map_err(|_| ShieldedPoolError::InvalidTokenConfig)?;
                let (expected_pda, _) = find_token_pool_config_pda(&token_config.mint);
                r[1].assert_key(&expected_pda)
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfigPda)?;

                // [2] vault: valid token account
                require_valid_token_account(&r[2])?;

                // [8] pool_program: key validation
                r[8].assert_key(&TOKEN_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidProgramAccount)?;

                slot_accounts[1] = Some(SlotAccounts::Token(TokenSlotAccounts {
                    pool_config: &r[0],
                    token_pool_config: &r[1],
                    vault: &r[2],
                    escrow: &r[3],
                    escrow_vault_authority: &r[4],
                    escrow_token: &r[5],
                    recipient_token: &r[6],
                    relayer_token: &r[7],
                    pool_program: &r[8],
                }));
                remaining_idx += 9;
            }
            Some(SlotPoolType::UnifiedSol) => {
                // UnifiedSol: 10 accounts
                if remaining.len() < remaining_idx + 10 {
                    return Err(ShieldedPoolError::MissingAccounts.into());
                }
                let r = &remaining[remaining_idx..];

                // [1] unified_sol_pool_config: owner + PDA validation
                r[1].assert_owner(&UNIFIED_SOL_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidAccountOwner)?;
                let _unified_config = AccountLoader::<UnifiedSolPoolConfig>::new(&r[1])
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?
                    .load()
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?;
                let (expected_unified_pda, _) = find_unified_sol_pool_config_pda();
                r[1].assert_key(&expected_unified_pda)
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfigPda)?;

                // [2] lst_config: owner + PDA validation (CRITICAL for exchange rate)
                r[2].assert_owner(&UNIFIED_SOL_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidAccountOwner)?;
                let lst_config = AccountLoader::<LstConfig>::new(&r[2])
                    .map_err(|_| ShieldedPoolError::InvalidLstConfig)?
                    .load()
                    .map_err(|_| ShieldedPoolError::InvalidLstConfig)?;
                let (expected_lst_pda, _) = find_lst_config_pda(&lst_config.lst_mint);
                r[2].assert_key(&expected_lst_pda)
                    .map_err(|_| ShieldedPoolError::InvalidPoolConfigPda)?;

                // [3] vault: valid token account
                require_valid_token_account(&r[3])?;

                // [9] pool_program: key validation
                r[9].assert_key(&UNIFIED_SOL_POOL_PROGRAM_ID)
                    .map_err(|_| ShieldedPoolError::InvalidProgramAccount)?;

                slot_accounts[1] = Some(SlotAccounts::UnifiedSol(UnifiedSolSlotAccounts {
                    pool_config: &r[0],
                    unified_sol_pool_config: &r[1],
                    lst_config: &r[2],
                    vault: &r[3],
                    escrow: &r[4],
                    escrow_vault_authority: &r[5],
                    escrow_token: &r[6],
                    recipient_token: &r[7],
                    relayer_token: &r[8],
                    pool_program: &r[9],
                }));
                remaining_idx += 10;
            }
            Some(SlotPoolType::None) => {
                // Inactive slot - no accounts to load
            }
            None => return Err(ShieldedPoolError::InvalidPoolConfig.into()),
        }

        // Section 3: Extract and validate hub_authority (always last)
        // Note: remaining_idx is now relative to 'remaining' (after slot accounts),
        // so we access remaining.get() not remaining_accounts.get()
        let hub_authority = remaining
            .get(remaining_idx)
            .ok_or(ShieldedPoolError::MissingAccounts)?;

        if hub_authority.key() != &HUB_AUTHORITY_ADDRESS {
            return Err(ShieldedPoolError::InvalidHubAuthority.into());
        }

        (reward_config_map, slot_accounts, hub_authority)
    };

    // ========================================================================
    // P5: RELAYER VALIDATION (Spec §5.2)
    // ========================================================================
    // Validates relayer signer and token accounts only if relayer fees are charged.
    // R10: relayer authorized, R11: token accounts correct

    if has_relayer {
        // R10: Relayer must be a signer if there are non-zero relayer fees
        relayer.assert_signer()?;

        // R10: Verify relayer pubkey is not zero (would send fees to uncontrolled address)
        if transact_params.relayer == Pubkey::default() {
            return Err(ShieldedPoolError::InvalidRelayer.into());
        }

        // R11: Validate relayer token accounts for slots with non-zero relayer fees
        for i in 0..N_PUBLIC_LINES {
            if transact_params.relayer_fees[i] > 0
                && let Some(slot) = &slot_accounts[i]
            {
                let relayer_token = slot.relayer_token();

                // Verify relayer_token is owned by SPL Token program
                crate::validation::require_token_program_owner(relayer_token)?;

                // Verify token account owner is the relayer
                require_token_account_owner(relayer_token, relayer.key())?;

                // Verify relayer_token is the canonical ATA for the relayer
                let mint = crate::token::get_token_account_mint(relayer_token)?;
                crate::token::require_associated_token_account(
                    relayer_token,
                    relayer.key(),
                    &mint,
                    relayer_token.owner(),
                )?;
            }
        }
    }

    // ========================================================================
    // P6: REWARD ACCUMULATOR VALIDATION (Spec §5.4)
    // ========================================================================
    // R6: reward accumulators match on-chain values
    // R12: pools are operational
    //
    // The 8 reward registry lines provide privacy over which assets are transacted.
    // ALL non-zero lines must be validated (not just the privately-selected ones).

    let unified_sol_asset_id = compute_unified_sol_asset_id();

    for i in 0..N_REWARD_LINES {
        let in_asset_id = &proof.reward_asset_id[i];
        let in_accumulator = &proof.reward_acc[i];

        if *in_asset_id == [0u8; 32] {
            continue;
        }

        let reward_config = require_reward_config(&reward_config_map, in_asset_id)?;

        if *in_asset_id == unified_sol_asset_id {
            validate_unified_sol_accumulator(reward_config, in_asset_id, in_accumulator)?;
        } else {
            validate_token_accumulator(reward_config, in_asset_id, in_accumulator)?;
        }
    }

    // ========================================================================
    // P7: COMMITMENT ROOT VALIDATION (Spec §5.5)
    // ========================================================================
    // R3: commitment root must be in the tree's root history

    {
        let commitment_tree_data = accounts.commitment_tree.load()?;
        if !MerkleTree::is_known_root(&commitment_tree_data, proof.commitment_root) {
            return Err(ShieldedPoolError::UnknownRoot.into());
        }
    }

    // ========================================================================
    // P8: TRANSACT PARAMS HASH VALIDATION (Spec §5.6)
    // ========================================================================
    // R2: SHA256(params) mod Fr must match the ZK public input

    let transact_params_hash = utils::calculate_transact_params_hash(transact_params);

    if Fr::from_le_bytes_mod_order(&transact_params_hash)
        != Fr::from_be_bytes_mod_order(&proof.transact_params_hash)
    {
        return Err(ShieldedPoolError::TransactParamsHashMismatch.into());
    }

    // ========================================================================
    // P9: NULLIFIER PDA KEY VALIDATION (fail-fast before expensive proof)
    // ========================================================================
    // Check nullifier account keys match expected PDAs (~200K CU savings if wrong)

    for i in 0..N_INS {
        let (expected_pda, _) = find_nullifier_pda(&proof.nullifiers[i]);
        nullifiers[i].assert_key(&expected_pda)?;
    }

    // ========================================================================
    // P10: GROTH16 PROOF VERIFICATION (Spec §5.7)
    // ========================================================================
    // R1: Verify the main transact proof

    if !verify_proof(proof, TRANSACT_VK) {
        return Err(ShieldedPoolError::InvalidProof.into());
    }

    // ========================================================================
    // P11: PER-SLOT VALIDATION (Spec §5.8)
    // ========================================================================
    // R7: public amounts correct, R8: fees sufficient
    // R11: token accounts correct, R12: pools operational
    // Validates all configs and token accounts BEFORE state changes.

    let validation_result = validate_public_slots(
        proof,
        transact_params,
        &slot_accounts,
        unified_sol_asset_id,
    )?;
    let _accumulator_epoch = validation_result.accumulator_epoch;

    // ========================================================================
    // P12: NULLIFIER NON-MEMBERSHIP VALIDATION (Spec §5.9)
    // ========================================================================
    // R4: nullifier root valid
    // ZK proof verifies non-membership in indexed merkle tree (finalized epochs)

    {
        let tree = accounts.nullifier_indexed_tree.load()?;
        let has_epoch_root = epoch_root_pda.key() != &pinocchio_system::ID;
        let epoch_root_opt = if has_epoch_root {
            Some(epoch_root_pda)
        } else {
            None
        };
        verify_nullifier_non_membership_proof(
            &tree,
            epoch_root_opt,
            &proof.nullifiers,
            nullifier_nm_proof,
        )?;
    }

    // ========================================================================
    // === EXECUTION PHASE - State changes begin here ===
    // ========================================================================
    // All validation complete. From this point, any failure leaves partial state.
    // Operations are ordered to minimize damage from failures:
    // 1. Nullifier PDAs (prevents double-spend)
    // 2. Pool CPIs (transfers tokens)
    // 3. Commitment tree (records new notes)
    // 4. Receipt tree (audit trail)

    // ========================================================================
    // E1: NULLIFIER PDA CREATION (Spec §5.10)
    // ========================================================================
    // R5: nullifier PDAs must be uninitialized (double-spend prevention)
    // Creating PDA fails if already exists = double-spend attempt

    let starting_pending_index = {
        let mut tree = accounts.nullifier_indexed_tree.load_mut()?;
        let start_idx = tree.next_pending_index;
        tree.next_pending_index = tree
            .next_pending_index
            .checked_add(N_INS as u64)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
        start_idx
    };

    for i in 0..N_INS {
        verify_and_create_nullifier(
            nullifiers[i],
            payer,
            system_program,
            &proof.nullifiers[i],
            commitment_tree,
            global_config,
            shielded_pool_program,
            global_config_bump,
            starting_pending_index + i as u64,
        )?;
    }

    // ========================================================================
    // E2: POOL CPIs (Spec §6.1)
    // ========================================================================
    // Executes deposits/withdrawals via CPI to pool programs.
    // Escrow verification and consumption is done per-slot for deposits.

    execute_public_slots(
        program_id,
        &slot_accounts,
        token_program,
        hub_authority,
        transact_params,
        &*session_data_ref,
        relayer.key(),
    )?;

    // ========================================================================
    // E3: COMMITMENT TREE UPDATES (Spec §6.2)
    // ========================================================================
    // Appends new commitments and emits NewCommitmentEvent for each

    let (new_root, last_commitment_index) = {
        let mut commitment_tree_data = accounts.commitment_tree.load_mut()?;
        for i in 0..N_OUTS {
            append_commitment(
                program_id,
                &mut commitment_tree_data,
                proof.commitments[i],
                encrypted_outputs[i],
                global_config,
                shielded_pool_program,
                global_config_bump,
            )?;
        }
        let new_root = commitment_tree_data.root;
        let last_commitment_index = commitment_tree_data.next_index - 1;
        (new_root, last_commitment_index)
    };

    // ========================================================================
    // E4: RECEIPT TREE UPDATE (Spec §6.3)
    // ========================================================================
    // Computes receipt hash, appends to tree, and emits event

    let slot = clock.slot;
    let epoch = clock.epoch;

    let (receipt, receipt_hash) = compute_receipt_and_hash(
        slot,
        epoch,
        new_root,
        last_commitment_index,
        proof,
        transact_params_hash,
    )?;

    let receipt_index = {
        let mut receipt_tree_data = accounts.receipt_tree.load_mut()?;
        let receipt_index = receipt_tree_data.next_index;
        receipt_tree_data.append::<Sha256>(receipt_hash)?;
        receipt_index
    };

    emit_receipt_event(
        program_id,
        receipt_index,
        &receipt,
        receipt_hash,
        global_config,
        shielded_pool_program,
        global_config_bump,
    )?;

    Ok(())
}



