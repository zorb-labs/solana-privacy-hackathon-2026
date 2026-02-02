//! IDL generation trait for instruction enums.

use alloc::string::String;
use alloc::vec::Vec;

/// Trait for instruction enums that can provide IDL metadata.
///
/// This trait is automatically implemented by the `#[instructions]` macro.
/// It allows the `program!` macro to extract instruction metadata for IDL generation.
#[cfg(feature = "idl-build")]
pub trait InstructionIdl {
    /// Returns the IDL instruction definitions for this instruction enum.
    fn __idl_instructions() -> Vec<panchor_idl::IdlInstruction>;

    /// Returns type names that should be excluded from the IDL types array.
    /// This typically includes instruction data types that are already included
    /// in the instruction args.
    fn __idl_excluded_types() -> Vec<String>;
}
