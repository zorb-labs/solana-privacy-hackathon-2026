//! Initialize a deposit escrow for relayer-assisted deposits.
//!
//! This instruction creates an escrow account and vault, then transfers tokens
//! from the depositor to the vault. The escrow is bound to a specific proof hash,
//! which the relayer must match when executing the transact.

use crate::{
    errors::ShieldedPoolError,
    events::{DepositEscrowCreatedEvent, emit_event},
    pda::{find_deposit_escrow_pda, find_escrow_vault_authority_pda, gen_deposit_escrow_seeds},
    state::DepositEscrow,
};
use panchor::prelude::*;
use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::Signer as PinocchioSigner,
    pubkey::Pubkey,
    sysvars::{Sysvar, clock::Clock, rent::Rent},
};
use pinocchio_associated_token_account::instructions::CreateIdempotent;
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::Transfer;

/// Instruction data for InitDepositEscrow.
///
/// # Fields
/// - `proof_hash`: SHA256(session_body) that this escrow is bound to
/// - `nonce`: Unique nonce for this escrow (allows multiple concurrent escrows)
/// - `amount`: Amount of tokens to transfer to escrow vault
/// - `authorized_relayer`: [0;32] allows any relayer, otherwise specific relayer only
/// - `expiry_slots`: Number of slots after creation when escrow expires
#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable, InstructionArgs, IdlType)]
pub struct InitDepositEscrowData {
    /// SHA256 hash of the session_body that this escrow is bound to.
    pub proof_hash: [u8; 32],
    /// Unique nonce for this escrow.
    pub nonce: u64,
    /// Amount of tokens to transfer to escrow vault.
    pub amount: u64,
    /// Authorized relayer pubkey, or [0;32] to allow any relayer.
    pub authorized_relayer: Pubkey,
    /// Number of slots after creation when escrow expires.
    /// User can reclaim tokens after expiry.
    pub expiry_slots: u64,
}

/// Accounts for InitDepositEscrow instruction.
///
/// # Account Layout
/// 0. `[signer, mut]` depositor - Pays for account creation and transfers tokens
/// 1. `[mut]` escrow - PDA to create ["deposit_escrow", depositor, nonce]
/// 2. `[]` escrow_vault_authority - PDA that owns the vault ["escrow_vault_authority", escrow]
/// 3. `[mut]` escrow_vault - ATA of escrow_vault_authority for the mint
/// 4. `[mut]` depositor_token_account - Source token account
/// 5. `[]` mint - SPL token mint
/// 6. `[]` token_program - SPL Token program
/// 7. `[]` associated_token_program - Associated Token program
/// 8. `[]` system_program - System program
/// 9. `[]` shielded_pool_program - This program (for event emission)
#[derive(Accounts)]
pub struct InitDepositEscrowAccounts<'info> {
    /// Depositor (payer) for the escrow.
    /// Must be a signer and will transfer tokens to the escrow vault.
    #[account(mut)]
    pub depositor: Signer<'info>,

    /// Escrow PDA to create ["deposit_escrow", depositor, nonce]
    /// Raw AccountInfo since we're creating this account via CPI.
    #[account(mut)]
    pub escrow: &'info AccountInfo,

    /// Escrow vault authority PDA ["escrow_vault_authority", escrow]
    /// This PDA will own the escrow vault and sign transfers out.
    pub escrow_vault_authority: &'info AccountInfo,

    /// Escrow vault - ATA of escrow_vault_authority for the mint.
    /// Created if it doesn't exist.
    #[account(mut)]
    pub escrow_vault: &'info AccountInfo,

    /// Depositor's token account (source of tokens).
    #[account(mut)]
    pub depositor_token_account: &'info AccountInfo,

    /// SPL token mint for this escrow.
    pub mint: &'info AccountInfo,

    /// SPL Token program.
    pub token_program: Program<'info, Token>,

    /// Associated Token program.
    pub associated_token_program: Program<'info, AssociatedToken>,

    /// System program for account creation.
    pub system_program: Program<'info, System>,

    /// Shielded pool program (for event emission via self-CPI)
    #[account(address = crate::ID)]
    pub shielded_pool_program: &'info AccountInfo,
}

