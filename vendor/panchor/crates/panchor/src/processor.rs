//! Generic instruction processor for Solana programs
//!
//! Provides a generic `process_instruction` function that works with any
//! instruction enum that implements `InstructionDispatch` and `TryFrom<u8>`.

use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{Pubkey, pubkey_eq},
};
use pinocchio_log::log;

use crate::InstructionDispatch;

/// Process an instruction using a generic instruction type.
///
/// This function:
/// 1. Verifies the program ID matches
/// 2. Parses the instruction discriminator (first byte)
/// 3. Converts the discriminator to the instruction enum
/// 4. Logs the instruction name (unless it returns empty string)
/// 5. Dispatches to the appropriate handler
///
/// # Type Parameters
///
/// * `T` - The instruction enum type that must implement:
///   - `InstructionDispatch` - For dispatching to handlers
///   - `TryFrom<u8>` - For parsing the discriminator byte
///   - `AsRef<str>` - For logging the instruction name
///
/// # Example
///
/// ```ignore
/// use panchor::process_instruction;
///
/// pinocchio_pubkey::entrypoint!(process);
///
/// fn process(
///     program_id: &Pubkey,
///     accounts: &[AccountInfo],
///     data: &[u8],
/// ) -> ProgramResult {
///     process_instruction::<MinesInstruction>(program_id, accounts, data, &ID)
/// }
/// ```
pub fn process_instruction<T>(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
    expected_program_id: &Pubkey,
) -> ProgramResult
where
    T: InstructionDispatch + TryFrom<u8> + AsRef<str>,
{
    // Verify program ID
    if !pubkey_eq(program_id, expected_program_id) {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Parse instruction discriminator
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let discriminator = instruction_data[0];
    let data = &instruction_data[1..]; // Strip discriminator for handlers

    // Parse instruction using TryFrom<u8>
    let instruction: T = discriminator.try_into().map_err(|_| {
        log!("Unknown instruction: {}", discriminator);
        ProgramError::InvalidInstructionData
    })?;

    // Log instruction name if not empty
    let name: &str = instruction.as_ref();
    log!("Instruction: {}", name);

    // Dispatch to appropriate handler
    instruction.dispatch(accounts, data)
}
