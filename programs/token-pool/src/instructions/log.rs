//! Log instruction handler for emitting events.
//!
//! This instruction is invoked via CPI from within the program to emit events.
//! The actual event data is passed as raw bytes and emitted to program logs.
//!
//! # Security
//!
//! The Log instruction validates that the authority account:
//! 1. Is a signer (proves `invoke_signed` was used with valid PDA seeds)
//! 2. Is owned by this program (proves the PDA belongs to this program)
//!
//! Together these checks ensure only this program's code paths can emit events.

use panchor::prelude::*;
use pinocchio::{ProgramResult, account_info::AccountInfo, program_error::ProgramError};

/// Accounts for Log instruction.
#[derive(Accounts)]
pub struct LogAccounts<'info> {
    /// Authority PDA that signed the CPI call.
    /// Must be owned by this program and signed via invoke_signed.
    pub authority: &'info AccountInfo,
}

/// Process a log instruction.
///
/// This instruction simply logs the provided data. It's called via self-CPI
/// with a program-owned PDA as signer to ensure only valid program invocations
/// can emit events.
///
/// # Security Checks
///
/// - Authority must be a signer (PDA signed via `invoke_signed`)
/// - Authority must be owned by this program (proves it's our PDA)
///
/// The event data format is: [length (4 bytes), discriminator (8 bytes), event fields...]
pub fn process_log(ctx: Context<LogAccounts>, data: &[u8]) -> ProgramResult {
    let LogAccounts { authority } = ctx.accounts;

    // Authority must be a signer (PDA signed via invoke_signed)
    if !authority.is_signer() {
        log!("log: authority must be a signer");
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Authority must be owned by this program (proves it's our PDA)
    if authority.owner() != &crate::ID {
        log!("log: authority must be owned by this program");
        return Err(ProgramError::IllegalOwner);
    }

    // Parse the length prefix (4 bytes, little-endian)
    if data.len() < 4 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let event_data = &data[4..];

    if event_data.len() < len {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Log the raw event bytes using base64 encoding
    // Format: "Program data: <base64_encoded_data>"
    pinocchio::log::sol_log_data(&[&event_data[..len]]);

    log!("Token pool event emitted");

    Ok(())
}