/// Initialize a deposit escrow for relayer-assisted deposits.
///
/// Creates the escrow PDA, creates the escrow vault ATA, transfers tokens
/// from the depositor to the vault, and initializes the escrow state.
pub fn process_init_deposit_escrow(
    ctx: Context<InitDepositEscrowAccounts>,
    data: InitDepositEscrowData,
) -> ProgramResult {
    let InitDepositEscrowAccounts {
        depositor,
        escrow,
        escrow_vault_authority,
        escrow_vault,
        depositor_token_account,
        mint,
        token_program,
        associated_token_program: _,
        system_program,
        shielded_pool_program,
    } = ctx.accounts;

    let program_id = &crate::ID;

    // ========================================================================
    // 1. VALIDATE AND CREATE ESCROW PDA
    // ========================================================================

    // Derive and verify escrow PDA
    let (expected_escrow_pda, escrow_bump) = find_deposit_escrow_pda(depositor.key(), data.nonce);
    if escrow.key() != &expected_escrow_pda {
        log!("init_deposit_escrow: invalid escrow PDA");
        return Err(pinocchio::program_error::ProgramError::InvalidSeeds);
    }

    // Create the escrow account
    let space = DepositEscrow::ACCOUNT_SIZE;
    let rent = Rent::get()?;

    let nonce_bytes = data.nonce.to_le_bytes();
    let bump_slice = [escrow_bump];
    let seeds = gen_deposit_escrow_seeds(depositor.key(), &nonce_bytes, &bump_slice);
    let signer = PinocchioSigner::from(&seeds);

    CreateAccount {
        from: depositor,
        to: escrow,
        lamports: rent.minimum_balance(space),
        space: space as u64,
        owner: program_id,
    }
    .invoke_signed(&[signer])?;

    // ========================================================================
    // 2. VALIDATE ESCROW VAULT AUTHORITY PDA
    // ========================================================================

    let (expected_vault_authority, _vault_auth_bump) =
        find_escrow_vault_authority_pda(escrow.key());
    if escrow_vault_authority.key() != &expected_vault_authority {
        log!("init_deposit_escrow: invalid escrow_vault_authority PDA");
        return Err(pinocchio::program_error::ProgramError::InvalidSeeds);
    }

    // ========================================================================
    // 3. CREATE ESCROW VAULT ATA (if needed)
    // ========================================================================

    // Create the escrow vault ATA owned by escrow_vault_authority
    CreateIdempotent {
        funding_account: depositor,
        account: escrow_vault,
        wallet: escrow_vault_authority,
        mint,
        system_program,
        token_program,
    }
    .invoke()?;

    // ========================================================================
    // 4. TRANSFER TOKENS TO ESCROW VAULT
    // ========================================================================

    if data.amount > 0 {
        Transfer {
            from: depositor_token_account,
            to: escrow_vault,
            authority: depositor,
            amount: data.amount,
        }
        .invoke()?;
    }

    // ========================================================================
    // 5. INITIALIZE ESCROW STATE
    // ========================================================================

    // Get current slot for expiry calculation
    let clock = Clock::get()?;
    let expiry_slot = clock
        .slot
        .checked_add(data.expiry_slots)
        .ok_or(ShieldedPoolError::ArithmeticOverflow)?;

    // Initialize the escrow account
    {
        let mut escrow_data = escrow.try_borrow_mut_data()?;

        // Write discriminator
        escrow_data[..8].copy_from_slice(&DepositEscrow::DISCRIMINATOR.to_le_bytes());

        // Write escrow fields
        let escrow_state =
            bytemuck::from_bytes_mut::<DepositEscrow>(&mut escrow_data[8..DepositEscrow::ACCOUNT_SIZE]);
        escrow_state.proof_hash = data.proof_hash;
        escrow_state.mint = *mint.key();
        escrow_state.authorized_relayer = data.authorized_relayer;
        escrow_state.expiry_slot = expiry_slot;
        escrow_state.nonce = data.nonce;
        escrow_state.consumed = 0;
        escrow_state.bump = escrow_bump;
        escrow_state._padding = [0u8; 6];
    }

    log!("init_deposit_escrow: escrow created successfully");

    // Emit event using escrow PDA as signer (already created above)
    //
    // AUDIT: We use the escrow PDA as the event signer since it's already
    // present in this instruction. This avoids adding global_config as an
    // extra account. If this pattern proves problematic (e.g., for indexer
    // consistency), we may introduce a dedicated event-signing PDA.
    let nonce_bytes = data.nonce.to_le_bytes();
    let escrow_bump_slice = [escrow_bump];
    let escrow_seeds = gen_deposit_escrow_seeds(depositor.key(), &nonce_bytes, &escrow_bump_slice);
    let escrow_signer = PinocchioSigner::from(&escrow_seeds);

    let event = DepositEscrowCreatedEvent {
        depositor: *depositor.key(),
        escrow: *escrow.key(),
        mint: *mint.key(),
        proof_hash: data.proof_hash,
        authorized_relayer: data.authorized_relayer,
        expiry_slot,
        nonce: data.nonce,
        amount: data.amount,
    };

    emit_event(escrow, shielded_pool_program, escrow_signer, &event)?;

    Ok(())
}
