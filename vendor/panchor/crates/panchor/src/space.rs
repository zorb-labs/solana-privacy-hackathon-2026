//! `InitSpace` trait for calculating account allocation size

use crate::{Discriminator, InnerSize};

/// The size of a discriminator in bytes (always 8 bytes / u64)
pub const DISCRIMINATOR_SIZE: usize = core::mem::size_of::<u64>();

/// Trait for types that have a known total space requirement for account initialization.
///
/// This trait is automatically implemented for any type that implements both
/// `Discriminator` and `InnerSize`. The total space is calculated as:
/// `DISCRIMINATOR_SIZE + INNER_SIZE`
///
/// # Example
///
/// ```ignore
/// use panchor::prelude::*;
///
/// // If MyAccount implements Discriminator + InnerSize,
/// // InitSpace is automatically available:
/// let space = MyAccount::INIT_SPACE; // = 8 + sizeof(MyAccount)
/// ```
pub trait InitSpace {
    /// The total space in bytes needed for this account type
    const INIT_SPACE: usize;
}

/// Blanket implementation: any type with Discriminator + `InnerSize` has `InitSpace`
impl<T: Discriminator + InnerSize> InitSpace for T {
    const INIT_SPACE: usize = DISCRIMINATOR_SIZE + T::INNER_SIZE;
}
