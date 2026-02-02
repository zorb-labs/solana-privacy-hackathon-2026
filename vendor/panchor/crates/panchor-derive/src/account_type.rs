//! `AccountType` attribute macro
//!
//! Generates an account type enum with proper derives for discriminators.
//!
//! This macro:
//! - Adds #[repr(u64)] to ensure proper discriminator layout
//! - Adds derives: Clone, Copy, Debug, Eq, `PartialEq`, `PartialOrd`, Ord, `TryFromPrimitive`
//! - Implements `from_u64` using `TryFromPrimitive`

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Error, parse_quote};

/// Core implementation for `account_type` attribute macro
pub fn account_type_impl(mut input: DeriveInput) -> TokenStream2 {
    // Ensure this is an enum
    if !matches!(&input.data, Data::Enum(_)) {
        return Error::new_spanned(&input.ident, "account_type attribute only supports enums")
            .to_compile_error();
    }

    // Add #[repr(u64)] attribute
    let repr: syn::Attribute = parse_quote! {
        #[repr(u64)]
    };
    input.attrs.insert(0, repr);

    // Add standard derives
    let derives: syn::Attribute = parse_quote! {
        #[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, ::panchor::num_enum::TryFromPrimitive)]
    };
    input.attrs.insert(1, derives);

    let name = &input.ident;

    quote! {
        #input

        impl #name {
            /// Convert from u64 to the account type variant.
            /// Returns `None` if the value doesn't match any variant.
            pub fn from_u64(value: u64) -> Option<Self> {
                Self::try_from(value).ok()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_and_expand(input: TokenStream2) -> TokenStream2 {
        let input = syn::parse2::<DeriveInput>(input).unwrap();
        account_type_impl(input)
    }

    #[test]
    fn test_account_type_basic() {
        let input = quote! {
            pub enum MinesAccount {
                Automation = 100,
                Mine = 101,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check that repr(u64) was added
        assert!(output_str.contains("repr"));
        assert!(output_str.contains("u64"));

        // Check that derives were added
        assert!(output_str.contains("Clone"));
        assert!(output_str.contains("Copy"));
        assert!(output_str.contains("Debug"));
        assert!(output_str.contains("Eq"));
        assert!(output_str.contains("PartialEq"));
        assert!(output_str.contains("PartialOrd"));
        assert!(output_str.contains("Ord"));
        assert!(output_str.contains("TryFromPrimitive"));

        // Check that from_u64 method is generated using try_from
        assert!(output_str.contains("fn from_u64"));
        assert!(output_str.contains("try_from"));
    }

    #[test]
    fn test_account_type_only_enums() {
        let input = quote! {
            pub struct NotAnEnum {
                pub data: u64,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should contain an error about only supporting enums
        assert!(output_str.contains("only supports enums"));
    }
}
