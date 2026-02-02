//! Zero-copy attribute macro for deriving repr(C), Copy, Clone, Pod, Zeroable, Eq, and `PartialEq`.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, parse_quote};

/// Check if the input already has a repr(C) attribute.
fn has_repr_c(input: &DeriveInput) -> bool {
    input.attrs.iter().any(|attr| {
        if attr.path().is_ident("repr")
            && let Ok(nested) = attr.parse_args::<syn::Ident>()
        {
            return nested == "C";
        }
        false
    })
}

/// Core implementation for `zero_copy` attribute macro.
///
/// Adds `#[repr(C)]` (if not already present) and derives Copy, Clone, Pod, Zeroable, Eq, and `PartialEq`.
pub fn zero_copy_impl(mut input: DeriveInput) -> TokenStream2 {
    // Add derives for zero-copy compatible types
    let derives: syn::Attribute = parse_quote! {
        #[derive(Clone, Copy, PartialEq, Eq, ::panchor::bytemuck::Pod, ::panchor::bytemuck::Zeroable)]
    };

    // Only add repr(C) if not already present
    if has_repr_c(&input) {
        input.attrs.insert(0, derives);
    } else {
        let repr_c: syn::Attribute = parse_quote! {
            #[repr(C)]
        };
        input.attrs.insert(0, repr_c);
        input.attrs.insert(1, derives);
    }

    quote! {
        #input
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_and_expand(input: TokenStream2) -> TokenStream2 {
        let input = syn::parse2::<DeriveInput>(input).unwrap();
        zero_copy_impl(input)
    }

    #[test]
    fn test_zero_copy_adds_repr_c() {
        let input = quote! {
            pub struct Point {
                pub x: i32,
                pub y: i32,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        assert!(output_str.contains("repr (C)"));
    }

    #[test]
    fn test_zero_copy_adds_derives() {
        let input = quote! {
            pub struct Point {
                pub x: i32,
                pub y: i32,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        assert!(output_str.contains("Clone"));
        assert!(output_str.contains("Copy"));
        assert!(output_str.contains("PartialEq"));
        assert!(output_str.contains("Eq"));
        assert!(output_str.contains("Pod"));
        assert!(output_str.contains("Zeroable"));
    }

    #[test]
    fn test_zero_copy_preserves_fields() {
        let input = quote! {
            pub struct Point {
                pub x: i32,
                pub y: i32,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        assert!(output_str.contains("pub x : i32"));
        assert!(output_str.contains("pub y : i32"));
    }

    #[test]
    fn test_zero_copy_preserves_docs() {
        let input = quote! {
            /// A 2D point
            pub struct Point {
                /// X coordinate
                pub x: i32,
                /// Y coordinate
                pub y: i32,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        assert!(output_str.contains("A 2D point"));
        assert!(output_str.contains("X coordinate"));
        assert!(output_str.contains("Y coordinate"));
    }
}
