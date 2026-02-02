//! Per-slot validation for execute_transact.
//!
//! This module handles V8 validation (Spec §5.8) which validates each public slot
//! before any state-changing operations occur. This ensures all validation passes
//! before nullifiers are created ("not burned until validated").
//!
//! # Security Considerations
//! - Inactive slots must have zero values (prevents ghost value injection)
//! - Active slots must have non-zero values (ensures consistency)
//! - Pool configs are validated via PDA derivation (prevents spoofing)
//! - Public amounts are validated against ZK proof (prevents amount manipulation)
//! - Token accounts are validated for correct ownership and mint

use crate::{
    errors::ShieldedPoolError,
    instructions::types::{N_PUBLIC_LINES, TransactParams, TransactProofData},
    state::{
        HubPoolType, LstConfig, PoolConfig as HubPoolConfig, TokenPoolConfig,
        UnifiedSolPoolConfig,
    },
    validation::require_token_account_mint,
};
use panchor::prelude::AccountLoader;
use pinocchio::{ProgramResult, program_error::ProgramError, pubkey::Pubkey};

use super::accounts::SlotAccounts;
use super::pool_config::PoolConfig;
use super::validators::{
    validate_token_public_amount, validate_unified_sol_public_amount,
};

// ============================================================================
// Per-Slot Validation
// ============================================================================

/// Result of per-slot validation containing data needed for execution.
pub struct SlotValidationResult {
    /// Accumulator epoch captured from unified SOL config (for harvest validation)
    pub accumulator_epoch: u64,
}

