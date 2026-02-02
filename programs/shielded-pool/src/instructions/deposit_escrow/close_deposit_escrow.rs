//! Close a deposit escrow and reclaim tokens.
//!
//! This instruction allows the depositor to reclaim their tokens after the escrow
//! has expired. It transfers all tokens from the escrow vault back to the depositor,
//! closes the vault ATA, and closes the escrow account (returning rent lamports).

use crate::{
    errors::ShieldedPoolError,
    events::{DepositEscrowClosedEvent, emit_event},
    pda::{
        find_deposit_escrow_pda, find_escrow_vault_authority_pda,
        gen_deposit_escrow_seeds, gen_escrow_vault_authority_seeds,
    },
    state::DepositEscrow,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    sysvars::{Sysvar, clock::Clock},
};
use pinocchio_log::log;
use pinocchio_token::instructions::{CloseAccount, Transfer};

/// Instruction data for CloseDepositEscrow.
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct CloseDepositEscrowData {
    /// Escrow nonce (used to derive the PDA).
    pub nonce: u64,
}

/// Accounts for CloseDepositEscrow instruction.
///
/// # Account Layout
/// 0. `[signer, mut]` depositor - Original escrow creator, receives rent and tokens
/// 1. `[mut]` escrow - Escrow PDA to close
/// 2. `[]` escrow_vault_authority - PDA that owns the vault
/// 3. `[mut]` escrow_vault - ATA to close and transfer tokens from
/// 4. `[mut]` depositor_token_account - Destination for reclaimed tokens
/// 5. `[]` token_program - SPL Token program
/// 6. `[]` shielded_pool_program - This program (for event emission)
#[derive(Accounts)]
pub struct CloseDepositEscrowAccounts<'info> {
    /// Depositor (original escrow creator).
    /// Must be a signer and will receive rent and tokens back.
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// Escrow PDA to close ["deposit_escrow", depositor, nonce]
    #[account(mut)]
    pub escrow: &'info AccountInfo,

    /// Escrow vault authority PDA ["escrow_vault_authority", escrow]
    /// Signs transfers and close instructions.
    pub escrow_vault_authority: &'info AccountInfo,

    /// Escrow vault ATA to close.
    #[account(mut)]
    pub escrow_vault: &'info AccountInfo,

    /// Depositor's token account (receives reclaimed tokens).
    #[account(mut)]
    pub depositor_token_account: &'info AccountInfo,

    /// SPL Token program.
    pub token_program: Program<'info, Token>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Close a deposit escrow and reclaim tokens.
