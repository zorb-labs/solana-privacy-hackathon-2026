//! `InstructionArgs` derive macro implementation
//!
//! Generates `TryFrom`<&[u8]> implementation for instruction data structs using bytemuck.
//!
//! Note: `IdlBuildArgs` is implemented by the `IdlType` derive macro, which should also
//! be derived on instruction data structs.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields};

/// Implementation for `InstructionArgs` derive macro
///
/// Generates `TryFrom`<&[u8]> for parsing instruction data.
/// For IDL generation, derive `IdlType` separately - it implements `IdlBuildArgs`.
pub fn derive_instruction_args_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Ensure this is a struct with named fields
    match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(_) => {}
            _ => {
                return Error::new_spanned(
                    &input.ident,
                    "InstructionArgs only supports structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return Error::new_spanned(
                &input.ident,
                "InstructionArgs can only be derived for structs",
            )
            .to_compile_error();
        }
    }

    quote! {
        impl #impl_generics ::core::convert::TryFrom<&[u8]> for #name #ty_generics #where_clause {
            type Error = ::panchor::pinocchio::program_error::ProgramError;

            #[inline]
            fn try_from(data: &[u8]) -> ::core::result::Result<Self, Self::Error> {
                ::panchor::parse_instruction_data(data)
            }
        }
    }
}
