//! `AsAccountInfo` trait for types that hold an `AccountInfo` reference

use pinocchio::account_info::AccountInfo;

/// Trait for types that hold an `AccountInfo` reference
pub trait AsAccountInfo<'info> {
    /// Returns the inner `AccountInfo` reference
    fn account_info(&self) -> &'info AccountInfo;
}

/// Implement `AsAccountInfo` for raw `AccountInfo` reference
impl<'info> AsAccountInfo<'info> for &'info AccountInfo {
    #[inline(always)]
    fn account_info(&self) -> &'info AccountInfo {
        self
    }
}
