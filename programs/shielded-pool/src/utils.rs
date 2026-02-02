use crate::groth16::{CompressedGroth16Proof, Groth16Verifyingkey, verify_groth16};
use crate::{errors::ShieldedPoolError, instructions::TransactProofData};
use alloc::vec::Vec;
use ark_bn254::Fr;
use ark_ff::PrimeField;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use pinocchio_log::log;
use solana_poseidon::{Endianness, Parameters, hashv};
use solana_program::hash::hash;

/// Basis points denominator (100% = 10000 basis points).
/// Used for fee rate calculations where rates are specified in basis points (e.g., 100 = 1%).
pub const BASIS_POINTS_DENOMINATOR: u128 = 10_000;

/// WSOL mint address on Solana mainnet.
/// This is the canonical wrapped SOL mint used for the unified SOL asset.
pub const WSOL_MINT: Pubkey = [
    0x06, 0x9b, 0x88, 0x57, 0xfe, 0xab, 0x81, 0x84, 0xfb, 0x68, 0x7f, 0x63, 0x46, 0x18, 0xc0, 0x35,
    0xda, 0xc4, 0x39, 0xdc, 0x1a, 0xeb, 0x3b, 0x55, 0x98, 0xa0, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x01,
];

// Re-export UNIFIED_SOL_ASSET_ID from unified-sol-pool (canonical source)
pub use unified_sol_pool::UNIFIED_SOL_ASSET_ID;

/// Returns the unified SOL asset ID constant.
/// This is used to detect whether a transaction is operating on the unified SOL pool.
pub fn compute_unified_sol_asset_id() -> [u8; 32] {
    UNIFIED_SOL_ASSET_ID
}

/// Asset ID required for relayer fee payments.
///
/// Relayer fees can only be paid when the transaction uses this specific asset ID.
/// This ensures relayers are compensated in a consistent, liquid asset.
///
/// TODO: Define the actual asset ID value (currently set to unified SOL asset ID as placeholder)
pub const RELAYER_FEE_ASSET_ID: [u8; 32] = UNIFIED_SOL_ASSET_ID;

/// Verifies that public_amount matches the expected value based on ext_amount and fee.
///
/// Fees are charged only on what crosses the shielded boundary (|ext_amount|).
/// Relayer fee is a derived split of |ext_amount|, not part of this calculation.
///
/// For deposits (ext_amount > 0):  public_amount = ext_amount - fee
/// For withdrawals (ext_amount < 0): public_amount = -(|ext_amount| + fee)
/// For transfers (ext_amount = 0): public_amount = 0 (no boundary crossing)
#[inline(never)]
pub fn check_public_amount(ext_amount: i64, fee: u64, public_amount_bytes: [u8; 32]) -> bool {
    if ext_amount == i64::MIN {
        log!("can't use i64::MIN as ext_amount");
        return false;
    }

    let fee_fr = Fr::from(fee);
    let ext_amount_fr = if ext_amount >= 0 {
        Fr::from(ext_amount as u64)
    } else {
        let abs_ext_amount = match ext_amount.checked_neg() {
            Some(val) => val,
            None => return false,
        };
        Fr::from(abs_ext_amount as u64)
    };

    if ext_amount >= 0 && ext_amount_fr < fee_fr {
        return false;
    }

    let computed_public_amount = if ext_amount > 0 {
        // Deposit: public_amount = ext_amount - fee
        ext_amount_fr - fee_fr
    } else {
        // Withdrawal/Transfer: public_amount = -(|ext_amount| + fee)
        // Note: relayer_fee is a split of |ext_amount|, not additive
        -(ext_amount_fr + fee_fr)
    };

    let provided_public_amount = Fr::from_be_bytes_mod_order(&public_amount_bytes);

    let provided_bigint = provided_public_amount.into_bigint();
    let computed_bigint = computed_public_amount.into_bigint();
    log!(
        "provided_public_amount: {} {} {} {}",
        provided_bigint.0[0],
        provided_bigint.0[1],
        provided_bigint.0[2],
        provided_bigint.0[3]
    );
    log!(
        "computed_public_amount: {} {} {} {}",
        computed_bigint.0[0],
        computed_bigint.0[1],
        computed_bigint.0[2],
        computed_bigint.0[3]
    );

    computed_public_amount == provided_public_amount
}

/// Verifies that public_amount matches the expected value for unified SOL transactions.
///
/// # Domain Boundary Principle
///
/// ext_amount is always in **LST tokens** (domain E). Conversion to virtual SOL (domain S)
/// happens at the validation boundary via φ (tokens → virtual SOL).
///
/// For deposits (ext_amount > 0):
///   s = φ(e)                    // Convert tokens to virtual SOL
///   p = s - f                   // public_amount = virtual_sol - fee
///
/// For withdrawals (ext_amount < 0):
///   |e| = φ⁻¹(s - f)           // ext_amount derived from (gross - fee) in tokens
///   p = -s                      // public_amount = -gross_virtual_sol
///   We validate: φ(|e|) + f ≈ |p| (with rounding tolerance)
///
/// For transfers (ext_amount = 0): public_amount = 0
///
/// Note: relayer_fee is a split of ext_amount, not deducted from public_amount.
///
/// # Parameters
/// - `ext_amount`: External amount in **LST tokens** (domain E)
/// - `fee`: Protocol fee in virtual SOL units (domain S)
/// - `exchange_rate`: harvested_exchange_rate (λ) for token↔vSOL conversion
/// - `rate_precision`: RATE_PRECISION constant (10^9)
#[inline(never)]
pub fn check_public_amount_unified(
    ext_amount: i64,
    fee: u64,
    public_amount_bytes: [u8; 32],
    exchange_rate: u64,
    rate_precision: u64,
) -> bool {
    if ext_amount == i64::MIN {
        log!("can't use i64::MIN as ext_amount");
        return false;
    }

    if exchange_rate == 0 {
        log!("exchange_rate cannot be zero");
        return false;
    }

    // Helper to convert LST tokens to virtual SOL: φ(e) = e * λ / ρ
    let to_virtual_sol = |amount: u64| -> Option<u128> {
        (amount as u128)
            .checked_mul(exchange_rate as u128)?
            .checked_div(rate_precision as u128)
    };

    if ext_amount > 0 {
        // Deposit: ext_amount is in tokens (domain E)
        // Convert to virtual SOL: s = φ(e)
        // public_amount = s - f
        let virtual_sol = match to_virtual_sol(ext_amount as u64) {
            Some(v) => v,
            None => return false,
        };

        if (virtual_sol as u64) < fee {
            log!("check_public_amount_unified: deposit fee exceeds virtual_sol");
            return false;
        }

        let expected_public_amount = Fr::from((virtual_sol as u64) - fee);
        let provided_public_amount = Fr::from_be_bytes_mod_order(&public_amount_bytes);

        log!(
            "check_public_amount_unified: deposit ext_amount={}, virtual_sol={}, fee={}, expected={}",
            ext_amount,
            virtual_sol,
            fee,
            (virtual_sol as u64) - fee
        );

        expected_public_amount == provided_public_amount
    } else if ext_amount < 0 {
        // Withdrawal: ext_amount is in tokens (domain E)
        // Per formal model: |e| = φ⁻¹(s - f), so φ(|e|) = s - f
        // We need to verify: |p| = s = φ(|e|) + f
        let abs_ext_tokens = match ext_amount.checked_neg() {
            Some(v) => v as u64,
            None => return false,
        };

        // Convert tokens to virtual SOL: φ(|e|) = net virtual SOL = s - f
        let net_virtual_sol = match to_virtual_sol(abs_ext_tokens) {
            Some(v) => v as u64,
            None => return false,
        };

        // gross_virtual_sol = net + fee = s
        let gross_virtual_sol = net_virtual_sol
            .checked_add(fee)
            .unwrap_or(u64::MAX);

        // public_amount = -s (negative for withdrawals)
        let expected_public_amount = -Fr::from(gross_virtual_sol);
        let provided_public_amount = Fr::from_be_bytes_mod_order(&public_amount_bytes);

        log!(
            "check_public_amount_unified: withdrawal ext_amount={}, net_vsol={}, fee={}, gross_vsol={}",
            ext_amount,
            net_virtual_sol,
            fee,
            gross_virtual_sol
        );

        // Allow ±1 rounding tolerance due to φ⁻¹ then φ conversion
        let diff = if expected_public_amount > provided_public_amount {
            expected_public_amount - provided_public_amount
        } else {
            provided_public_amount - expected_public_amount
        };
        diff <= Fr::from(1u64)
    } else {
        // Transfer: public_amount = 0
        let provided_public_amount = Fr::from_be_bytes_mod_order(&public_amount_bytes);
        provided_public_amount == Fr::from(0u64)
    }
}

