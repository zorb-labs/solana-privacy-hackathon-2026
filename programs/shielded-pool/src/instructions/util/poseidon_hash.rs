use pinocchio::{
    ProgramResult, account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey,
};
use pinocchio_log::log;
use solana_poseidon::{Endianness, Parameters, PoseidonHash, hashv};

/// Compute a Poseidon hash (utility instruction).
///
/// This is a utility instruction for computing Poseidon hashes on-chain.
/// Useful for testing and verifying hash computations client-side.
///
/// # Accounts
///
/// No accounts required.
///
/// # Arguments
///
/// * `input` - 32-byte input to hash
///
/// # Returns
///
/// Logs the first 4 bytes of the Poseidon hash output.
///
/// # Notes
///
/// - Uses BN254 curve parameters with big-endian encoding
/// - This is primarily a utility for development and testing
pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], input: [u8; 32]) -> ProgramResult {
    log!("Poseidon hash instruction called");

    // Test 1-input Poseidon
    let result1: PoseidonHash = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&input])
        .map_err(|e| {
            log!("1-input Poseidon failed");
            ProgramError::InvalidArgument
        })?;
    log!("1-input Poseidon: OK");

    // Test 2-input Poseidon
    let zero = [0u8; 32];
    let result2: PoseidonHash = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&input, &zero])
        .map_err(|e| {
            log!("2-input Poseidon failed");
            ProgramError::InvalidArgument
        })?;
    log!("2-input Poseidon: OK");

    // Test 3-input Poseidon (like indexed leaf hash)
    let result3: PoseidonHash = hashv(Parameters::Bn254X5, Endianness::BigEndian, &[&input, &zero, &zero])
        .map_err(|e| {
            log!("3-input Poseidon failed");
            ProgramError::InvalidArgument
        })?;
    log!("3-input Poseidon: OK");

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
