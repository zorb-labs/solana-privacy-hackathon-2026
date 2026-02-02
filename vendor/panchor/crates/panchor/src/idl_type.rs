//! `IdlType` trait for IDL generation
//!
//! Types implementing this trait will be included in the generated IDL.
//! The derive macro validates at compile time that all fields also implement `IdlType`.

/// Marker trait for types that should be included in IDL generation.
///
/// # Example
///
/// ```ignore
/// use panchor::IdlType;
///
/// #[derive(IdlType)]
/// pub struct MyData {
///     pub amount: u64,
///     pub flag: bool,
/// }
/// ```
///
/// The derive macro will error at compile time if any field type doesn't implement `IdlType`:
///
/// ```ignore,compile_fail
/// struct NotIdlType;
///
/// #[derive(IdlType)]
/// pub struct InvalidData {
///     pub bad_field: NotIdlType, // Error: NotIdlType doesn't implement IdlType
/// }
/// ```
pub trait IdlType {
    /// The name of this type as it appears in the IDL
    const TYPE_NAME: &'static str;
}

// Implement IdlType for primitive types
impl IdlType for u8 {
    const TYPE_NAME: &'static str = "u8";
}

impl IdlType for u16 {
    const TYPE_NAME: &'static str = "u16";
}

impl IdlType for u32 {
    const TYPE_NAME: &'static str = "u32";
}

impl IdlType for u64 {
    const TYPE_NAME: &'static str = "u64";
}

impl IdlType for u128 {
    const TYPE_NAME: &'static str = "u128";
}

impl IdlType for i8 {
    const TYPE_NAME: &'static str = "i8";
}

impl IdlType for i16 {
    const TYPE_NAME: &'static str = "i16";
}

impl IdlType for i32 {
    const TYPE_NAME: &'static str = "i32";
}

impl IdlType for i64 {
    const TYPE_NAME: &'static str = "i64";
}

impl IdlType for i128 {
    const TYPE_NAME: &'static str = "i128";
}

impl IdlType for bool {
    const TYPE_NAME: &'static str = "bool";
}

impl IdlType for () {
    const TYPE_NAME: &'static str = "()";
}

impl IdlType for str {
    const TYPE_NAME: &'static Self = "string";
}

impl IdlType for &str {
    const TYPE_NAME: &'static str = "string";
}

// Implement for arrays of types that implement IdlType
// Note: Pubkey is a type alias for [u8; 32], so it uses this impl.
// The derive macro handles Pubkey specially at the AST level.
impl<T: IdlType, const N: usize> IdlType for [T; N] {
    const TYPE_NAME: &'static str = "array";
}

// Implement for Option
impl<T: IdlType> IdlType for Option<T> {
    const TYPE_NAME: &'static str = "option";
}

/// Macro to implement `IdlType` and `IdlBuildType` for a type alias.
///
/// This is useful for wrapper types (like bitflags or newtypes) that should
/// appear as their underlying primitive type in the IDL.
///
/// # Example
///
/// ```ignore
/// use panchor::idl_type;
///
/// bitflags! {
///     #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, bytemuck::Pod, bytemuck::Zeroable)]
///     #[repr(transparent)]
///     pub struct MineFlags: u8 {
///         const MINING_STARTED = 1 << 0;
///     }
/// }
///
/// idl_type!(MineFlags, alias = u8);
///
/// // For newtypes:
/// #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
/// #[repr(transparent)]
/// pub struct Bps(u16);
///
/// idl_type!(Bps, alias = u16);
/// ```
#[macro_export]
macro_rules! idl_type {
    ($name:ident, alias = $alias:ty) => {
        impl $crate::IdlType for $name {
            const TYPE_NAME: &'static str = stringify!($alias);
        }

        #[cfg(feature = "idl-build")]
        impl ::panchor_idl::IdlBuildType for $name {
            fn __idl_type_def() -> ::panchor_idl::IdlTypeDef {
                extern crate alloc;
                use alloc::string::ToString;
                ::panchor_idl::IdlTypeDef {
                    name: stringify!($name).to_string(),
                    docs: alloc::vec::Vec::new(),
                    serialization: ::panchor_idl::IdlSerialization::default(),
                    repr: None,
                    generics: alloc::vec::Vec::new(),
                    ty: ::panchor_idl::IdlTypeDefTy::Type {
                        alias: ::panchor_idl::rust_type_to_idl_type(stringify!($alias)),
                    },
                }
            }
        }

        $crate::__idl_type_alias_test!($name);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_type_names() {
        // Verify primitive types have correct TYPE_NAMEs
        assert_eq!(u8::TYPE_NAME, "u8");
        assert_eq!(u16::TYPE_NAME, "u16");
        assert_eq!(u32::TYPE_NAME, "u32");
        assert_eq!(u64::TYPE_NAME, "u64");
        assert_eq!(u128::TYPE_NAME, "u128");
        assert_eq!(i8::TYPE_NAME, "i8");
        assert_eq!(i16::TYPE_NAME, "i16");
        assert_eq!(i32::TYPE_NAME, "i32");
        assert_eq!(i64::TYPE_NAME, "i64");
        assert_eq!(i128::TYPE_NAME, "i128");
        assert_eq!(bool::TYPE_NAME, "bool");
    }

    #[test]
    fn test_array_type_name() {
        // Arrays should have TYPE_NAME "array"
        assert_eq!(<[u8; 32]>::TYPE_NAME, "array");
    }

    #[test]
    fn test_option_type_name() {
        // Options should have TYPE_NAME "option"
        assert_eq!(<Option<u64>>::TYPE_NAME, "option");
    }
}

/// Internal macro to generate the IDL build test for idl_type_alias.
/// This is separate to handle the conditional compilation properly.
#[doc(hidden)]
#[macro_export]
macro_rules! __idl_type_alias_test {
    ($name:ident) => {
        $crate::paste::paste! {
            #[cfg(all(test, feature = "idl-build"))]
            mod [<__idl_type_ $name:lower>] {
                extern crate std;
                extern crate alloc;
                use super::*;
                use alloc::string::ToString;

                #[test]
                fn __idl_build_type() {
                    use ::panchor_idl::IdlBuildType;
                    let type_def = <$name as IdlBuildType>::__idl_type_def();
                    let json = ::serde_json::to_string_pretty(&type_def)
                        .expect("Failed to serialize type");
                    std::println!("--- IDL type {} ---", stringify!($name));
                    std::println!("{}", json);
                    std::println!("--- end ---");
                }
            }
        }
    };
}