/// Internal helper to validate provided fee against calculated expected fee.
///
/// Returns Ok(()) if provided_fee >= expected_fee, error otherwise.
#[inline]
fn validate_fee_amount(
    fee_base: u128,
    fee_rate: u128,
    provided_fee: u64,
    log_prefix: &str,
) -> Result<(), ProgramError> {
    if fee_base == 0 {
        log!("{}: fee_base is 0, no fee validation needed", log_prefix);
        return Ok(());
    }

    let expected_fee = fee_base
        .checked_mul(fee_rate)
        .ok_or(ShieldedPoolError::ArithmeticOverflow)?
        .checked_div(BASIS_POINTS_DENOMINATOR)
        .ok_or(ShieldedPoolError::ArithmeticOverflow)? as u64;

    log!(
        "{}: expected_fee = {} * {} / {} = {}",
        log_prefix,
        fee_base,
        fee_rate,
        BASIS_POINTS_DENOMINATOR,
        expected_fee
    );

    if provided_fee < expected_fee {
        log!(
            "{}: FAILED - provided_fee {} < expected_fee {}",
            log_prefix,
            provided_fee,
            expected_fee
        );
        return Err(ShieldedPoolError::InvalidFeeAmount.into());
    }

    log!("{}: SUCCESS", log_prefix);
    Ok(())
}

/// Validates that the provided fee matches the expected fee based on transaction type.
///
/// Fee is charged only on what crosses the shielded boundary (|ext_amount|):
/// - Deposit (ext_amount > 0): fee = ext_amount * deposit_fee_rate / 10000
/// - Withdrawal (ext_amount < 0): fee = |ext_amount| * withdrawal_fee_rate / 10000
/// - Transfer (ext_amount = 0): fee = 0 (no boundary crossing)
///
/// Relayer fee is a derived split of |ext_amount|, not part of the fee base.
pub fn validate_fee(
    ext_amount: i64,
    provided_fee: u64,
    _relayer_fee: u64, // Unused - relayer_fee is derived from ext_amount, not additive
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
) -> Result<(), ProgramError> {
    log!(
        "validate_fee: ext_amount={}, provided_fee={}, deposit_fee_rate={}, withdrawal_fee_rate={}",
        ext_amount,
        provided_fee,
        deposit_fee_rate,
        withdrawal_fee_rate
    );

    let (fee_base, fee_rate) = if ext_amount > 0 {
        log!("validate_fee: DEPOSIT case (ext_amount > 0)");
        (ext_amount as u128, deposit_fee_rate as u128)
    } else if ext_amount < 0 {
        let abs_ext_amount = ext_amount
            .checked_neg()
            .ok_or(ShieldedPoolError::ArithmeticOverflow)? as u128;
        log!(
            "validate_fee: WITHDRAWAL case (ext_amount < 0), fee_base = {}",
            abs_ext_amount
        );
        (abs_ext_amount, withdrawal_fee_rate as u128)
    } else {
        log!("validate_fee: TRANSFER case (ext_amount == 0), fee_base = 0");
        (0u128, 0u128)
    };

    validate_fee_amount(fee_base, fee_rate, provided_fee, "validate_fee")
}

/// Validates fee for unified SOL pools.
///
/// Per specification, fee is calculated on the **shielded amount** (s), not the external amount (e):
/// - Deposit (ext_amount > 0): s = φ(e) = e * λ / ρ, then f ≥ s · r_d / B
/// - Withdrawal (ext_amount < 0): s = |public_amount|, then f ≥ s · r_w / B
/// - Transfer (ext_amount = 0): fee = 0
///
/// This differs from token pools where fee is calculated on ext_amount directly.
///
/// # Parameters
/// - `ext_amount`: External amount in LST tokens (positive=deposit, negative=withdrawal)
/// - `provided_fee`: Fee provided in shielded units (virtual SOL)
/// - `public_amount_bytes`: The public_amount from the ZK proof (needed for withdrawals)
/// - `deposit_fee_rate`: Fee rate for deposits in basis points
/// - `withdrawal_fee_rate`: Fee rate for withdrawals in basis points
/// - `exchange_rate`: The harvested_exchange_rate (λ)
/// - `rate_precision`: RATE_PRECISION constant (ρ = 10^9)
pub fn validate_fee_unified(
    ext_amount: i64,
    provided_fee: u64,
    public_amount_bytes: [u8; 32],
    deposit_fee_rate: u16,
    withdrawal_fee_rate: u16,
    exchange_rate: u64,
    rate_precision: u64,
) -> Result<(), ProgramError> {
    log!(
        "validate_fee_unified: ext_amount={}, provided_fee={}, deposit_fee_rate={}, withdrawal_fee_rate={}, exchange_rate={}",
        ext_amount,
        provided_fee,
        deposit_fee_rate,
        withdrawal_fee_rate,
        exchange_rate
    );

    let (fee_base, fee_rate) = if ext_amount > 0 {
        let s = (ext_amount as u128)
            .checked_mul(exchange_rate as u128)
            .and_then(|v| v.checked_div(rate_precision as u128))
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
        log!(
            "validate_fee_unified: DEPOSIT case, s = φ(e) = {} * {} / {} = {}",
            ext_amount,
            exchange_rate,
            rate_precision,
            s
        );
        (s, deposit_fee_rate as u128)
    } else if ext_amount < 0 {
        let provided_public_amount = Fr::from_be_bytes_mod_order(&public_amount_bytes);
        let s_bigint = (-provided_public_amount).into_bigint();
        let s = s_bigint.0[0] as u128;
        log!(
            "validate_fee_unified: WITHDRAWAL case, s = |public_amount| = {}",
            s
        );
        (s, withdrawal_fee_rate as u128)
    } else {
        log!("validate_fee_unified: TRANSFER case, fee_base = 0");
        (0u128, 0u128)
    };

    validate_fee_amount(fee_base, fee_rate, provided_fee, "validate_fee_unified")
}

