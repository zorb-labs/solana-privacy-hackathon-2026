//! Upload a chunk of transaction data to a tra
use crate::{
    errors::ShieldedPoolError,
    state::{TRANSACT_SESSION_HEADER_SIZE, TransactSession},
};
use panchor::prelude::*;
use pinocchio::ProgramResult;
use pinocchio_log::log;

/// Accounts for UploadTransactChunk instruction.
#[derive(Accounts)]
pub struct UploadTransactChunkAccounts<'info> {
    /// Transact session PDA
    #[account(mut)]
    pub transact_session: AccountLoader<'info, TransactSession>,

    /// Authority (must match session creator)
    pub authority: Signer<'info>,
}

/// Upload a chunk of transaction data to a transact session account.
///
/// This instruction allows uploading transaction data in multiple transactions
/// to work around transaction size limits. Chunks can be uploaded in any
/// order as long as they don't overlap.
///
/// # Raw Data Format
///
/// * `[0..4]` - offset: u32 (little-endian) - Byte offset where this chunk should be written
/// * `[4..]` - data: bytes - The chunk data to write
pub fn process_upload_transact_chunk(
    ctx: Context<UploadTransactChunkAccounts>,
    data: &[u8],
) -> ProgramResult {
    let UploadTransactChunkAccounts {
        transact_session,
        authority,
    } = ctx.accounts;

    let program_id = &crate::ID;

    // Parse the raw data:
    // The Codama-generated TypeScript client wraps data with a Borsh u32 length prefix.
    // Wire format: [borsh_len(4)][offset(4)][chunk_data]
    // So we skip the first 4 bytes (Borsh length prefix) to get to the actual data.
    if data.len() < 8 {
        log!("upload_transact_chunk: invalid data length (need at least 8 bytes)");
        return Err(pinocchio::program_error::ProgramError::InvalidInstructionData);
    }
    // Skip Borsh length prefix (bytes 0-3), read offset from bytes 4-7
    let offset = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let chunk_data = &data[8..];

    // Load header to validate authority and data_len, then extract values and drop borrow
    let (header_authority, header_data_len) = {
        let header = TransactSession::load_header(transact_session, program_id)?;
        (header.authority, header.data_len)
    };

    // Verify authority
    if header_authority != *authority.key() {
        log!("upload_transact_chunk: unauthorized");
        return Err(ShieldedPoolError::Unauthorized.into());
    }

    // Validate offset and data length
    let end_offset = offset
        .checked_add(chunk_data.len() as u32)
        .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

    if end_offset > header_data_len {
        log!("upload_transact_chunk: chunk exceeds data_len");
        return Err(ShieldedPoolError::ProofPayloadOverflow.into());
    }

    // Write the chunk to data section (after header)
    // Note: Overlapping writes are intentionally allowed - the authority owns the session
    // and may overwrite their own data if needed (e.g., to correct upload errors).
    let mut account_data = transact_session.try_borrow_mut_data()?;
    let body = &mut account_data[TRANSACT_SESSION_HEADER_SIZE..];
    let start = offset as usize;
    let end = end_offset as usize;
    body[start..end].copy_from_slice(chunk_data);

    log!("upload_transact_chunk: chunk written successfully");

    Ok(())
}