/// Validate all public slots (V8: Spec §5.8).
///
/// This function validates:
/// - R7: public amounts match ZK proof
/// - R8: fees are sufficient
/// - R11: token accounts are correct
/// - R12: pools are operational
///
/// # Security
/// - ALL validation must pass BEFORE nullifier creation
/// - Inactive slots must have zero values in both proof and params
/// - Active slots must have non-zero values and matching configs
/// - Exchange rates are validated for unified SOL
///
/// # Arguments
/// * `proof` - The transact proof data containing public inputs
/// * `transact_params` - The transaction parameters
/// * `slot_accounts` - Loaded slot accounts for each public slot
/// * `unified_sol_asset_id` - The computed unified SOL asset ID
///
/// # Returns
/// * `Ok(SlotValidationResult)` - Validation passed, returns data needed for execution
/// * `Err(ProgramError)` - Validation failed
#[inline(never)]
pub fn validate_public_slots<'a>(
    proof: &TransactProofData,
    transact_params: &TransactParams,
    slot_accounts: &[Option<SlotAccounts<'a>>; N_PUBLIC_LINES],
    unified_sol_asset_id: [u8; 32],
) -> Result<SlotValidationResult, ProgramError> {
    let mut accumulator_epoch: u64 = 0;

    for i in 0..N_PUBLIC_LINES {
        let public_asset_id = proof.public_asset_ids[i];
        let ext_amount = transact_params.ext_amounts[i];

        // V8.PRE: Validate inactive slots have zero values
        if slot_accounts[i].is_none() {
            validate_inactive_slot(i, &public_asset_id, &proof.public_amounts[i], transact_params)?;
            continue;
        }

        // V8.PRE.2: Active slots must have non-zero values
        if public_asset_id == [0u8; 32] {
            return Err(ShieldedPoolError::InvalidSlotConfiguration.into());
        }
        if ext_amount == 0 {
            return Err(ShieldedPoolError::InvalidSlotConfiguration.into());
        }

        // V8.1: R7 - Validate asset_ids match between proof and transact_params
        if transact_params.asset_ids[i] != public_asset_id {
            return Err(ShieldedPoolError::InvalidAssetId.into());
        }

        // Get slot accounts for this slot
        let slot = slot_accounts[i]
            .as_ref()
            .ok_or(ShieldedPoolError::MissingAccounts)?;

        let fee = transact_params.fees[i];
        let recipient = transact_params.recipients[i];
        let is_unified = public_asset_id == unified_sol_asset_id;

        // V8.0: Defense-in-depth - Validate hub pool_config
        validate_hub_pool_config(slot)?;

        // Load config and construct PoolConfig based on slot type
        let pool = load_and_validate_pool_config(slot, is_unified, &mut accumulator_epoch)?;

        // Relayer fee used directly - if too high, transfer will fail naturally
        let relayer_fee = transact_params.relayer_fees[i];

        // V8.3/V8.4: R7 (public amounts), R8 (fees), R12 (is_active)
        let public_amount = proof.public_amounts[i];

        if is_unified {
            validate_unified_sol_public_amount(&pool, ext_amount, fee, relayer_fee, public_amount)?;
        } else {
            validate_token_public_amount(&pool, ext_amount, fee, relayer_fee, public_amount)?;
        }

        // V8.3.1/V8.4.1: R11 - Mint validation
        if pool.vault_mint() != transact_params.mints[i] {
            return Err(ShieldedPoolError::InvalidMint.into());
        }

        // V8.3.2/V8.4.2: R11 - Vault validation
        if pool.expected_vault_address() != *slot.vault().key() {
            return Err(ShieldedPoolError::InvalidVault.into());
        }

        // Defense-in-depth: Validate vault mint matches expected
        require_token_account_mint(slot.vault(), &pool.vault_mint())?;

        // V8.3.5/V8.4.5: R12 - Pool-specific deposit validation
        if ext_amount > 0 {
            pool.validate_deposit(ext_amount as u64, accumulator_epoch)?;
        }

        // V8.3.6-9/V8.4.6-9: R11 - Token account validations
        validate_slot_token_accounts(
            slot,
            &pool.vault_mint(),
            &recipient,
            relayer_fee,
            ext_amount,
        )?;
    }

    Ok(SlotValidationResult { accumulator_epoch })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Validate that an inactive slot has zero values in proof and params.
///
/// # Security
/// - Prevents ZK proof from claiming value in a slot that won't be executed
/// - All slot-related fields must be zero/default
#[inline(never)]
fn validate_inactive_slot(
    slot_index: usize,
    public_asset_id: &[u8; 32],
    public_amount: &[u8; 32],
    transact_params: &TransactParams,
) -> Result<(), ProgramError> {
    const ZERO_BYTES: [u8; 32] = [0u8; 32];
    let i = slot_index;

    // All fields must be zero/default for inactive slots
    let is_valid = *public_asset_id == ZERO_BYTES
        && *public_amount == ZERO_BYTES
        && transact_params.ext_amounts[i] == 0
        && transact_params.asset_ids[i] == ZERO_BYTES
        && transact_params.mints[i] == Pubkey::default()
        && transact_params.fees[i] == 0
        && transact_params.recipients[i] == Pubkey::default()
        && transact_params.relayer_fees[i] == 0;

    if !is_valid {
        return Err(ShieldedPoolError::InvalidSlotConfiguration.into());
    }
    Ok(())
}

/// Validate hub pool_config owner and pool_type match slot type.
///
/// # Security
/// - Verifies hub's PoolConfig matches the expected pool type based on SlotAccounts
#[inline(never)]
fn validate_hub_pool_config(slot: &SlotAccounts) -> Result<(), ProgramError> {
    let hub_config = AccountLoader::<HubPoolConfig>::new(slot.pool_config())
        .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?
        .load()
        .map_err(|_| ShieldedPoolError::InvalidPoolConfig)?;

    // Verify pool_type matches the SlotAccounts variant
    let expected_pool_type = match slot {
        SlotAccounts::Token(_) => HubPoolType::Token,
        SlotAccounts::UnifiedSol(_) => HubPoolType::UnifiedSol,
    };
    let actual_pool_type =
        HubPoolType::from_u8(hub_config.pool_type).ok_or(ShieldedPoolError::InvalidPoolConfig)?;
    if actual_pool_type != expected_pool_type {
        return Err(ShieldedPoolError::InvalidPoolConfig.into());
    }

    Ok(())
}

/// Load and validate pool config based on slot type.
///
/// # Security
/// - PDA derivation already verified in load_slot_accounts (accounts.rs)
/// - Asset ID validation prevents cross-pool attacks
/// - Captures accumulator_epoch for unified SOL harvest validation
///
/// # Note on PDA Validation
/// PDA validation is performed in load_slot_accounts during account loading.
/// This function only loads config data and validates the is_unified flag.
#[inline(never)]
fn load_and_validate_pool_config<'a>(
    slot: &SlotAccounts<'a>,
    is_unified: bool,
    accumulator_epoch: &mut u64,
) -> Result<PoolConfig<'a>, ProgramError> {
    match slot {
        SlotAccounts::UnifiedSol(unified) => {
            let unified_config =
                AccountLoader::<UnifiedSolPoolConfig>::new(unified.unified_sol_pool_config)?
                    .load()?;

            // Capture reward_epoch for harvest validation
            *accumulator_epoch = unified_config.reward_epoch;

            // PDA validation already done in load_slot_accounts (accounts.rs L980-993, L1020-1033)

            let lst_config = AccountLoader::<LstConfig>::new(unified.lst_config)?.load()?;

            // Validate is_unified flag matches asset type
            if !is_unified {
                return Err(ShieldedPoolError::InvalidAssetId.into());
            }

            Ok(PoolConfig::UnifiedSol {
                unified_config,
                lst_config,
            })
        }
        SlotAccounts::Token(token) => {
            let token_config =
                AccountLoader::<TokenPoolConfig>::new(token.token_pool_config)?.load()?;

            // PDA validation already done in load_slot_accounts (accounts.rs L742-755)

            // Validate is_unified flag matches asset type
            if is_unified {
                return Err(ShieldedPoolError::InvalidAssetId.into());
            }

            Ok(PoolConfig::Token {
                config: token_config,
            })
        }
    }
}

