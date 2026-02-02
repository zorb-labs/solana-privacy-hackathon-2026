//! Id trait for types with known addresses

use pinocchio::pubkey::Pubkey;

/// Trait for types that have a known address.
///
/// This is typically implemented for:
/// - Program marker types (System Program, Token Program, etc.)
/// - Singleton accounts with fixed addresses (e.g., `GlobalState`)
///
/// # Example
///
/// ```ignore
/// // For a program
/// pub struct TokenProgram;
/// impl Id for TokenProgram {
///     const ID: Pubkey = TOKEN_PROGRAM_ID;
/// }
///
/// // For a singleton account
/// #[account(MinesAccount::GlobalState, id = GLOBAL_STATE_ADDRESS)]
/// pub struct GlobalState { ... }
/// ```
pub trait Id {
    /// The known address for this type.
    const ID: Pubkey;

    /// Returns the address as a reference.
    #[inline]
    fn id() -> &'static Pubkey {
        &Self::ID
    }
}