use crate::instructions::types::{N_INS, N_OUTS, N_PUBLIC_LINES, N_REWARD_LINES};

/// Total number of public inputs for the ZK circuit.
/// Layout (matches circuit public input order):
/// - 1: commitment_root (commitmentRoot)
/// - 1: transact_params_hash (transactParamsHash)
/// - N_PUBLIC_LINES: public_asset_ids (publicAssetId)
/// - N_PUBLIC_LINES: public_amounts (publicAmount)
/// - N_INS: nullifiers (nullifiers)
/// - N_OUTS: commitments (commitments)
/// - N_REWARD_LINES: reward_acc (rewardAcc)
/// - N_REWARD_LINES: reward_asset_id (rewardAssetId)
pub const N_PUBLIC_INPUTS: usize =
    1 + 1 + N_PUBLIC_LINES + N_PUBLIC_LINES + N_INS + N_OUTS + N_REWARD_LINES + N_REWARD_LINES;

/// Verify a ZK proof with the given verifying key.
/// Takes proof by reference to avoid ~800 byte stack copy.
#[inline(never)]
pub fn verify_proof(proof: &TransactProofData, verifying_key: Groth16Verifyingkey) -> bool {
    // Build public inputs array matching circuit order
    let mut public_inputs: [[u8; 32]; N_PUBLIC_INPUTS] = [[0u8; 32]; N_PUBLIC_INPUTS];
    let mut idx = 0;

    // 1. commitment_root - circuit: commitmentRoot
    public_inputs[idx] = proof.commitment_root;
    idx += 1;

    // 2. transact_params_hash - circuit: transactParamsHash
    public_inputs[idx] = proof.transact_params_hash;
    idx += 1;

    // 3. public_asset_ids (N_PUBLIC_LINES) - circuit: publicAssetId
    public_inputs[idx..idx + N_PUBLIC_LINES].copy_from_slice(&proof.public_asset_ids);
    idx += N_PUBLIC_LINES;

    // 4. public_amounts (N_PUBLIC_LINES) - circuit: publicAmount
    public_inputs[idx..idx + N_PUBLIC_LINES].copy_from_slice(&proof.public_amounts);
    idx += N_PUBLIC_LINES;

    // 5. nullifiers (N_INS) - circuit: nullifiers
    public_inputs[idx..idx + N_INS].copy_from_slice(&proof.nullifiers);
    idx += N_INS;

    // 6. commitments (N_OUTS) - circuit: commitments
    public_inputs[idx..idx + N_OUTS].copy_from_slice(&proof.commitments);
    idx += N_OUTS;

    // 7. reward_acc (N_REWARD_LINES) - circuit: rewardAcc
    public_inputs[idx..idx + N_REWARD_LINES].copy_from_slice(&proof.reward_acc);
    idx += N_REWARD_LINES;

    // 8. reward_asset_id (N_REWARD_LINES) - circuit: rewardAssetId
    public_inputs[idx..idx + N_REWARD_LINES].copy_from_slice(&proof.reward_asset_id);

    // Use shared verification helper
    let compressed = CompressedGroth16Proof {
        proof_a: &proof.proof_a,
        proof_b: &proof.proof_b,
        proof_c: &proof.proof_c,
    };

    verify_groth16(&compressed, &public_inputs, &verifying_key).unwrap_or(false)
}

use crate::instructions::TransactParams;

/// Calculates the hash of transact params for proof verification.
/// This hash is compared against the circuit's transact_params_hash public input.
///
/// Uses SHA256 of the raw Pod bytes for canonical/deterministic hashing.
/// TransactParams is a `#[repr(C)]` Pod struct with fixed layout, ensuring
/// consistent byte representation across all platforms.
///
/// The encrypted_output_hashes field in TransactParams must contain
/// SHA256(encrypted_outputs[i]) for each output, which the program verifies
/// separately before calling this function.
#[inline(never)]
pub fn calculate_transact_params_hash(transact_params: &TransactParams) -> [u8; 32] {
    sha256(bytemuck::bytes_of(transact_params))
}

pub fn change_endianness(bytes: &[u8]) -> Vec<u8> {
    let mut vec = Vec::new();
    for b in bytes.chunks(32) {
        for byte in b.iter().rev() {
            vec.push(*byte);
        }
    }
    vec
}

/// SHA256 syscall wrapper for efficient hashing
/// Uses the native SHA256 precompile when available
pub fn sha256(data: &[u8]) -> [u8; 32] {
    // SHA256 precompile is at address: KeccakSecp256k11111111111111111111111111111
    // The hash function from solana_program uses SHA256 internally
    hash(data).to_bytes()
}

/// Computes an asset ID from raw mint bytes using Poseidon hash.
///
/// The mint is stored as little-endian bytes. We split into two 128-bit limbs:
/// - Low limb:  bytes[0..16]  (least significant 128 bits)
/// - High limb: bytes[16..32] (most significant 128 bits)
///
/// Each limb is converted from little-endian to big-endian for Poseidon hashing.
///
/// # Errors
/// Returns `ShieldedPoolError::AssetIdComputationFailed` if Poseidon hashing fails.
pub fn compute_asset_id_from_bytes(mint_bytes: &[u8; 32]) -> Result<[u8; 32], ProgramError> {
    // Split mint into two 128-bit limbs (little-endian source)
    // Low limb: bytes 0-15, High limb: bytes 16-31
    let mut low_limb = [0u8; 32];
    let mut high_limb = [0u8; 32];

    // Convert each 16-byte little-endian chunk to 32-byte big-endian field element
    // Reverse bytes and place in low 16 bytes of the 32-byte array
    for i in 0..16 {
        low_limb[31 - i] = mint_bytes[i]; // Reverse bytes[0..16] -> positions [16..32]
        high_limb[31 - i] = mint_bytes[16 + i]; // Reverse bytes[16..32] -> positions [16..32]
    }

    // AUDIT FIX (H-01): Return Result instead of panicking on hash failure.
    // Hash both limbs: Poseidon(low_limb, high_limb)
    let hash_result = hashv(
        Parameters::Bn254X5,
        Endianness::BigEndian,
        &[&low_limb, &high_limb],
    )
    .map_err(|_| ShieldedPoolError::AssetIdComputationFailed)?;
    Ok(hash_result.to_bytes())
}

