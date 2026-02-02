//! Account builder for creating mock `AccountInfo` objects in tests.

use core::mem::size_of;

use pinocchio::account_info::AccountInfo;
use pinocchio::pubkey::Pubkey;

/// Borrow state marker indicating the account is not borrowed.
/// From pinocchio's entrypoint module.
const NOT_BORROWED: u8 = 0b_1111_1111;

/// Raw account data structure.
///
/// This mirrors the internal structure that pinocchio's `AccountInfo` expects.
/// The layout must match exactly for the `AccountInfo` wrapper to work correctly.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawAccount {
    /// Borrow state for lamports and account data.
    borrow_state: u8,
    /// Indicates whether the transaction was signed by this account.
    is_signer: u8,
    /// Indicates whether the account is writable.
    is_writable: u8,
    /// Indicates whether this account represents a program.
    executable: u8,
    /// Difference between the original data length and the current data length.
    resize_delta: i32,
    /// Public key of the account.
    key: Pubkey,
    /// Program that owns this account.
    owner: Pubkey,
    /// The lamports in the account.
    lamports: u64,
    /// Length of the data.
    data_len: u64,
}

/// A test account that owns its data and provides an `AccountInfo` reference.
///
/// This struct holds the raw account data and any associated data buffer,
/// providing a safe way to create `AccountInfo` objects for testing.
pub struct TestAccount {
    /// The raw account data followed by the account data bytes.
    /// This is stored as a Vec<u8> to ensure proper memory layout.
    buffer: Vec<u8>,
}

impl TestAccount {
    /// Get an `AccountInfo` reference to this test account.
    ///
    /// # Safety
    /// The returned `AccountInfo` is only valid while this `TestAccount` is alive.
    pub fn info(&self) -> AccountInfo {
        // AccountInfo is a simple wrapper around a pointer to the raw account data.
        // We create an AccountInfo by transmuting the pointer layout.
        //
        // SAFETY: AccountInfo is repr(C) with a single field `raw: *mut Account`.
        // We're creating the exact same memory layout.
        unsafe { core::mem::transmute(self.buffer.as_ptr()) }
    }
}

/// Builder for creating test `AccountInfo` objects.
///
/// # Example
///
/// ```rust
/// use pinocchio_test_utils::AccountInfoBuilder;
/// use pinocchio::pubkey::Pubkey;
///
/// let key = Pubkey::default();
/// let owner = Pubkey::default();
/// let account = AccountInfoBuilder::new()
///     .key(&key)
///     .owner(&owner)
///     .signer(true)
///     .writable(true)
///     .lamports(1_000_000)
///     .data(&[1, 2, 3, 4])
///     .build();
///
/// assert!(account.info().is_signer());
/// assert!(account.info().is_writable());
/// ```
#[derive(Default)]
pub struct AccountInfoBuilder<'a> {
    key: Option<&'a Pubkey>,
    owner: Option<&'a Pubkey>,
    is_signer: bool,
    is_writable: bool,
    executable: bool,
    lamports: u64,
    data: Option<&'a [u8]>,
}

impl<'a> AccountInfoBuilder<'a> {
    /// Create a new `AccountInfoBuilder` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the account's public key.
    pub fn key(mut self, key: &'a Pubkey) -> Self {
        self.key = Some(key);
        self
    }

    /// Set the program that owns this account.
    pub fn owner(mut self, owner: &'a Pubkey) -> Self {
        self.owner = Some(owner);
        self
    }

    /// Set whether the account is a signer.
    pub fn signer(mut self, is_signer: bool) -> Self {
        self.is_signer = is_signer;
        self
    }

    /// Set whether the account is writable.
    pub fn writable(mut self, is_writable: bool) -> Self {
        self.is_writable = is_writable;
        self
    }

    /// Set whether the account is executable (a program).
    pub fn executable(mut self, executable: bool) -> Self {
        self.executable = executable;
        self
    }

    /// Set the lamports in the account.
    pub fn lamports(mut self, lamports: u64) -> Self {
        self.lamports = lamports;
        self
    }

    /// Set the account's data.
    pub fn data(mut self, data: &'a [u8]) -> Self {
        self.data = Some(data);
        self
    }

