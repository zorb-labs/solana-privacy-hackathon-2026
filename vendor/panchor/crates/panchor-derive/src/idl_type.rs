//! `IdlType` derive macro implementation
//!
//! Generates a marker implementation for types that should be included in IDL generation.
//! Also validates at compile time that all fields implement `IdlType` (except reference types).
//! Additionally implements `IdlBuildType` for generating IDL type definitions.
//!
//! Supports types with lifetimes (e.g., `MyStruct<'a>`). Reference types like `&'a str`
//! are handled specially - they map to "string" in the IDL without requiring `IdlType`.
//!
//! ## Serialization and Repr Detection
//!
//! The macro automatically detects `#[repr(C)]`:
//! - Sets `repr: Some(IdlRepr::C(...))`
//! - Sets `serialization: IdlSerialization::Bytemuck` (since repr(C) types use bytemuck Pod)
//!
//! ## Type Aliases
//!
//! For wrapper types (newtypes, bitflags) that should appear as primitives in the IDL,
//! use the `idl_type!` macro instead of `#[derive(IdlType)]`:
//!
//! ```ignore
//! idl_type!(Bps, alias = u16);
//! ```

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Error, Fields, Type};

use crate::utils::extract_docs;

/// Check if the type has `#[repr(C)]` attribute.
///
/// Types with repr(C) in this codebase use bytemuck Pod for serialization,
/// so we use this to determine the serialization method.
fn has_repr_c(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("repr")
            && let Ok(args) = attr.parse_args::<syn::Ident>()
        {
            return args == "C";
        }
        false
    })
}

/// Check if a type is a reference type (starts with &)
fn is_reference_type(ty: &Type) -> bool {
    matches!(ty, Type::Reference(_))
}

/// Check if a type is an array type
fn is_array_type(ty: &Type) -> bool {
    matches!(ty, Type::Array(_))
}

/// Check if a type path represents Pubkey (e.g., "Pubkey", "pubkey::Pubkey", etc.)
fn is_pubkey_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        // Get the last segment of the path (e.g., "Pubkey" from "pinocchio::pubkey::Pubkey")
        if let Some(last_segment) = type_path.path.segments.last() {
            return last_segment.ident == "Pubkey";
        }
    }
    false
}

/// Generate an IdlType expression for a given syn::Type.
/// This handles arrays specially by evaluating their length at compile time.
/// For types implementing IdlType, uses TYPE_NAME to get the canonical type name,
/// which allows type aliases (like Numeric -> u128) to be resolved correctly.
///
/// Arrays are handled recursively to support nested arrays like `[[u8; 32]; 26]`.
/// Without this, the inner `[u8; 32]` would use TYPE_NAME="array" which loses
/// the element type and size information.
///
/// Pubkey is handled specially because it's a type alias for `[u8; 32]`, so
/// its TYPE_NAME is "array" which would generate incorrect IDL. We detect
/// Pubkey at the AST level and directly generate IdlType::Pubkey.
fn generate_idl_type_expr(ty: &Type) -> TokenStream2 {
    // Handle Pubkey specially - it's a type alias for [u8; 32] but should be IdlType::Pubkey
    if is_pubkey_type(ty) {
        return quote! {
            ::panchor::panchor_idl::IdlType::Pubkey
        };
    }

    match ty {
        Type::Array(array) => {
            // For arrays, recursively generate the element type expression
            // This handles nested arrays like [[u8; 32]; 26] correctly
            let elem_ty = &array.elem;
            let len_expr = &array.len;
            let elem_type_expr = generate_idl_type_expr(elem_ty);
            quote! {
                {
                    // Evaluate array length at compile time
                    const LEN: usize = #len_expr;
                    ::panchor::panchor_idl::idl_array(
                        #elem_type_expr,
                        LEN
                    )
                }
            }
        }
        _ => {
            // Use the type's IdlType::TYPE_NAME constant to get the canonical name.
            // This allows types with aliases (like Numeric -> u128) to be resolved correctly.
            quote! {
                ::panchor::panchor_idl::rust_type_to_idl_type(
                    <#ty as ::panchor::IdlType>::TYPE_NAME
                )
            }
        }
    }
}

