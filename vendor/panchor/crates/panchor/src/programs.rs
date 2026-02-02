//! Common Solana program marker types
//!
//! These marker structs implement [`Id`] and can be used with
//! [`Program<'info, T>`](crate::accounts::Program) for type-safe program account validation.
//!
//! # Example
//!
//! ```ignore
//! use panchor::prelude::*;
//!
//! #[derive(Accounts)]
//! pub struct MyAccounts<'info> {
//!     pub system_program: Program<'info, System>,
//!     pub token_program: Program<'info, Token>,
//!     pub associated_token_program: Program<'info, AssociatedToken>,
//!     pub token_metadata_program: Program<'info, TokenMetadata>,
//! }
//! ```

use pinocchio::pubkey::Pubkey;
use pinocchio_contrib::constants::{
    ASSOCIATED_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID, TOKEN_METADATA_PROGRAM_ID, TOKEN_PROGRAM_ID,
};

use crate::accounts::Id;

/// System Program marker type.
///
/// Used with `Program<'info, System>` to validate the System Program.
pub struct System;

impl Id for System {
    const ID: Pubkey = SYSTEM_PROGRAM_ID;
}

/// SPL Token Program marker type.
///
/// Used with `Program<'info, Token>` to validate the SPL Token Program.
pub struct Token;

impl Id for Token {
    const ID: Pubkey = TOKEN_PROGRAM_ID;
}

/// Associated Token Program marker type.
///
/// Used with `Program<'info, AssociatedToken>` to validate the Associated Token Program.
pub struct AssociatedToken;

impl Id for AssociatedToken {
    const ID: Pubkey = ASSOCIATED_TOKEN_PROGRAM_ID;
}

/// Metaplex Token Metadata Program marker type.
///
/// Used with `Program<'info, TokenMetadata>` to validate the Token Metadata Program.
pub struct TokenMetadata;

impl Id for TokenMetadata {
    const ID: Pubkey = TOKEN_METADATA_PROGRAM_ID;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_id() {
        assert_eq!(System::ID, SYSTEM_PROGRAM_ID);
        assert_eq!(System::id(), &SYSTEM_PROGRAM_ID);
    }

    #[test]
    fn test_token_id() {
        assert_eq!(Token::ID, TOKEN_PROGRAM_ID);
        assert_eq!(Token::id(), &TOKEN_PROGRAM_ID);
    }

    #[test]
    fn test_associated_token_id() {
        assert_eq!(AssociatedToken::ID, ASSOCIATED_TOKEN_PROGRAM_ID);
        assert_eq!(AssociatedToken::id(), &ASSOCIATED_TOKEN_PROGRAM_ID);
    }

    #[test]
    fn test_token_metadata_id() {
        assert_eq!(TokenMetadata::ID, TOKEN_METADATA_PROGRAM_ID);
        assert_eq!(TokenMetadata::id(), &TOKEN_METADATA_PROGRAM_ID);
    }
}