///
/// # Requirements
/// - Escrow must exist and not be consumed
/// - Current slot must be past expiry_slot
/// - Depositor must be the original creator (verified via PDA)
pub fn process_close_deposit_escrow(
    ctx: Context<CloseDepositEscrowAccounts>,
    data: CloseDepositEscrowData,
) -> ProgramResult {
    let CloseDepositEscrowAccounts {
        depositor,
        escrow,
        escrow_vault_authority,
        escrow_vault,
        depositor_token_account,
        token_program: _,
        shielded_pool_program,
    } = ctx.accounts;

    let program_id = &crate::ID;

    // ========================================================================
    // 1. VALIDATE ESCROW PDA
    // ========================================================================

    // Verify escrow PDA derivation (this also proves depositor is the creator)
    let (expected_escrow_pda, _escrow_bump) = find_deposit_escrow_pda(depositor.key(), data.nonce);
    if escrow.key() != &expected_escrow_pda {
        log!("close_deposit_escrow: invalid escrow PDA");
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    // Verify escrow is owned by this program
    if escrow.owner() != program_id {
        log!("close_deposit_escrow: escrow not owned by program");
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    // ========================================================================
    // 2. LOAD AND VALIDATE ESCROW STATE
    // ========================================================================

    let escrow_data = escrow.try_borrow_data()?;
    if escrow_data.len() < DepositEscrow::ACCOUNT_SIZE {
        log!("close_deposit_escrow: escrow data too small");
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    // Verify discriminator
    let discriminator = u64::from_le_bytes(escrow_data[..8].try_into().unwrap());
    if discriminator != DepositEscrow::DISCRIMINATOR {
        log!("close_deposit_escrow: invalid discriminator");
        return Err(ShieldedPoolError::InvalidEscrowAccount.into());
    }

    let escrow_state =
        bytemuck::from_bytes::<DepositEscrow>(&escrow_data[8..DepositEscrow::ACCOUNT_SIZE]);

    // Verify escrow is not already consumed
    if escrow_state.is_consumed() {
        log!("close_deposit_escrow: escrow already consumed");
        return Err(ShieldedPoolError::EscrowAlreadyConsumed.into());
    }

    // Verify escrow has expired
    let clock = Clock::get()?;
    if !escrow_state.is_expired(clock.slot) {
        log!("close_deposit_escrow: escrow not expired");
        return Err(ShieldedPoolError::EscrowNotExpired.into());
    }

    // Save data for event emission (before we close the escrow)
    let mint = escrow_state.mint;
    let escrow_bump = escrow_state.bump;

    // Drop borrow before we modify escrow
    drop(escrow_data);

    // ========================================================================
    // 3. VALIDATE ESCROW VAULT AUTHORITY PDA
    // ========================================================================

    let (expected_vault_authority, vault_auth_bump) =
        find_escrow_vault_authority_pda(escrow.key());
    if escrow_vault_authority.key() != &expected_vault_authority {
        log!("close_deposit_escrow: invalid escrow_vault_authority PDA");
        return Err(pinocchio::program_error::ProgramError::InvalidSeeds);
    }

    // ========================================================================
    // 4. TRANSFER TOKENS FROM VAULT TO DEPOSITOR
    // ========================================================================

    // Get vault balance
    let vault_balance = crate::token::get_token_account_balance(escrow_vault)?;

    if vault_balance > 0 {
        // Generate signer seeds for escrow_vault_authority
        let escrow_key = escrow.key();
        let bump_slice = [vault_auth_bump];
        let seeds = gen_escrow_vault_authority_seeds(escrow_key, &bump_slice);
        let signer = PinocchioSigner::from(&seeds);

        Transfer {
            from: escrow_vault,
            to: depositor_token_account,
            authority: escrow_vault_authority,
            amount: vault_balance,
        }
        .invoke_signed(&[signer])?;
    }

    // ========================================================================
    // 5. CLOSE ESCROW VAULT ATA
    // ========================================================================

    {
        let escrow_key = escrow.key();
        let bump_slice = [vault_auth_bump];
        let seeds = gen_escrow_vault_authority_seeds(escrow_key, &bump_slice);
        let signer = PinocchioSigner::from(&seeds);

        CloseAccount {
            account: escrow_vault,
            destination: depositor,
            authority: escrow_vault_authority,
        }
        .invoke_signed(&[signer])?;
    }

    // ========================================================================
    // 6. EMIT EVENT (before closing escrow - need escrow PDA to sign)
    // ========================================================================
    //
    // AUDIT: We use the escrow PDA as the event signer since it's already
    // present in this instruction. This must happen BEFORE closing the escrow
    // because the PDA must be owned by this program to sign. If this pattern
    // proves problematic (e.g., for indexer consistency or if we need events
    // after closure), we may introduce a dedicated event-signing PDA.

    // Get escrow lamports before closing
    let escrow_lamports = escrow.lamports();

    // Emit event using escrow PDA as signer
    let nonce_bytes = data.nonce.to_le_bytes();
    let escrow_bump_slice = [escrow_bump];
    let escrow_seeds = gen_deposit_escrow_seeds(depositor.key(), &nonce_bytes, &escrow_bump_slice);
    let escrow_signer = PinocchioSigner::from(&escrow_seeds);

    let event = DepositEscrowClosedEvent {
        depositor: *depositor.key(),
        escrow: *escrow.key(),
        mint,
        amount_returned: vault_balance,
        nonce: data.nonce,
        lamports_reclaimed: escrow_lamports,
    };

    emit_event(escrow, shielded_pool_program, escrow_signer, &event)?;

    // ========================================================================
    // 7. CLOSE ESCROW ACCOUNT (return rent)
    // ========================================================================

    // Transfer lamports from escrow to depositor
    unsafe {
        *escrow.borrow_mut_lamports_unchecked() = 0;
        *depositor.borrow_mut_lamports_unchecked() = depositor
            .lamports()
            .checked_add(escrow_lamports)
            .ok_or(ShieldedPoolError::ArithmeticOverflow)?;
    }

    // Zero out escrow data to mark it as closed
    let mut escrow_data = escrow.try_borrow_mut_data()?;
    escrow_data.fill(0);

    // Reassign escrow to system program (marks it as fully closed)
    // SAFETY: We have exclusive access to the escrow account (validated via PDA)
    // and are closing it properly by zeroing data first.
    unsafe {
        escrow.assign(&pinocchio_system::ID);
    }

    log!("close_deposit_escrow: escrow closed successfully");

    Ok(())
}
