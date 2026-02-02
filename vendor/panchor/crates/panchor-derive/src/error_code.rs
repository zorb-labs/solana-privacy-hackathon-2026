//! Errors attribute macro
//!
//! Generates IDL error metadata and utility implementations from an error enum.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error};

use crate::utils::extract_doc;

/// Core implementation for errors attribute macro
pub fn errors_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;

    // Get enum variants
    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Error::new_spanned(&input.ident, "errors attribute only supports enums")
                .to_compile_error();
        }
    };

    // Build error definitions using anchor's IdlErrorCode
    let mut error_builders: Vec<TokenStream2> = Vec::new();

    for variant in variants {
        let variant_name = &variant.ident;
        let variant_name_str = variant_name.to_string();

        // Extract doc comment for the error message
        let msg_expr = if let Some(msg) = extract_doc(&variant.attrs) {
            quote! { Some(#msg.to_string()) }
        } else {
            quote! { None }
        };

        error_builders.push(quote! {
            errors.push(::panchor::panchor_idl::IdlErrorCode {
                code: #name::#variant_name as u32,
                name: #variant_name_str.to_string(),
                msg: #msg_expr,
            });
        });
    }

    // Generate test module name
    let test_mod_name = format_ident!("__idl_errors_{}", name.to_string().to_lowercase());

    quote! {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            ::panchor::strum::IntoStaticStr,
            ::panchor::num_enum::TryFromPrimitive
        )]
        #[repr(u32)]
        #input

        impl #name {
            /// Returns the error name as a static string.
            #[inline]
            pub fn name(&self) -> &'static str {
                self.into()
            }
        }

        impl ::panchor::pinocchio::program_error::ToStr for #name {
            fn to_str<E>(&self) -> &'static str
            where
                E: 'static + ::panchor::pinocchio::program_error::ToStr + TryFrom<u32>,
            {
                self.into()
            }
        }

        impl From<#name> for u64 {
            #[inline]
            fn from(e: #name) -> Self {
                e as Self
            }
        }

        impl From<#name> for ::panchor::pinocchio::program_error::ProgramError {
            #[inline]
            fn from(e: #name) -> Self {
                Self::Custom(e as u32)
            }
        }

        #[cfg(feature = "idl-build")]
        impl ::panchor::panchor_idl::IdlBuildErrors for #name {
            fn __idl_errors() -> ::alloc::vec::Vec<::panchor::panchor_idl::IdlErrorCode> {
                extern crate alloc;
                use alloc::string::ToString;
                let mut errors = ::alloc::vec::Vec::new();
                #(#error_builders)*
                errors
            }
        }

        #[cfg(all(test, feature = "idl-build"))]
        mod #test_mod_name {
            extern crate std;
            extern crate alloc;
            use super::*;
            use alloc::string::ToString;

            #[test]
            fn __idl_build_errors() {
                use ::panchor::panchor_idl::IdlBuildErrors;
                let errors = <#name as IdlBuildErrors>::__idl_errors();
                let json = ::serde_json::to_string_pretty(&errors).expect("Failed to serialize errors");
                std::println!("--- IDL begin errors ---");
                std::println!("{}", json);
                std::println!("--- IDL end errors ---");
            }
        }
    }
}
