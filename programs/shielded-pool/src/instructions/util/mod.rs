//! Utility instructions.
//!
//! This module contains helper instructions for logging, hashing, and testing.

use alloc::vec::Vec;
use bytemuck::{Pod, Zeroable};
use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, program_error::ProgramError};
use pinocchio_log::log;
use solana_poseidon::{Endianness, Parameters, PoseidonHash as SolanaPoseidonHash, hashv};

use crate::errors::ShieldedPoolError;
use crate::groth16::{CompressedGroth16Proof, verify_groth16};
use crate::verifying_keys::{N_PUBLIC_INPUTS_BATCH_4, NULLIFIER_BATCH_VK_4};

// ============================================================================
// Data Structs
// ============================================================================

/// Data for PoseidonHash instruction.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct PoseidonHashData {
    /// 32-byte input to hash
    pub input: [u8; 32],
}

/// Size of the TestGroth16Data struct in bytes
pub const TEST_GROTH16_DATA_SIZE: usize = core::mem::size_of::<TestGroth16Data>();

/// Data for TestGroth16 instruction.
///
/// Uses the nullifier batch insertion circuit (batch size 4) with 7 public inputs:
/// [old_root, new_root, nullifier0, nullifier1, nullifier2, nullifier3, starting_index]
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct TestGroth16Data {
    /// Compressed G1 point for proof element A (32 bytes, big-endian)
    pub proof_a: [u8; 32],
    /// Compressed G2 point for proof element B (64 bytes, big-endian)
    pub proof_b: [u8; 64],
    /// Compressed G1 point for proof element C (32 bytes, big-endian)
    pub proof_c: [u8; 32],
    /// Public inputs for the circuit (7 x 32-byte field elements, big-endian)
    pub public_inputs: [[u8; 32]; N_PUBLIC_INPUTS_BATCH_4],
}

// Manual Borsh implementation for TestGroth16Data (Pod struct - just copy bytes)
impl borsh::BorshSerialize for TestGroth16Data {
    fn serialize<W: borsh::io::Write>(&self, writer: &mut W) -> borsh::io::Result<()> {
        writer.write_all(bytemuck::bytes_of(self))
    }
}

impl borsh::BorshDeserialize for TestGroth16Data {
    fn deserialize_reader<R: borsh::io::Read>(reader: &mut R) -> borsh::io::Result<Self> {
        let mut bytes = [0u8; TEST_GROTH16_DATA_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(*bytemuck::from_bytes(&bytes))
    }
}

// ============================================================================
// Accounts Structs
// ============================================================================

/// Accounts for PoseidonHash instruction.
/// This is a utility instruction with no required accounts.
/// The instruction reads input from data, not accounts.
#[derive(Accounts)]
pub struct PoseidonHashAccounts<'info> {
    /// Placeholder account (poseidon_hash needs no accounts, this is for type safety)
    /// In practice, callers can pass any account here.
    pub placeholder: &'info AccountInfo,
}

/// Accounts for TestGroth16 instruction.
/// This is a utility instruction for testing Groth16 proof verification.
/// No accounts are required; all data comes from instruction data.
#[derive(Accounts)]
pub struct TestGroth16Accounts<'info> {
    /// Placeholder account (test_groth16 needs no accounts, this is for type safety)
    /// In practice, callers can pass any account here.
    pub placeholder: &'info AccountInfo,
}

/// Accounts for Log instruction.
#[derive(Accounts)]
pub struct LogAccounts<'info> {
    /// Authority PDA that must be owned by this program
    pub authority: &'info AccountInfo,
}

// ============================================================================
// Handlers
// ============================================================================

