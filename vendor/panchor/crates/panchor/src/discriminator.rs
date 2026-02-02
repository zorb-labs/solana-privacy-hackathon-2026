//! Discriminator trait for account type identification

/// Discriminator length in bytes (8 bytes for u64)
pub const DISCRIMINATOR_LEN: usize = 8;

/// Trait for account types with a discriminator
///
/// Implement this trait on account structs to enable discriminator-checked
/// loading via `as_account_checked()` and `as_account_checked_mut()`.
///
/// # Example
///
/// ```ignore
/// use panchor::Discriminator;
///
/// #[repr(C)]
/// #[derive(Pod, Zeroable)]
/// pub struct MyAccount {
///     pub discriminator: u64,
///     // ... other fields
/// }
///
/// impl Discriminator for MyAccount {
///     const DISCRIMINATOR: u64 = 100;
/// }
/// ```
pub trait Discriminator {
    /// The expected discriminator value for this account type (8 bytes)
    const DISCRIMINATOR: u64;
}

/// Trait for setting the discriminator on account data.
///
/// This trait provides a method to write the discriminator bytes at the
/// beginning of an account's data buffer.
pub trait SetDiscriminator: Discriminator {
    /// Set the discriminator on the account data buffer.
    ///
    /// Writes the discriminator as little-endian bytes to the first 8 bytes.
    #[inline]
    fn set_discriminator(data: &mut [u8]) {
        data[..DISCRIMINATOR_LEN].copy_from_slice(&Self::DISCRIMINATOR.to_le_bytes());
    }
}

// Blanket implementation for all types that implement Discriminator
impl<T: Discriminator> SetDiscriminator for T {}
