use pinocchio::{
    ProgramResult,
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use pinocchio_token::{
    instructions::{CloseAccount, SyncNative, Transfer, TransferChecked},
    state::{Mint, TokenAccount},
};

/// SPL Token Program ID
pub const SPL_TOKEN_PROGRAM_ID: Pubkey = [
    0x06, 0xdd, 0xf6, 0xe1, 0xd7, 0x65, 0xa1, 0x93, 0xd9, 0xcb, 0xe1, 0x46, 0xce, 0xeb, 0x79, 0xac,
    0x1c, 0xb4, 0x85, 0xed, 0x5f, 0x5b, 0x37, 0x91, 0x3a, 0x8c, 0xf5, 0x85, 0x7e, 0xff, 0x00, 0xa9,
];

/// SPL Token-2022 Program ID
pub const SPL_TOKEN_2022_PROGRAM_ID: Pubkey = [
    0x06, 0xa7, 0xd5, 0x17, 0x18, 0x7b, 0xd1, 0x65, 0x35, 0x50, 0xc4, 0x9a, 0x3a, 0x8b, 0x9a, 0x28,
    0xb9, 0x51, 0x9f, 0x60, 0x7d, 0x1f, 0x55, 0xb8, 0x26, 0xb4, 0x53, 0x06, 0x76, 0x8b, 0x9f, 0x71,
];

/// Associated Token Account Program ID
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = [
    0x8c, 0x97, 0x25, 0x8f, 0x4e, 0x24, 0x89, 0xf1, 0xbb, 0x3d, 0x10, 0x29, 0x14, 0x8e, 0x0d, 0x83,
    0x0b, 0x5a, 0x13, 0x99, 0xda, 0xff, 0x10, 0x84, 0x04, 0x8e, 0x7b, 0xd8, 0xdb, 0xe9, 0xf8, 0x59,
];

/// Check if a program ID is a valid token program (SPL Token or Token-2022)
pub fn is_token_program(program_id: &Pubkey) -> bool {
    *program_id == SPL_TOKEN_PROGRAM_ID || *program_id == SPL_TOKEN_2022_PROGRAM_ID
}

/// Transfer tokens from source to destination (user-signed)
pub fn transfer_tokens(
    source: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    _token_program: &AccountInfo,
) -> ProgramResult {
    Transfer {
        from: source,
        to: destination,
        authority,
        amount,
    }
    .invoke()?;
    Ok(())
}

/// Transfer tokens from source to destination with PDA signer
///
/// signer_seeds should be a slice of seeds (e.g., &[b"vault", tree_key.as_ref(), &[bump]])
pub fn transfer_tokens_signed(
    source: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    _token_program: &AccountInfo,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    // Convert signer_seeds to Seed types
    let seeds: [Seed; 3] = [
        Seed::from(signer_seeds[0]),
        Seed::from(signer_seeds[1]),
        Seed::from(signer_seeds[2]),
    ];
    let signer = [Signer::from(&seeds[..])];

    Transfer {
        from: source,
        to: destination,
        authority,
        amount,
    }
    .invoke_signed(&signer)?;
    Ok(())
}

/// Transfer tokens with decimals check (for Token-2022 compatibility)
pub fn transfer_checked(
    source: &AccountInfo,
    mint: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    decimals: u8,
    _token_program: &AccountInfo,
) -> ProgramResult {
    TransferChecked {
        from: source,
        mint,
        to: destination,
        authority,
        amount,
        decimals,
    }
    .invoke()?;
    Ok(())
}

/// Transfer tokens with decimals check and PDA signer
pub fn transfer_checked_signed(
    source: &AccountInfo,
    mint: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    amount: u64,
    decimals: u8,
    _token_program: &AccountInfo,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    // Convert signer_seeds to Seed types
    let seeds: [Seed; 3] = [
        Seed::from(signer_seeds[0]),
        Seed::from(signer_seeds[1]),
        Seed::from(signer_seeds[2]),
    ];
    let signer = [Signer::from(&seeds[..])];

    TransferChecked {
        from: source,
        mint,
        to: destination,
        authority,
        amount,
        decimals,
    }
    .invoke_signed(&signer)?;
    Ok(())
}

/// SPL Token Mint size
pub const MINT_SIZE: usize = 82;

/// Decode decimals from a mint account using pinocchio_token state
pub fn get_mint_decimals(mint_account: &AccountInfo) -> Result<u8, ProgramError> {
    let mint = Mint::from_account_info(mint_account)?;
    Ok(mint.decimals())
}

/// Decode supply from a mint account using pinocchio_token state
pub fn get_mint_supply(mint_account: &AccountInfo) -> Result<u64, ProgramError> {
    let mint = Mint::from_account_info(mint_account)?;
    Ok(mint.supply())
}

/// Check if mint is initialized using pinocchio_token state
pub fn is_mint_initialized(mint_account: &AccountInfo) -> Result<bool, ProgramError> {
    let mint = Mint::from_account_info(mint_account)?;
    Ok(mint.is_initialized())
}

/// Verify that a token account belongs to the expected owner
pub fn verify_token_account_owner(
    token_account: &AccountInfo,
    expected_owner: &Pubkey,
) -> Result<(), ProgramError> {
    let account = TokenAccount::from_account_info(token_account)?;
    if account.owner() != expected_owner {
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

// ============================================================================
// WSOL Helpers
// ============================================================================

/// WSOL (Native Mint) address
pub const WSOL_MINT: Pubkey = [
    0x06, 0x9b, 0x88, 0x57, 0xfe, 0xab, 0x81, 0x84, 0xfb, 0x68, 0x7f, 0x63, 0x46, 0x18, 0xc0, 0x35,
    0xda, 0xc4, 0x39, 0xdc, 0x1a, 0xeb, 0x3b, 0x55, 0x98, 0xa0, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x01,
];

/// Token account size
pub const TOKEN_ACCOUNT_SIZE: usize = 165;

/// Get the balance of a token account using pinocchio_token state
pub fn get_token_account_balance(token_account: &AccountInfo) -> Result<u64, ProgramError> {
    let account = TokenAccount::from_account_info(token_account)?;
    Ok(account.amount())
}

/// Get the mint of a token account using pinocchio_token state
pub fn get_token_account_mint(token_account: &AccountInfo) -> Result<Pubkey, ProgramError> {
    let account = TokenAccount::from_account_info(token_account)?;
    Ok(*account.mint())
}

/// Close a token account and send lamports to destination.
/// For WSOL accounts, this effectively "unwraps" the WSOL to native SOL.
pub fn close_token_account(
    account: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    _token_program: &AccountInfo,
) -> ProgramResult {
    CloseAccount {
        account,
        destination,
        authority,
    }
    .invoke()?;
    Ok(())
}

/// Close a token account with PDA signer.
/// For WSOL accounts, this effectively "unwraps" the WSOL to native SOL.
pub fn close_token_account_signed(
    account: &AccountInfo,
    destination: &AccountInfo,
    authority: &AccountInfo,
    _token_program: &AccountInfo,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let seeds: [Seed; 3] = [
        Seed::from(signer_seeds[0]),
        Seed::from(signer_seeds[1]),
        Seed::from(signer_seeds[2]),
    ];
    let signer = [Signer::from(&seeds[..])];

    CloseAccount {
        account,
        destination,
        authority,
    }
    .invoke_signed(&signer)?;
    Ok(())
}

/// Sync native token account balance with lamport balance.
/// Call this after transferring SOL lamports to a WSOL token account
/// to update the token balance to reflect the new lamport amount.
pub fn sync_native(native_token: &AccountInfo, _token_program: &AccountInfo) -> ProgramResult {
    SyncNative { native_token }.invoke()?;
    Ok(())
}

/// Check if a token account holds WSOL
pub fn is_wsol_account(token_account: &AccountInfo) -> Result<bool, ProgramError> {
    let mint = get_token_account_mint(token_account)?;
    Ok(mint == WSOL_MINT)
}

/// Derive the Associated Token Account address for a wallet and mint.
///
/// The ATA is a PDA derived from:
/// - seeds: [wallet_address, token_program_id, mint_address]
/// - program_id: ASSOCIATED_TOKEN_PROGRAM_ID
///
/// This returns the canonical ATA address that wallets like Phantom use by default.
pub fn find_associated_token_address(
    wallet_address: &Pubkey,
    mint_address: &Pubkey,
    token_program_id: &Pubkey,
) -> (Pubkey, u8) {
    pinocchio::pubkey::find_program_address(
        &[wallet_address, token_program_id, mint_address],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
}

/// Verify that a token account is the canonical ATA for the given wallet and mint.
///
/// Returns Ok(()) if the token_account address matches the derived ATA,
/// or InvalidAccountData if it doesn't.
pub fn require_associated_token_account(
    token_account: &AccountInfo,
    wallet_address: &Pubkey,
    mint_address: &Pubkey,
    token_program_id: &Pubkey,
) -> Result<(), ProgramError> {
    let (expected_ata, _) =
        find_associated_token_address(wallet_address, mint_address, token_program_id);
    if token_account.key() != &expected_ata {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(())
}
