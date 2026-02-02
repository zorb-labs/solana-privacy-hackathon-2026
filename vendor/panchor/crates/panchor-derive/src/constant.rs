//! Constant attribute macro
//!
//! Marks a constant for inclusion in IDL generation.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{ItemConst, Type};

use crate::utils::extract_docs;

/// Convert a type to its IDL string representation
fn type_to_idl_string(ty: &Type) -> String {
    use quote::ToTokens;
    ty.to_token_stream().to_string().replace(' ', "")
}

/// Check if a type is a numeric type that can be represented as a string in IDL
fn is_numeric_type(ty: &Type) -> bool {
    let ty_str = type_to_idl_string(ty);
    matches!(
        ty_str.as_str(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" | "usize"
    )
}

/// Core implementation for constant attribute macro
pub fn constant_impl(input: ItemConst) -> TokenStream2 {
    let name = &input.ident;
    let name_str = name.to_string();
    let ty = &input.ty;
    let ty_str = type_to_idl_string(ty);

    // Extract doc comments
    let docs = extract_docs(&input.attrs);

    // Generate test module name (unique per constant)
    let test_mod_name = format_ident!("__idl_constant_{}", name_str.to_lowercase());

    // Generate the value expression for the test
    // For numeric types, we can directly format them
    let value_expr = if is_numeric_type(ty) {
        quote! {
            // Format numeric value as string
            ::alloc::format!("{}", #name)
        }
    } else {
        // For other types (like Bps), try to convert to a representable form
        quote! {
            // Try to represent as string - may need custom handling
            ::alloc::format!("{:?}", #name)
        }
    };

    quote! {
        #input

        #[cfg(all(test, feature = "idl-build"))]
        mod #test_mod_name {
            extern crate std;
            extern crate alloc;
            use super::*;
            use alloc::string::ToString;

            #[test]
            fn __idl_build_constant() {
                use ::panchor::panchor_idl::IdlConst;

                let constant = IdlConst {
                    name: #name_str.to_string(),
                    docs: ::alloc::vec![#(#docs.to_string()),*],
                    ty: ::panchor::panchor_idl::rust_type_to_idl_type(#ty_str),
                    value: #value_expr,
                };
                let json = ::serde_json::to_string_pretty(&constant).expect("Failed to serialize constant");
                std::println!("--- IDL constant {} ---", #name_str);
                std::println!("{}", json);
                std::println!("--- end ---");
            }
        }
    }
}
