//! `SetBump` trait for optionally setting bump seeds on account creation

/// Trait for optionally setting the bump seed after account creation.
///
/// This trait is used internally by the `init` and `init_idempotent` constraints
/// to automatically set the bump on account types that have a bump field.
///
/// When `#[account(..., bump)]` is used, the derive macro generates an
/// implementation that sets the bump field. For types without bump,
/// the generated implementation is a no-op.
pub trait SetBump {
    /// Optionally set the bump seed on this account.
    ///
    /// For account types with `#[account(..., bump)]`, this sets the bump field.
    /// For account types without bump, this is a no-op.
    fn set_bump(&mut self, bump: u8);
}

// Note: No blanket implementation - each account type must implement this trait.
// The #[account] derive macro generates the implementation.
