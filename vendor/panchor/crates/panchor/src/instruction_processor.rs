//! `InstructionDispatch` trait for instruction dispatch
//!
//! This trait is implemented by instruction enum types to handle dispatch
//! to the appropriate processor function based on the instruction discriminator.

use pinocchio::{ProgramResult, account_info::AccountInfo};

/// Trait for instruction enums that can dispatch to processor functions.
///
/// This trait is automatically derived by the `#[derive(InstructionDispatch)]` macro,
/// which is added by the `#[instructions]` attribute macro.
///
/// # Example
///
/// ```ignore
/// use panchor::InstructionDispatch;
///
/// #[instructions]
/// pub enum MyInstruction {
///     #[handler]
///     Initialize,
///
///     #[handler(data)]
///     Transfer,
/// }
///
/// // The macro generates:
/// impl InstructionDispatch for MyInstruction {
///     fn dispatch(&self, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
///         // ... dispatch logic
///     }
/// }
/// ```
pub trait InstructionDispatch {
    /// Dispatch the instruction to the appropriate processor function.
    ///
    /// # Arguments
    /// * `accounts` - The account infos passed to the instruction
    /// * `data` - The instruction data (excluding the discriminator byte)
    ///
    /// # Returns
    /// * `ProgramResult` - Success or error from the processor
    fn dispatch(&self, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult;
}