/// Implementation for `IdlType` derive macro
pub fn derive_idl_type_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;
    let name_str = name.to_string();
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Detect repr(C) for serialization/repr fields.
    // Types with repr(C) use bytemuck Pod for serialization in this codebase.
    let is_repr_c = has_repr_c(&input.attrs);

    // Generate serialization expression - repr(C) types use bytemuck
    let serialization_expr = if is_repr_c {
        quote! { ::panchor::panchor_idl::IdlSerialization::Bytemuck }
    } else {
        quote! { ::panchor::panchor_idl::IdlSerialization::default() }
    };

    // Generate repr expression
    let repr_expr = if is_repr_c {
        quote! {
            Some(::panchor::panchor_idl::IdlRepr::C(::panchor::panchor_idl::IdlReprModifier {
                packed: false,
                align: None,
            }))
        }
    } else {
        quote! { None }
    };

    // Check if the type has any lifetime parameters
    let has_lifetimes = input.generics.lifetimes().next().is_some();

    // Extract struct docs
    let struct_docs = extract_docs(&input.attrs);

    // Get field types for validation and IDL generation
    let (field_types, field_names, field_docs): (Vec<_>, Vec<_>, Vec<_>) = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => {
                let types: Vec<_> = fields.named.iter().map(|f| &f.ty).collect();
                let names: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap().to_string())
                    .collect();
                let docs: Vec<_> = fields
                    .named
                    .iter()
                    .map(|f| extract_docs(&f.attrs))
                    .collect();
                (types, names, docs)
            }
            Fields::Unnamed(fields) => {
                let types: Vec<_> = fields.unnamed.iter().map(|f| &f.ty).collect();
                let names: Vec<_> = (0..types.len()).map(|i| format!("field_{i}")).collect();
                let docs: Vec<_> = fields
                    .unnamed
                    .iter()
                    .map(|f| extract_docs(&f.attrs))
                    .collect();
                (types, names, docs)
            }
            Fields::Unit => (vec![], vec![], vec![]),
        },
        Data::Enum(_) => {
            return Error::new_spanned(
                &input.ident,
                "IdlType can only be derived for structs, not enums",
            )
            .to_compile_error();
        }
        Data::Union(_) => {
            return Error::new_spanned(
                &input.ident,
                "IdlType can only be derived for structs, not unions",
            )
            .to_compile_error();
        }
    };

    // Generate compile-time checks that each field type implements IdlType
    // Skip reference types (like &str) and array types since they can't implement IdlType
    // For arrays, the element type is checked via rust_type_to_idl_type
    let field_checks: Vec<_> = field_types
        .iter()
        .enumerate()
        .filter(|(_, ty)| !is_reference_type(ty) && !is_array_type(ty))
        .map(|(i, ty)| {
            let check_name = format_ident!("_idl_type_check_{}", i);
            quote! {
                const #check_name: () = {
                    // This will fail to compile if the type doesn't implement IdlType
                    let _ = <#ty as ::panchor::IdlType>::TYPE_NAME;
                };
            }
        })
        .collect();

    // Generate IDL field expressions for IdlBuildType
    // Use generate_idl_type_expr to handle array types with constant lengths
    let field_exprs: Vec<TokenStream2> = field_names
        .iter()
        .zip(field_docs.iter())
        .zip(field_types.iter())
        .map(|((name, docs), ty)| {
            let docs_expr = if docs.is_empty() {
                quote! { ::alloc::vec::Vec::new() }
            } else {
                quote! { ::alloc::vec![#(#docs.to_string()),*] }
            };

            let type_expr = generate_idl_type_expr(ty);

            quote! {
                ::panchor::panchor_idl::IdlField {
                    name: #name.to_string(),
                    docs: #docs_expr,
                    ty: #type_expr,
                }
            }
        })
        .collect();

    let struct_docs_expr = if struct_docs.is_empty() {
        quote! { ::alloc::vec::Vec::new() }
    } else {
        quote! { ::alloc::vec![#(#struct_docs.to_string()),*] }
    };

    // Generate test module for idl-build
    let test_mod_name = format_ident!("__idl_type_{}", name_str.to_lowercase());

    // Generate test function - use 'static lifetime for types with lifetime parameters
    let test_fn = if has_lifetimes {
        quote! {
            #[test]
            fn __idl_build_type() {
                use ::panchor::panchor_idl::IdlBuildType;
                let type_def = <#name::<'static> as IdlBuildType>::__idl_type_def();
                let json = ::serde_json::to_string_pretty(&type_def).expect("Failed to serialize type");
                std::println!("--- IDL type {} ---", #name_str);
                std::println!("{}", json);
                std::println!("--- end ---");
            }
        }
    } else {
        quote! {
            #[test]
            fn __idl_build_type() {
                use ::panchor::panchor_idl::IdlBuildType;
                let type_def = <#name as IdlBuildType>::__idl_type_def();
                let json = ::serde_json::to_string_pretty(&type_def).expect("Failed to serialize type");
                std::println!("--- IDL type {} ---", #name_str);
                std::println!("{}", json);
                std::println!("--- end ---");
            }
        }
    };

    quote! {
        impl #impl_generics ::panchor::IdlType for #name #ty_generics #where_clause {
            const TYPE_NAME: &'static str = stringify!(#name);
        }

        // Compile-time checks that all field types implement IdlType
        #[doc(hidden)]
        #[allow(dead_code)]
        const _: () = {
            #(#field_checks)*
        };

        #[cfg(feature = "idl-build")]
        impl #impl_generics ::panchor::panchor_idl::IdlBuildType for #name #ty_generics #where_clause {
            fn __idl_type_def() -> ::panchor::panchor_idl::IdlTypeDef {
                extern crate alloc;
                use alloc::string::ToString;
                ::panchor::panchor_idl::IdlTypeDef {
                    name: #name_str.to_string(),
                    docs: #struct_docs_expr,
                    serialization: #serialization_expr,
                    repr: #repr_expr,
                    generics: ::alloc::vec::Vec::new(),
                    ty: ::panchor::panchor_idl::IdlTypeDefTy::Struct {
                        fields: Some(::panchor::panchor_idl::IdlDefinedFields::Named(
                            ::alloc::vec![#(#field_exprs),*]
                        )),
                    },
                }
            }
        }

        #[cfg(feature = "idl-build")]
        impl #impl_generics ::panchor::panchor_idl::IdlBuildArgs for #name #ty_generics #where_clause {
            fn __idl_args() -> ::alloc::vec::Vec<::panchor::panchor_idl::IdlField> {
                extern crate alloc;
                use alloc::string::ToString;
                ::alloc::vec![#(#field_exprs),*]
            }
        }

        #[cfg(all(test, feature = "idl-build"))]
        mod #test_mod_name {
            extern crate std;
            extern crate alloc;
            use super::*;
            use alloc::string::ToString;

            #test_fn
        }
    }
}