    /// Build the `TestAccount`.
    #[allow(clippy::cast_ptr_alignment)]
    pub fn build(self) -> TestAccount {
        let data = self.data.unwrap_or(&[]);
        let data_len = data.len();

        // Allocate buffer for RawAccount struct + data
        let buffer_size = size_of::<RawAccount>() + data_len;
        let mut buffer = vec![0u8; buffer_size];

        // Get a mutable pointer to the RawAccount portion
        let raw = buffer.as_mut_ptr().cast::<RawAccount>();

        let default_key = Pubkey::default();
        let key = self.key.unwrap_or(&default_key);
        let owner = self.owner.unwrap_or(&default_key);

        // SAFETY: We allocated enough space for RawAccount
        unsafe {
            (*raw).borrow_state = NOT_BORROWED;
            (*raw).is_signer = u8::from(self.is_signer);
            (*raw).is_writable = u8::from(self.is_writable);
            (*raw).executable = u8::from(self.executable);
            (*raw).resize_delta = 0;
            (*raw).key = *key;
            (*raw).owner = *owner;
            (*raw).lamports = self.lamports;
            (*raw).data_len = data_len as u64;
        }

        // Copy data after the RawAccount struct
        if !data.is_empty() {
            let data_start = size_of::<RawAccount>();
            buffer[data_start..data_start + data_len].copy_from_slice(data);
        }

        TestAccount { buffer }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_default_values() {
        let account = AccountInfoBuilder::new().build();
        let info = account.info();

        assert!(!info.is_signer());
        assert!(!info.is_writable());
        assert!(!info.executable());
        assert_eq!(info.lamports(), 0);
        assert_eq!(info.data_len(), 0);
    }

    #[test]
    fn test_builder_with_signer() {
        let account = AccountInfoBuilder::new().signer(true).build();
        let info = account.info();

        assert!(info.is_signer());
        assert!(!info.is_writable());
    }

    #[test]
    fn test_builder_with_writable() {
        let account = AccountInfoBuilder::new().writable(true).build();
        let info = account.info();

        assert!(!info.is_signer());
        assert!(info.is_writable());
    }

    #[test]
    fn test_builder_with_lamports() {
        let account = AccountInfoBuilder::new().lamports(1_000_000).build();
        let info = account.info();

        assert_eq!(info.lamports(), 1_000_000);
    }

    #[test]
    fn test_builder_with_data() {
        let data = [1u8, 2, 3, 4, 5];
        let account = AccountInfoBuilder::new().data(&data).build();
        let info = account.info();

        assert_eq!(info.data_len(), 5);

        // Verify data contents
        // SAFETY: We just created this account with the data
        let borrowed_data = unsafe { info.borrow_data_unchecked() };
        assert_eq!(borrowed_data, &data);
    }

    #[test]
    fn test_builder_with_key() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let account = AccountInfoBuilder::new().key(&key).build();
        let info = account.info();

        assert_eq!(info.key(), &key);
    }

    #[test]
    fn test_builder_with_owner() {
        let owner = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let account = AccountInfoBuilder::new().owner(&owner).build();
        let info = account.info();

        assert_eq!(info.owner(), &owner);
    }

    #[test]
    fn test_builder_with_executable() {
        let account = AccountInfoBuilder::new().executable(true).build();
        let info = account.info();

        assert!(info.executable());
    }

    #[test]
    fn test_builder_full_config() {
        let key = pinocchio_pubkey::pubkey!("11111111111111111111111111111111");
        let owner = pinocchio_pubkey::pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
        let data = [10u8, 20, 30];

        let account = AccountInfoBuilder::new()
            .key(&key)
            .owner(&owner)
            .signer(true)
            .writable(true)
            .lamports(500_000)
            .data(&data)
            .build();

        let info = account.info();

        assert_eq!(info.key(), &key);
        assert_eq!(info.owner(), &owner);
        assert!(info.is_signer());
        assert!(info.is_writable());
        assert_eq!(info.lamports(), 500_000);
        assert_eq!(info.data_len(), 3);

        let borrowed_data = unsafe { info.borrow_data_unchecked() };
        assert_eq!(borrowed_data, &data);
    }
}
