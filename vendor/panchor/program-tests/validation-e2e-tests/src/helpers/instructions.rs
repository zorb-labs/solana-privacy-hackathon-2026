//! Instruction builders for validation-test program

use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use super::{SYSTEM_PROGRAM_ID, constants::PROGRAM_ID, find_test_account_pda};

/// Build `TestSigner` instruction (discriminator = 0)
///
/// Tests: #[account(signer)]
pub fn test_signer(authority: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*authority, true)],
        data: vec![0], // discriminator
    }
}

/// Build `TestSigner` instruction with invalid signer flag
///
/// This should fail with `MissingRequiredSignature`
pub fn test_signer_invalid(authority: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*authority, false)], // NOT a signer
        data: vec![0],
    }
}

/// Build `TestMutable` instruction (discriminator = 1)
///
/// Tests: #[account(mut)]
pub fn test_mutable(target: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new(*target, false)], // writable
        data: vec![1],
    }
}

/// Build `TestMutable` instruction with readonly account
///
/// This should fail with `InvalidAccountData`
pub fn test_mutable_invalid(target: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*target, false)], // NOT writable
        data: vec![1],
    }
}

/// Build `TestOwner` instruction (discriminator = 2)
///
/// Tests: `AccountLoader`<T> - validates owner, discriminator, size
pub fn test_owner(test_account: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*test_account, false)],
        data: vec![2],
    }
}

/// Build `TestProgram` instruction (discriminator = 3)
///
/// Tests: Program<T> - validates executable and program ID
pub fn test_program(system_program: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*system_program, false)],
        data: vec![3],
    }
}

/// Build `TestAddress` instruction (discriminator = 4)
///
/// Tests: #[account(address = expr)]
pub fn test_address(target: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*target, false)],
        data: vec![4],
    }
}

/// Build `TestInit` instruction (discriminator = 5)
///
/// Tests: #[account(init, seeds = [...], payer = ...)]
pub fn test_init(payer: &Pubkey) -> Instruction {
    let (test_account, _) = find_test_account_pda(payer);

    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(test_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data: vec![5],
    }
}

/// Build `TestOwnerConstraint` instruction (discriminator = 6)
///
/// Tests: #[account(owner = expr)]
pub fn test_owner_constraint(target: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*target, false)],
        data: vec![6],
    }
}

/// Build `TestSignerWrapper` instruction (discriminator = 7)
///
/// Tests: Signer<'info> wrapper with valid signer
pub fn test_signer_wrapper(authority: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*authority, true)],
        data: vec![7],
    }
}

/// Build `TestSignerWrapper` instruction with invalid signer flag
///
/// Tests: Signer<'info> wrapper - should fail with `MissingRequiredSignature`
pub fn test_signer_wrapper_invalid(authority: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*authority, false)], // NOT a signer
        data: vec![7],
    }
}

/// Build `TestLazyMint` instruction (discriminator = 8)
///
/// Tests: `LazyAccount`<'info, Mint> - validates Token Program owner and 82-byte size
pub fn test_lazy_mint(mint: &Pubkey) -> Instruction {
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![AccountMeta::new_readonly(*mint, false)],
        data: vec![8],
    }
}