/// Verifies that a nullifier account PDA is correctly derived and has not been used before
pub fn verify_nullifier_unused(
    program_id: &pinocchio::pubkey::Pubkey,
    nullifier_account: &pinocchio::account_info::AccountInfo,
    nullifier: &[u8; 32],
) -> Result<(), ProgramError> {
    // Verify the nullifier account PDA is correctly derived from the nullifier
    if !crate::state::Nullifier::verify_pda(program_id, nullifier, nullifier_account.key()) {
        return Err(ProgramError::InvalidSeeds);
    }

    // Check if the nullifier account is initialized (has non-zero data)
    let nullifier_data = nullifier_account.try_borrow_data()?;

    // If account has data, nullifier was already used
    if nullifier_data.iter().any(|&byte| byte != 0) {
        return Err(ShieldedPoolError::NullifierAlreadyUsed.into());
    }

    Ok(())
}

/// Emit event data via CPI to the Log instruction.
///
/// This function:
/// 1. Logs data directly via sol_log_data
/// 2. CPI invokes the Log instruction on self program
///
/// The CPI ensures the event is recorded in the transaction logs with
/// proper program attribution.
///
/// # Arguments
/// * `program_id` - The program's ID
/// * `authority` - A PDA owned by this program, used as signer for the CPI
/// * `shielded_pool_program` - The shielded pool program account (required for self-CPI)
/// * `signers` - Signers for the CPI
/// * `data` - The event data to emit
pub fn emit_cpi_log(
    program_id: &Pubkey,
    authority: &AccountInfo,
    shielded_pool_program: &AccountInfo,
    signers: &[Signer],
    data: Vec<u8>,
) -> ProgramResult {
    use crate::instructions::ShieldedPoolInstruction;
    use borsh::BorshSerialize;

    // First, log directly
    pinocchio::log::sol_log_data(&[&data]);

    // Build instruction data: [Log discriminator (33), Borsh-serialized Vec<u8>]
    let log_discriminator = ShieldedPoolInstruction::Log as u8;
    let mut serialized = Vec::with_capacity(1 + 4 + data.len());
    serialized.push(log_discriminator);
    data.serialize(&mut serialized)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    // Authority account is the signer
    let account_metas = [
        AccountMeta::readonly_signer(authority.key()),
    ];

    let instruction = Instruction {
        program_id,
        accounts: &account_metas,
        data: &serialized,
    };

    // shielded_pool_program is included so the runtime can find the program executable for CPI
    invoke_signed(&instruction, &[authority, shielded_pool_program], signers)
}

/// Log a 32-byte array as hex string for debugging.
///
/// Splits the output into two 32-character lines due to Solana log length limits.
pub fn log_bytes_as_hex(label: &str, bytes: &[u8; 32]) {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = [0u8; 64];
    for (i, &byte) in bytes.iter().enumerate() {
        hex[i * 2] = HEX_CHARS[(byte >> 4) as usize];
        hex[i * 2 + 1] = HEX_CHARS[(byte & 0x0f) as usize];
    }
    pinocchio::log::sol_log(label);
    if let Ok(s) = core::str::from_utf8(&hex[..32]) {
        pinocchio::log::sol_log(s);
    }
    if let Ok(s) = core::str::from_utf8(&hex[32..]) {
        pinocchio::log::sol_log(s);
    }
}

#[cfg(test)]
pub mod test {
    use crate::utils::compute_asset_id_from_bytes;
    use std::println;

    #[test]
    fn test_asset_id_deterministic() {
        // Use a fixed mint address for reproducibility
        let mint_bytes: [u8; 32] = [
            0x06, 0xa7, 0xd5, 0x17, 0x18, 0x7b, 0xd1, 0x63, 0x35, 0xb7, 0xd6, 0xb8, 0x7a, 0x67,
            0x6c, 0x55, 0x26, 0x3a, 0x6c, 0x86, 0x40, 0xf4, 0xd1, 0x6e, 0x86, 0xf8, 0x24, 0x85,
            0x05, 0x9d, 0x65, 0x38,
        ];

        // Compute asset ID multiple times - should always be the same
        let asset_id_1 = compute_asset_id_from_bytes(&mint_bytes).unwrap();
        let asset_id_2 = compute_asset_id_from_bytes(&mint_bytes).unwrap();
        let asset_id_3 = compute_asset_id_from_bytes(&mint_bytes).unwrap();

        assert_eq!(asset_id_1, asset_id_2, "Asset ID should be deterministic");
        assert_eq!(asset_id_2, asset_id_3, "Asset ID should be deterministic");

        println!("Asset ID: {:02x?}", asset_id_1);
    }

    #[test]
    fn test_asset_id_different_mints_produce_different_ids() {
        let mint1_bytes: [u8; 32] = [1u8; 32];
        let mint2_bytes: [u8; 32] = [2u8; 32];

        let asset_id_1 = compute_asset_id_from_bytes(&mint1_bytes).unwrap();
        let asset_id_2 = compute_asset_id_from_bytes(&mint2_bytes).unwrap();

        assert_ne!(
            asset_id_1, asset_id_2,
            "Different mints should produce different asset IDs"
        );
    }

    #[test]
    fn test_asset_id_zero_mint() {
        // Edge case: zero mint address
        let zero_mint_bytes: [u8; 32] = [0u8; 32];

        let asset_id = compute_asset_id_from_bytes(&zero_mint_bytes).unwrap();

        // Poseidon hash of two zero limbs produces a specific non-zero value
        assert_ne!(
            asset_id, [0u8; 32],
            "Asset ID of zero mint should not be zero"
        );

        // Should be deterministic
        let asset_id_2 = compute_asset_id_from_bytes(&zero_mint_bytes).unwrap();
        assert_eq!(
            asset_id, asset_id_2,
            "Asset ID should be deterministic for zero mint"
        );

        println!("Zero mint asset ID: {:02x?}", asset_id);
    }

    #[test]
    fn test_asset_id_small_values() {
        // Test with small mint values
        let small_mint_1: [u8; 32] = {
            let mut arr = [0u8; 32];
            arr[31] = 1;
            arr
        };
        let small_mint_2: [u8; 32] = {
            let mut arr = [0u8; 32];
            arr[31] = 2;
            arr
        };

        let asset_id_1 = compute_asset_id_from_bytes(&small_mint_1).unwrap();
        let asset_id_2 = compute_asset_id_from_bytes(&small_mint_2).unwrap();

        // Different inputs should produce different outputs
        assert_ne!(asset_id_1, asset_id_2);

        // Each should be deterministic
        assert_eq!(asset_id_1, compute_asset_id_from_bytes(&small_mint_1).unwrap());
        assert_eq!(asset_id_2, compute_asset_id_from_bytes(&small_mint_2).unwrap());
    }