/// Compute a Poseidon hash (utility instruction).
///
/// This is a utility instruction for computing Poseidon hashes on-chain.
/// Useful for testing and verifying hash computations client-side.
///
/// Uses BN254 curve parameters with big-endian encoding.
/// Tests 1, 2, and 3 input variants using both solana_poseidon and light_hasher.
pub fn process_poseidon_hash(
    _ctx: Context<PoseidonHashAccounts>,
    data: PoseidonHashData,
) -> ProgramResult {
    use light_hasher::{Hasher, Poseidon as LightPoseidon};

    log!("Poseidon hash instruction called");

    // Test using solana_poseidon (official crate)
    let zero = [0u8; 32];

    // Test 1-input solana_poseidon
    let result1: SolanaPoseidonHash =
        hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&data.input]).map_err(|_e| {
            log!("solana_poseidon 1-input failed");
            ProgramError::InvalidArgument
        })?;
    log!("solana_poseidon 1-input: OK");

    // Test 2-input solana_poseidon
    let _result2: SolanaPoseidonHash = hashv(
        Parameters::Bn254X5,
        Endianness::BigEndian,
        &[&data.input, &zero],
    )
    .map_err(|_e| {
        log!("solana_poseidon 2-input failed");
        ProgramError::InvalidArgument
    })?;
    log!("solana_poseidon 2-input: OK");

    // Test 3-input solana_poseidon
    let _result3: SolanaPoseidonHash = hashv(
        Parameters::Bn254X5,
        Endianness::BigEndian,
        &[&data.input, &zero, &zero],
    )
    .map_err(|_e| {
        log!("solana_poseidon 3-input failed");
        ProgramError::InvalidArgument
    })?;
    log!("solana_poseidon 3-input: OK");

    // Now test using light_hasher (used by indexed merkle tree)
    // Test 1-input light_hasher
    let _lh1 = LightPoseidon::hashv(&[&data.input]).map_err(|_e| {
        log!("light_hasher 1-input failed");
        ProgramError::InvalidArgument
    })?;
    log!("light_hasher 1-input: OK");

    // Test 2-input light_hasher
    let _lh2 = LightPoseidon::hashv(&[&data.input, &zero]).map_err(|_e| {
        log!("light_hasher 2-input failed");
        ProgramError::InvalidArgument
    })?;
    log!("light_hasher 2-input: OK");

    // Test 3-input light_hasher
    let _lh3 = LightPoseidon::hashv(&[&data.input, &zero, &zero]).map_err(|_e| {
        log!("light_hasher 3-input failed");
        ProgramError::InvalidArgument
    })?;
    log!("light_hasher 3-input: OK");

    // Test the exact code path from IndexedMerkleTree::compute_leaf_hash
    // This mimics what happens during initialize
    use crate::state::IndexedLeaf;
    let genesis_leaf = IndexedLeaf::genesis();

    // Convert next_index to 32 bytes (little-endian padded) - exact copy from indexed_merkle_tree.rs
    let mut next_index_bytes = [0u8; 32];
    next_index_bytes[..8].copy_from_slice(&genesis_leaf.next_index.to_le_bytes());

    log!("Testing genesis leaf hash computation...");
    log!(
        "value[0..4]: {} {} {} {}",
        genesis_leaf.value[0],
        genesis_leaf.value[1],
        genesis_leaf.value[2],
        genesis_leaf.value[3]
    );
    log!("next_index: {}", genesis_leaf.next_index);
    log!(
        "next_value[0..4]: {} {} {} {}",
        genesis_leaf.next_value[0],
        genesis_leaf.next_value[1],
        genesis_leaf.next_value[2],
        genesis_leaf.next_value[3]
    );

    // 3-input Poseidon: (value, next_index, next_value) - exact copy from indexed_merkle_tree.rs
    let genesis_hash = LightPoseidon::hashv(&[
        &genesis_leaf.value,
        &next_index_bytes,
        &genesis_leaf.next_value,
    ])
    .map_err(|_e| {
        log!("genesis leaf hash FAILED");
        ProgramError::InvalidArgument
    })?;
    log!("genesis leaf hash: OK");
    log!(
        "hash[0..4]: {} {} {} {}",
        genesis_hash[0],
        genesis_hash[1],
        genesis_hash[2],
        genesis_hash[3]
    );

    let output = result1.to_bytes();
    log!(
        "Poseidon output[0..4]: {} {} {} {}",
        output[0],
        output[1],
        output[2],
        output[3]
    );

    Ok(())
}

/// Test Groth16 proof verification (utility instruction).
///
/// This instruction verifies a Groth16 proof against the nullifier batch insertion
/// circuit (batch size 4) verification key. It's useful for testing that the
/// Groth16 verifier is working correctly on-chain.
///
/// # Arguments
/// * `data` - The proof elements and public inputs
///
/// # Returns
/// * `Ok(())` if the proof is valid
/// * `Err(InvalidProof)` if the proof verification fails
pub fn process_test_groth16(
    _ctx: Context<TestGroth16Accounts>,
    data: TestGroth16Data,
) -> ProgramResult {
    log!("TestGroth16 instruction called");

    let compressed = CompressedGroth16Proof {
        proof_a: &data.proof_a,
        proof_b: &data.proof_b,
        proof_c: &data.proof_c,
    };

    let result = verify_groth16::<N_PUBLIC_INPUTS_BATCH_4>(
        &compressed,
        &data.public_inputs,
        &NULLIFIER_BATCH_VK_4,
    );

    match result {
        Ok(true) => {
            log!("Groth16 proof verification succeeded");
            Ok(())
        }
        Ok(false) => {
            log!("Groth16 proof verification returned false");
            Err(ShieldedPoolError::InvalidProof.into())
        }
        Err(e) => {
            log!("Groth16 proof verification error: {}", e as u32);
            Err(ShieldedPoolError::InvalidProof.into())
        }
    }
}

/// Log event data via CPI self-invocation.
///
/// This instruction calls sol_log_data with the provided data.
/// Access is restricted to accounts owned by this program.
pub fn process_log(ctx: Context<LogAccounts>, data: &[u8]) -> ProgramResult {
    use borsh::BorshDeserialize;

    #[derive(BorshDeserialize)]
    struct LogData {
        data: Vec<u8>,
    }

    let args = LogData::try_from_slice(data).map_err(|_| ProgramError::InvalidInstructionData)?;

    let LogAccounts { authority } = ctx.accounts;

    // Authority must be a signer (PDA signed via invoke_signed)
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Authority must be owned by this program
    if authority.owner() != &crate::ID {
        return Err(ProgramError::IllegalOwner);
    }

    pinocchio::log::sol_log_data(&[&args.data]);
    Ok(())
}