/// Validate token accounts for a slot.
///
/// # Security
/// - Recipient token address must match transact_params (ZK-bound)
/// - For withdrawals: recipient token must be valid SPL token with correct mint
/// - Relayer token mint is validated (owner validated in V2)
///
/// # Escrow Flow
/// - All deposits now use per-slot escrow accounts
/// - Escrow validation is done in `execute_public_slots` via `verify_escrow_for_deposit`
#[inline(never)]
pub fn validate_slot_token_accounts(
    slot: &SlotAccounts,
    mint: &Pubkey,
    recipient: &Pubkey,
    relayer_fee: u64,
    ext_amount: i64,
) -> ProgramResult {
    // Recipient token address validation - ALWAYS validate
    if slot.recipient_token().key() != recipient {
        return Err(ShieldedPoolError::RecipientMismatch.into());
    }

    // Additional recipient validation for withdrawals
    if ext_amount < 0 {
        let withdrawal_amount = (-ext_amount) as u64;
        let recipient_amount = withdrawal_amount
            .checked_sub(relayer_fee)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

        // Only validate recipient token account if recipient actually receives tokens
        if recipient_amount > 0 {
            // Validate recipient is not zero pubkey
            if *recipient == Pubkey::default() {
                return Err(ShieldedPoolError::InvalidRecipient.into());
            }

            // Validate recipient_token is owned by SPL Token program
            crate::validation::require_token_program_owner(slot.recipient_token())?;

            // Validate recipient_token mint matches expected
            require_token_account_mint(slot.recipient_token(), mint)?;
        }
    }

    // Relayer token mint validation (owner validated in V2)
    if relayer_fee > 0 {
        require_token_account_mint(slot.relayer_token(), mint)?;
    }

    Ok(())
}