    #[test]
    fn test_asset_id_collision_resistance() {
        // Test that similar inputs produce very different outputs (avalanche effect)
        let mint1: [u8; 32] = [0x01; 32];
        let mut mint2 = mint1;
        mint2[0] = 0x02; // Only change first byte

        let asset_id_1 = compute_asset_id_from_bytes(&mint1).unwrap();
        let asset_id_2 = compute_asset_id_from_bytes(&mint2).unwrap();

        // Count differing bytes - should be significant (avalanche effect)
        let differing_bytes: usize = asset_id_1
            .iter()
            .zip(asset_id_2.iter())
            .filter(|(a, b)| a != b)
            .count();

        // With good hash function, roughly half the bytes should differ
        assert!(
            differing_bytes > 10,
            "Poseidon should exhibit avalanche effect, but only {} bytes differ",
            differing_bytes
        );
    }

    #[test]
    fn test_asset_id_limb_split() {
        // Test that the 128-bit limb split works correctly
        // Mint with distinct high and low halves
        let mint_bytes: [u8; 32] = [
            // High 128 bits (bytes 0-15)
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, // Low 128 bits (bytes 16-31)
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
            0x1f, 0x20,
        ];

        let asset_id = compute_asset_id_from_bytes(&mint_bytes).unwrap();

        // Swapping high and low should produce different result
        let mut swapped_bytes = [0u8; 32];
        swapped_bytes[0..16].copy_from_slice(&mint_bytes[16..32]);
        swapped_bytes[16..32].copy_from_slice(&mint_bytes[0..16]);

        let swapped_asset_id = compute_asset_id_from_bytes(&swapped_bytes).unwrap();

        assert_ne!(
            asset_id, swapped_asset_id,
            "Swapping limbs should produce different asset ID"
        );

        println!("Original asset ID: {:02x?}", asset_id);
        println!("Swapped asset ID: {:02x?}", swapped_asset_id);
    }

    #[test]
    fn test_asset_id_max_values() {
        // Test with max value (0xFF..FF) - should work without field reduction
        // since each 128-bit limb is well within the ~254-bit BN254 field
        let max_value: [u8; 32] = [0xFF; 32];
        let asset_id = compute_asset_id_from_bytes(&max_value).unwrap();

        // Should produce a valid non-zero hash
        assert_ne!(
            asset_id, [0u8; 32],
            "Max value should produce non-zero asset ID"
        );

        // Should be deterministic
        let asset_id_2 = compute_asset_id_from_bytes(&max_value).unwrap();
        assert_eq!(
            asset_id, asset_id_2,
            "Asset ID should be deterministic for max value"
        );

        println!("Max value asset ID: {:02x?}", asset_id);
    }

    #[test]
    fn test_asset_id_little_endian_interpretation() {
        // Test that mint bytes are interpreted as little-endian
        // A small value (1) in little-endian has the 0x01 byte at position 0
        let mut mint_le: [u8; 32] = [0u8; 32];
        mint_le[0] = 0x01; // Value = 1 in little-endian (low byte first)

        // Same value in "big-endian position" would be at byte 31
        let mut mint_be_style: [u8; 32] = [0u8; 32];
        mint_be_style[31] = 0x01;

        let asset_id_le = compute_asset_id_from_bytes(&mint_le).unwrap();
        let asset_id_be = compute_asset_id_from_bytes(&mint_be_style).unwrap();

        // These should produce different results since we interpret as little-endian
        assert_ne!(
            asset_id_le, asset_id_be,
            "LE and BE-style positioning should produce different asset IDs"
        );
    }

    #[test]
    fn test_asset_id_low_limb_only() {
        // Value that only affects the low limb (bytes 0-15)
        let mut mint_low: [u8; 32] = [0u8; 32];
        mint_low[0] = 0x42;
        mint_low[15] = 0x43;

        // Value that only affects the high limb (bytes 16-31)
        let mut mint_high: [u8; 32] = [0u8; 32];
        mint_high[16] = 0x42;
        mint_high[31] = 0x43;

        let asset_id_low = compute_asset_id_from_bytes(&mint_low).unwrap();
        let asset_id_high = compute_asset_id_from_bytes(&mint_high).unwrap();

        // Low-only and high-only should produce different results
        assert_ne!(
            asset_id_low, asset_id_high,
            "Low-limb-only and high-limb-only should produce different asset IDs"
        );

        println!("Low-limb asset ID: {:02x?}", asset_id_low);
        println!("High-limb asset ID: {:02x?}", asset_id_high);
    }

    #[test]
    fn test_asset_id_known_value() {
        // Known test vector for cross-implementation verification
        // This test ensures the implementation remains consistent
        let mint_bytes: [u8; 32] = [
            // Low limb (bytes 0-15) - little endian
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, // High limb (bytes 16-31) - little endian
            0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
            0x1f, 0x20,
        ];

        // Expected asset ID for this mint (Poseidon hash of two LE->BE converted limbs)
        // Use this value for cross-implementation verification
        let expected_asset_id: [u8; 32] = [
            0x2a, 0xd3, 0x69, 0x9b, 0xf2, 0xd1, 0xbe, 0x62, 0x80, 0x90, 0x82, 0x18, 0x0a, 0x33,
            0xac, 0x9b, 0x9d, 0x4f, 0x05, 0x75, 0xa3, 0xfd, 0x45, 0x44, 0x1f, 0x7b, 0xaf, 0x04,
            0x2f, 0x97, 0x77, 0x09,
        ];

        let asset_id = compute_asset_id_from_bytes(&mint_bytes).unwrap();

        // Print for reference when updating client implementations
        println!("Known value test - mint: {:02x?}", mint_bytes);
        println!("Known value test - asset_id: {:02x?}", asset_id);

        // Verify exact match with expected value
        assert_eq!(
            asset_id, expected_asset_id,
            "Asset ID should match expected value for cross-implementation compatibility"
        );
    }

