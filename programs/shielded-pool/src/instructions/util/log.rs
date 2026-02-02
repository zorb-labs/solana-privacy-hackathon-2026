use crate::errors::ShieldedPoolError;
use alloc::vec::Vec;
use pinocchio::{
    ProgramResult, account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey,
};
use pinocchio_log::log;

/// Log event data.
/// This instruction calls sol_log_data with the provided data.
/// Access is restricted to accounts owned by this program.
///
/// # Accounts
///
/// 0. `[signer]` A PDA owned by this program (proves caller is this program via CPI)
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: Vec<u8>) -> ProgramResult {
    let [authority, ..] = accounts else {
        log!("log: missing required accounts");
        return Err(ShieldedPoolError::MissingAccounts.into());
    };

    // Authority must be a signer (PDA signed via invoke_signed)
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Authority must be owned by this program
    if authority.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    pinocchio::log::sol_log_data(&[&data]);
    Ok(())
}
