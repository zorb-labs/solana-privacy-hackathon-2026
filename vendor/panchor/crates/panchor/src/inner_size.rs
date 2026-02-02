//! `InnerSize` trait for account data size calculation

use bytemuck::Pod;

/// Trait for types that have a fixed inner data size for account allocation
///
/// Implement this trait on account structs to enable automatic size calculation
/// when creating accounts via the `create_pda` helper.
///
/// This trait is automatically implemented for all types that implement `Pod`,
/// using `core::mem::size_of::<Self>()` as the size.
pub trait InnerSize {
    /// The size in bytes of this account type's data
    const INNER_SIZE: usize;
}

/// Blanket implementation of `InnerSize` for all `Pod` types.
///
/// Since `Pod` types are plain-old-data with a fixed memory layout,
/// their size is simply `size_of::<Self>()`.
impl<T: Pod> InnerSize for T {
    const INNER_SIZE: usize = core::mem::size_of::<Self>();
}