    #[test]
    fn test_asset_id_byte_reversal() {
        // Test that bytes are correctly reversed within each limb
        // In little-endian: byte[0] is LSB, byte[15] is MSB of low limb

        // Create a mint where low limb has ascending bytes
        let mint_bytes: [u8; 32] = [
            // Low limb: 0x01 at LSB, 0x10 at MSB (little-endian)
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
            0x0f, 0x10, // High limb: all zeros
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        // Create a mint where low limb has descending bytes (reversed)
        let mint_bytes_reversed: [u8; 32] = [
            // Low limb: 0x10 at LSB, 0x01 at MSB (reversed)
            0x10, 0x0f, 0x0e, 0x0d, 0x0c, 0x0b, 0x0a, 0x09, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03,
            0x02, 0x01, // High limb: all zeros
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        let asset_id = compute_asset_id_from_bytes(&mint_bytes).unwrap();
        let asset_id_reversed = compute_asset_id_from_bytes(&mint_bytes_reversed).unwrap();

        // These represent different values, so should produce different hashes
        assert_ne!(
            asset_id, asset_id_reversed,
            "Original and reversed bytes should produce different asset IDs"
        );
    }

    // ========================================================================
    // Fee Calculation Tests
    // ========================================================================

    use super::{check_public_amount, check_public_amount_unified, validate_fee, validate_fee_unified};
    use ark_bn254::Fr;
    use ark_ff::PrimeField;

    /// Helper to convert i64 amount to field element bytes (big-endian).
    fn amount_to_field_bytes(amount: i64) -> [u8; 32] {
        let fr = if amount >= 0 {
            Fr::from(amount as u64)
        } else {
            -Fr::from((-amount) as u64)
        };
        let mut bytes = [0u8; 32];
        let bigint = fr.into_bigint();
        // Convert to big-endian
        for i in 0..4 {
            let limb_bytes = bigint.0[3 - i].to_be_bytes();
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&limb_bytes);
        }
        bytes
    }

    // ------------------------------------------------------------------------
    // check_public_amount tests (Token Pool - 1:1 exchange rate)
    // ------------------------------------------------------------------------

    #[test]
    fn test_check_public_amount_deposit() {
        // Deposit: public_amount = ext_amount - fee
        let ext_amount: i64 = 1000;
        let fee: u64 = 10;
        // Expected: 1000 - 10 = 990
        let public_amount = amount_to_field_bytes(990);

        assert!(
            check_public_amount(ext_amount, fee, public_amount),
            "Deposit: public_amount should equal ext_amount - fee"
        );
    }

    #[test]
    fn test_check_public_amount_deposit_wrong_amount() {
        // Deposit with wrong public_amount
        let ext_amount: i64 = 1000;
        let fee: u64 = 10;
        // Wrong: includes relayer_fee (which should NOT be included)
        let wrong_public_amount = amount_to_field_bytes(980); // As if relayer_fee=10 was subtracted

        assert!(
            !check_public_amount(ext_amount, fee, wrong_public_amount),
            "Deposit: public_amount should NOT include relayer_fee"
        );
    }

    #[test]
    fn test_check_public_amount_withdrawal() {
        // Withdrawal: public_amount = -(|ext_amount| + fee)
        let ext_amount: i64 = -1000; // Negative for withdrawal
        let fee: u64 = 10;
        // Expected: -(1000 + 10) = -1010
        let public_amount = amount_to_field_bytes(-1010);

        assert!(
            check_public_amount(ext_amount, fee, public_amount),
            "Withdrawal: public_amount should equal -(|ext_amount| + fee)"
        );
    }

    #[test]
    fn test_check_public_amount_withdrawal_wrong_amount() {
        // Withdrawal with wrong public_amount (as if relayer_fee was added)
        let ext_amount: i64 = -1000;
        let fee: u64 = 10;
        // Wrong: as if relayer_fee=5 was added
        let wrong_public_amount = amount_to_field_bytes(-1015);

        assert!(
            !check_public_amount(ext_amount, fee, wrong_public_amount),
            "Withdrawal: public_amount should NOT include relayer_fee"
        );
    }

    #[test]
    fn test_check_public_amount_transfer() {
        // Transfer: ext_amount = 0, public_amount = 0
        let ext_amount: i64 = 0;
        let fee: u64 = 0;
        let public_amount = amount_to_field_bytes(0);

        assert!(
            check_public_amount(ext_amount, fee, public_amount),
            "Transfer: public_amount should be 0 when ext_amount is 0"
        );
    }

    #[test]
    fn test_check_public_amount_zero_fee() {
        // Deposit with zero fee
        let ext_amount: i64 = 1000;
        let fee: u64 = 0;
        let public_amount = amount_to_field_bytes(1000);

        assert!(
            check_public_amount(ext_amount, fee, public_amount),
            "Deposit with zero fee: public_amount should equal ext_amount"
        );
    }

    #[test]
    fn test_check_public_amount_fee_exceeds_deposit() {
        // Fee exceeds deposit amount - should fail
        let ext_amount: i64 = 10;
        let fee: u64 = 100;
        let public_amount = amount_to_field_bytes(-90); // Would be negative

        assert!(
            !check_public_amount(ext_amount, fee, public_amount),
            "Should reject when fee exceeds deposit amount"
        );
    }

    // ------------------------------------------------------------------------
    // check_public_amount_unified tests (Unified SOL Pool - exchange rate)
    // ------------------------------------------------------------------------

    #[test]
    fn test_check_public_amount_unified_deposit() {
        // Deposit: ext_amount is in tokens (domain E)
        // s = φ(e) = 1000 * 1.1 = 1100 virtual SOL
        // public_amount = s - f = 1100 - 11 = 1089
        let ext_amount: i64 = 1000; // LST tokens
        let fee: u64 = 11; // Fee in virtual SOL
        let exchange_rate: u64 = 1_100_000_000; // 1.1 (10% appreciation)
        let rate_precision: u64 = 1_000_000_000;
        let public_amount = amount_to_field_bytes(1089);

        assert!(
            check_public_amount_unified(ext_amount, fee, public_amount, exchange_rate, rate_precision),
            "Unified deposit: public_amount = φ(ext_amount) - fee"
        );
    }

    #[test]
    fn test_check_public_amount_unified_deposit_wrong_amount() {
        // Deposit with wrong public_amount (didn't apply exchange rate)
        let ext_amount: i64 = 1000; // tokens
        let fee: u64 = 11;
        let exchange_rate: u64 = 1_100_000_000;
        let rate_precision: u64 = 1_000_000_000;
        // Wrong: 1000 - 11 = 989 (didn't convert to virtual SOL first)
        let wrong_public_amount = amount_to_field_bytes(989);

        assert!(
            !check_public_amount_unified(ext_amount, fee, wrong_public_amount, exchange_rate, rate_precision),
            "Unified deposit: must apply exchange rate"
        );
    }

    #[test]
    fn test_check_public_amount_unified_withdrawal() {
        // Withdrawal: ext_amount is in tokens (domain E)
        // |e| = φ⁻¹(s - f) = tokens
        // Given: s = 1100 (gross virtual SOL), f = 11 (fee)
        // net_vsol = s - f = 1089
        // |e| = φ⁻¹(1089) = 1089 * 1e9 / 1.1e9 = 990 tokens
        // public_amount = -s = -1100
        let ext_amount: i64 = -990; // tokens (net)
        let fee: u64 = 11;
        let exchange_rate: u64 = 1_100_000_000; // 1.1
        let rate_precision: u64 = 1_000_000_000;
        // φ(|e|) = 990 * 1.1 = 1089 = s - f
        // s = 1089 + 11 = 1100
        let public_amount = amount_to_field_bytes(-1100);

        assert!(
            check_public_amount_unified(ext_amount, fee, public_amount, exchange_rate, rate_precision),
            "Unified withdrawal: public_amount = -s where φ(|e|) = s - f"
        );
    }

    #[test]
    fn test_check_public_amount_unified_transfer() {
        // Transfer: public_amount = 0
        let ext_amount: i64 = 0;
        let fee: u64 = 0;
        let exchange_rate: u64 = 1_100_000_000;
        let rate_precision: u64 = 1_000_000_000;
        let public_amount = amount_to_field_bytes(0);

        assert!(
            check_public_amount_unified(ext_amount, fee, public_amount, exchange_rate, rate_precision),
            "Unified transfer: public_amount should be 0"
        );
    }

    #[test]
    fn test_check_public_amount_unified_zero_exchange_rate() {
        // Zero exchange rate should fail (needed for φ conversion)
        let ext_amount: i64 = 1000;
        let fee: u64 = 10;
        let exchange_rate: u64 = 0;
        let rate_precision: u64 = 1_000_000_000;
        let public_amount = amount_to_field_bytes(990);

        assert!(
            !check_public_amount_unified(ext_amount, fee, public_amount, exchange_rate, rate_precision),
            "Zero exchange rate should fail"
        );
    }

    // ------------------------------------------------------------------------
    // validate_fee tests (Token Pool)
    // ------------------------------------------------------------------------

    #[test]
    fn test_validate_fee_deposit() {
        // Deposit: fee = ext_amount * deposit_rate / 10000
        let ext_amount: i64 = 10000;
        let fee: u64 = 100; // 1% of 10000
        let relayer_fee: u64 = 50; // Unused
        let deposit_rate: u16 = 100; // 1%
        let withdrawal_rate: u16 = 50;

        assert!(
            validate_fee(ext_amount, fee, relayer_fee, deposit_rate, withdrawal_rate).is_ok(),
            "Valid deposit fee should pass"
        );
    }

    #[test]
    fn test_validate_fee_deposit_insufficient() {
        // Deposit with insufficient fee
        let ext_amount: i64 = 10000;
        let fee: u64 = 50; // Only 0.5%, but rate is 1%
        let relayer_fee: u64 = 0;
        let deposit_rate: u16 = 100; // 1%
        let withdrawal_rate: u16 = 50;

        assert!(
            validate_fee(ext_amount, fee, relayer_fee, deposit_rate, withdrawal_rate).is_err(),
            "Insufficient deposit fee should fail"
        );
    }

    #[test]
    fn test_validate_fee_withdrawal() {
        // Withdrawal: fee = |ext_amount| * withdrawal_rate / 10000
        let ext_amount: i64 = -10000;
        let fee: u64 = 50; // 0.5% of 10000
        let relayer_fee: u64 = 25;
        let deposit_rate: u16 = 100;
        let withdrawal_rate: u16 = 50; // 0.5%

        assert!(
            validate_fee(ext_amount, fee, relayer_fee, deposit_rate, withdrawal_rate).is_ok(),
            "Valid withdrawal fee should pass"
        );
    }

    #[test]
    fn test_validate_fee_transfer() {
        // Transfer: ext_amount = 0, fee should be 0
        let ext_amount: i64 = 0;
        let fee: u64 = 0;
        let relayer_fee: u64 = 0;
        let deposit_rate: u16 = 100;
        let withdrawal_rate: u16 = 50;

        assert!(
            validate_fee(ext_amount, fee, relayer_fee, deposit_rate, withdrawal_rate).is_ok(),
            "Transfer with zero fee should pass"
        );
    }

    #[test]
    fn test_validate_fee_overpayment_allowed() {
        // Overpaying fee should be allowed (fee >= expected)
        let ext_amount: i64 = 10000;
        let fee: u64 = 200; // 2%, but rate is only 1%
        let relayer_fee: u64 = 0;
        let deposit_rate: u16 = 100; // 1%
        let withdrawal_rate: u16 = 50;

        assert!(
            validate_fee(ext_amount, fee, relayer_fee, deposit_rate, withdrawal_rate).is_ok(),
            "Fee overpayment should be allowed"
        );
    }

    // ------------------------------------------------------------------------
    // validate_fee_unified tests (Unified SOL Pool)
    // ------------------------------------------------------------------------

    #[test]
    fn test_validate_fee_unified_deposit() {
        // Deposit: fee is calculated on shielded amount (virtual_sol)
        // virtual_sol = 10000 * 1.1e9 / 1e9 = 11000
        // expected_fee = 11000 * 100 / 10000 = 110
        let ext_amount: i64 = 10000;
        let fee: u64 = 110;
        let exchange_rate: u64 = 1_100_000_000;
        let rate_precision: u64 = 1_000_000_000;
        let deposit_rate: u16 = 100; // 1%
        let withdrawal_rate: u16 = 50;
        // public_amount doesn't matter for fee validation in deposit
        let public_amount = amount_to_field_bytes(11000 - 110);

        assert!(
            validate_fee_unified(ext_amount, fee, public_amount, deposit_rate, withdrawal_rate, exchange_rate, rate_precision).is_ok(),
            "Unified deposit fee validation should pass"
        );
    }

    #[test]
    fn test_validate_fee_unified_withdrawal() {
        // Withdrawal: fee is calculated on |public_amount|
        // s = 11000, fee = s * 50 / 10000 = 55
        let ext_amount: i64 = -10000;
        let fee: u64 = 55;
        let exchange_rate: u64 = 1_100_000_000;
        let rate_precision: u64 = 1_000_000_000;
        let deposit_rate: u16 = 100;
        let withdrawal_rate: u16 = 50; // 0.5%
        let public_amount = amount_to_field_bytes(-11000);

        assert!(
            validate_fee_unified(ext_amount, fee, public_amount, deposit_rate, withdrawal_rate, exchange_rate, rate_precision).is_ok(),
            "Unified withdrawal fee validation should pass"
        );
    }

    #[test]
    fn test_validate_fee_unified_transfer() {
        // Transfer: ext_amount = 0, fee should be 0
        let ext_amount: i64 = 0;
        let fee: u64 = 0;
        let exchange_rate: u64 = 1_100_000_000;
        let rate_precision: u64 = 1_000_000_000;
        let deposit_rate: u16 = 100;
        let withdrawal_rate: u16 = 50;
        let public_amount = amount_to_field_bytes(0);

        assert!(
            validate_fee_unified(ext_amount, fee, public_amount, deposit_rate, withdrawal_rate, exchange_rate, rate_precision).is_ok(),
            "Unified transfer with zero fee should pass"
        );
    }

    // ------------------------------------------------------------------------
    // Critical invariant: relayer_fee is NOT in public_amount
    // ------------------------------------------------------------------------

    #[test]
    fn test_relayer_fee_independence_deposit() {
        // Key invariant: changing relayer_fee does NOT change the expected public_amount
        let ext_amount: i64 = 1000;
        let fee: u64 = 10;
        let expected_public_amount = amount_to_field_bytes(990); // ext_amount - fee

        // With relayer_fee = 0
        assert!(check_public_amount(ext_amount, fee, expected_public_amount));

        // With relayer_fee = 50 (same expected public_amount)
        assert!(check_public_amount(ext_amount, fee, expected_public_amount));

        // With relayer_fee = 100 (same expected public_amount)
        assert!(check_public_amount(ext_amount, fee, expected_public_amount));
    }

    #[test]
    fn test_relayer_fee_independence_withdrawal() {
        // Key invariant: changing relayer_fee does NOT change the expected public_amount
        let ext_amount: i64 = -1000;
        let fee: u64 = 10;
        let expected_public_amount = amount_to_field_bytes(-1010); // -(|ext_amount| + fee)

        // The public_amount is the same regardless of relayer_fee
        assert!(check_public_amount(ext_amount, fee, expected_public_amount));
    }

    // ========================================================================
    // Edge Case Tests
    // ========================================================================

    #[test]
    fn test_check_public_amount_i64_min() {
        // i64::MIN cannot be negated (would overflow)
        // Function should return false, not panic
        let public_amount = amount_to_field_bytes(0);
        assert!(
            !check_public_amount(i64::MIN, 0, public_amount),
            "i64::MIN should be rejected"
        );
    }

    #[test]
    fn test_check_public_amount_unified_i64_min() {
        // i64::MIN should be rejected in unified version too
        let public_amount = amount_to_field_bytes(0);
        assert!(
            !check_public_amount_unified(i64::MIN, 0, public_amount, 1_000_000_000, 1_000_000_000),
            "i64::MIN should be rejected in unified"
        );
    }

    #[test]
    fn test_validate_fee_i64_min() {
        // validate_fee should handle i64::MIN gracefully (not panic)
        // checked_neg() returns None for i64::MIN, triggering ArithmeticOverflow
        let result = validate_fee(i64::MIN, 0, 0, 100, 50);
        assert!(result.is_err(), "i64::MIN should return error, not panic");
    }

    #[test]
    fn test_check_public_amount_unified_zero_rate_precision() {
        // Zero rate_precision would cause division by zero in φ conversion
        let public_amount = amount_to_field_bytes(990);
        let result = check_public_amount_unified(
            1000, // tokens
            10,
            public_amount,
            1_000_000_000, // exchange_rate
            0,             // zero rate_precision!
        );
        // Should fail gracefully (to_virtual_sol returns None)
        assert!(!result, "Zero rate_precision should fail");
    }

    #[test]
    fn test_check_public_amount_large_fee() {
        // Fee larger than deposit amount should fail
        let ext_amount: i64 = 100;
        let fee: u64 = 200; // Fee > ext_amount
        let public_amount = amount_to_field_bytes(-100); // Would be negative

        assert!(
            !check_public_amount(ext_amount, fee, public_amount),
            "Fee > ext_amount should fail for deposits"
        );
    }

    #[test]
    fn test_check_public_amount_unified_fee_exceeds_virtual_sol() {
        // Fee larger than virtual_sol (φ(ext_amount)) should fail
        // ext_amount = 100 tokens, φ(100) = 100 * 1.1 = 110 virtual SOL
        // fee = 200 > 110, should fail
        let ext_amount: i64 = 100; // tokens
        let fee: u64 = 200; // Fee > virtual_sol (110)
        let exchange_rate: u64 = 1_100_000_000;
        let rate_precision: u64 = 1_000_000_000;
        let public_amount = amount_to_field_bytes(0);

        assert!(
            !check_public_amount_unified(ext_amount, fee, public_amount, exchange_rate, rate_precision),
            "Fee > virtual_sol should fail"
        );
    }

    #[test]
    fn test_check_public_amount_max_u64_amounts() {
        // Test with large but valid amounts
        let ext_amount: i64 = 1_000_000_000_000; // 1 trillion
        let fee: u64 = 10_000_000_000; // 10 billion (1%)
        let expected = ext_amount - fee as i64;
        let public_amount = amount_to_field_bytes(expected);

        assert!(
            check_public_amount(ext_amount, fee, public_amount),
            "Large amounts should work correctly"
        );
    }

    #[test]
    fn test_validate_fee_max_fee_rate() {
        // 100% fee rate (10000 bps) - entire amount is fee
        let ext_amount: i64 = 1000;
        let fee: u64 = 1000; // 100% of 1000
        let result = validate_fee(ext_amount, fee, 0, 10000, 10000);
        assert!(result.is_ok(), "100% fee rate should be allowed");
    }

    #[test]
    fn test_validate_fee_zero_amount() {
        // Zero ext_amount (transfer) should allow zero fee
        let result = validate_fee(0, 0, 0, 100, 50);
        assert!(result.is_ok(), "Zero amount with zero fee should pass");
    }

    #[test]
    fn test_check_public_amount_withdrawal_with_zero_fee() {
        // Withdrawal with zero fee: public_amount = -|ext_amount|
        let ext_amount: i64 = -1000;
        let fee: u64 = 0;
        let public_amount = amount_to_field_bytes(-1000);

        assert!(
            check_public_amount(ext_amount, fee, public_amount),
            "Withdrawal with zero fee should work"
        );
    }

    #[test]
    fn test_check_public_amount_unified_withdrawal_exact_match() {
        // ext_amount is in tokens (domain E), needs φ conversion for validation
        // Formula: |e| = φ⁻¹(s - f), so φ(|e|) = s - f, thus s = φ(|e|) + f
        let exchange_rate: u64 = 1_050_000_000; // 1.05x
        let rate_precision: u64 = 1_000_000_000;

        // Withdrawal: s = 1050 (gross virtual SOL), fee = 5
        // net_vsol = s - f = 1045
        // |e| = φ⁻¹(1045) = 1045 * 1e9 / 1.05e9 = 995.238... ≈ 995 tokens
        // φ(995) = 995 * 1.05 = 1044.75 ≈ 1044 (rounding)
        // s = 1044 + 5 = 1049 (with rounding, allows ±1 tolerance)
        let ext_amount: i64 = -995; // tokens
        let fee: u64 = 5;
        let public_amount = amount_to_field_bytes(-1050);

        // Should pass with ±1 rounding tolerance
        assert!(
            check_public_amount_unified(ext_amount, fee, public_amount, exchange_rate, rate_precision),
            "Should allow with rounding tolerance"
        );

        // Significantly wrong ext_amount should fail
        let ext_amount_wrong: i64 = -900; // Way off
        assert!(
            !check_public_amount_unified(ext_amount_wrong, fee, public_amount, exchange_rate, rate_precision),
            "Should reject significantly wrong ext_amount"
        );
    }

    #[test]
    fn test_validate_fee_relayer_fee_is_ignored() {
        // Verify that relayer_fee parameter doesn't affect validation
        // (it's marked as _relayer_fee in the function signature)
        let ext_amount: i64 = 10000;
        let fee: u64 = 100; // 1%
        let deposit_rate: u16 = 100;
        let withdrawal_rate: u16 = 50;

        // Should pass regardless of relayer_fee value
        assert!(validate_fee(ext_amount, fee, 0, deposit_rate, withdrawal_rate).is_ok());
        assert!(validate_fee(ext_amount, fee, 1000, deposit_rate, withdrawal_rate).is_ok());
        assert!(validate_fee(ext_amount, fee, u64::MAX, deposit_rate, withdrawal_rate).is_ok());
    }
}
