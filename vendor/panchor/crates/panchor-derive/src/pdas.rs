//! PDA generation attribute macro
//!
//! Generates seed constants, finder functions, and seed generator functions.

use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, LitStr, Type};

use crate::utils::{
    extract_docs, extract_seeds_attr, is_u64_type, to_screaming_snake_case, to_snake_case,
};

/// A field with name and type
struct FieldDef {
    name: Ident,
    ty: Type,
}

/// Parsed PDA definition for code generation
struct PdaDef {
    name: Ident,
    seed: LitStr,
    fields: Vec<FieldDef>,
    docs: Vec<String>,
}

/// Generate the PDA module contents from an enum
pub fn pdas_impl(mut input: DeriveInput) -> TokenStream2 {
    let Data::Enum(ref mut data_enum) = input.data else {
        return syn::Error::new_spanned(&input, "pdas attribute can only be applied to enums")
            .to_compile_error();
    };

    // First pass: collect all PDA definitions
    let mut pda_defs: Vec<PdaDef> = Vec::new();

    for variant in &data_enum.variants {
        let name = variant.ident.clone();

        // Extract #[seeds("...")] attribute
        let Some(seed) = extract_seeds_attr(&variant.attrs) else {
            return syn::Error::new_spanned(
                variant,
                format!("variant {name} is missing #[seeds(\"...\")]"),
            )
            .to_compile_error();
        };

        // Extract doc comments
        let docs = extract_docs(&variant.attrs);

        // Extract fields with their types
        let fields: Vec<FieldDef> = match &variant.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .filter_map(|f| {
                    f.ident.as_ref().map(|name| FieldDef {
                        name: name.clone(),
                        ty: f.ty.clone(),
                    })
                })
                .collect(),
            Fields::Unit => vec![],
            Fields::Unnamed(_) => {
                return syn::Error::new_spanned(
                    variant,
                    "pdas variants must use named fields: Pool { mint: Pubkey }",
                )
                .to_compile_error();
            }
        };

        pda_defs.push(PdaDef {
            name,
            seed,
            fields,
            docs,
        });
    }

    // Strip #[seeds(...)] attributes from variants since no derive is consuming them
    for variant in &mut data_enum.variants {
        variant.attrs.retain(|attr| !attr.path().is_ident("seeds"));
    }

    // Add derives to the enum: Clone, Copy, Debug, PartialEq, Eq
    let has_derive = input
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("derive"));
    if !has_derive {
        input
            .attrs
            .push(syn::parse_quote!(#[derive(Clone, Copy, Debug, PartialEq, Eq)]));
    }

    let mut output = TokenStream2::new();

    // Output the modified enum
    output.extend(quote! { #input });

    // Generate seed constants as byte string literals
    for pda in &pda_defs {
        let name = &pda.name;
        let seed = &pda.seed;

        // Generate UPPER_SEED constant name
        let seed_const = format_ident!("{}_SEED", to_screaming_snake_case(&name.to_string()));

        // Use byte string literal b"..." for the seed
        let seed_bytes = Literal::byte_string(seed.value().as_bytes());

        output.extend(quote! {
            pub const #seed_const: &[u8] = #seed_bytes;
        });

        // For unit variants (no fields), generate static ADDRESS and BUMP constants
        if pda.fields.is_empty() {
            let address_const =
                format_ident!("{}_ADDRESS", to_screaming_snake_case(&name.to_string()));
            let bump_const = format_ident!("{}_BUMP", to_screaming_snake_case(&name.to_string()));

            output.extend(quote! {
                /// Statically derived PDA address (computed at compile time)
                pub const #address_const: ::pinocchio::pubkey::Pubkey =
                    ::const_crypto::ed25519::derive_program_address(&[#seed_const], &crate::ID).0;
                /// Statically derived PDA bump (computed at compile time)
                pub const #bump_const: u8 =
                    ::const_crypto::ed25519::derive_program_address(&[#seed_const], &crate::ID).1;
            });
        }
    }

    // Generate finder functions
    for pda in &pda_defs {
        let name = &pda.name;
        let fields = &pda.fields;

        // Generate UPPER_SEED constant name
        let seed_const = format_ident!("{}_SEED", to_screaming_snake_case(&name.to_string()));

        // Generate find_lower_pda function name
        let find_fn = format_ident!("find_{}_pda", to_snake_case(&name.to_string()));

        // Generate the finder function with typed parameters
        // For u64 fields, we take the value directly and call .to_le_bytes()
        // For other fields, we take a reference and call .as_ref()
        let find_params: Vec<_> = fields
            .iter()
            .map(|f| {
                let name = &f.name;
                let ty = &f.ty;
                if is_u64_type(ty) {
                    quote! { #name: #ty }
                } else {
                    quote! { #name: &#ty }
                }
            })
            .collect();

        // For u64 fields, we need to convert to bytes first
        // This requires creating local variables for the byte arrays
        let u64_conversions: Vec<_> = fields
            .iter()
            .filter(|f| is_u64_type(&f.ty))
            .map(|f| {
                let name = &f.name;
                let bytes_name = format_ident!("{}_bytes", name);
                quote! { let #bytes_name = #name.to_le_bytes(); }
            })
            .collect();

        let find_refs: Vec<_> = fields
            .iter()
            .map(|f| {
                let name = &f.name;
                if is_u64_type(&f.ty) {
                    let bytes_name = format_ident!("{}_bytes", name);
                    quote! { #bytes_name.as_ref() }
                } else {
                    quote! { #name.as_ref() }
                }
            })
            .collect();

        output.extend(quote! {
            #[inline]
            pub fn #find_fn(#(#find_params),*) -> (::pinocchio::pubkey::Pubkey, u8) {
                #(#u64_conversions)*
                ::pinocchio::pubkey::find_program_address(
                    &[#seed_const, #(#find_refs),*],
                    &crate::ID
                )
            }
        });
    }

    // Generate seed generator functions
    for pda in &pda_defs {
        let name = &pda.name;
        let fields = &pda.fields;

        let seed_const = format_ident!("{}_SEED", to_screaming_snake_case(&name.to_string()));

        // Generate the seeds function with typed parameters
        // Parameters need explicit 'a lifetime for Seed to work
        // For u64 fields, we take &'a [u8; 8] (the caller converts with .to_le_bytes())
        let gen_fn = format_ident!("gen_{}_seeds", to_snake_case(&name.to_string()));
        let seed_count = fields.len() + 2; // seed constant + fields + bump

        let gen_params: Vec<_> = fields
            .iter()
            .map(|f| {
                let name = &f.name;
                let ty = &f.ty;
                if is_u64_type(ty) {
                    // For u64 fields, take a reference to the byte array
                    quote! { #name: &'a [u8; 8] }
                } else {
                    quote! { #name: &'a #ty }
                }
            })
            .collect();
        let gen_refs: Vec<_> = fields
            .iter()
            .map(|f| {
                let name = &f.name;
                quote! { ::pinocchio::instruction::Seed::from(#name.as_ref()) }
            })
            .collect();

        output.extend(quote! {
            #[inline]
            pub fn #gen_fn<'a>(#(#gen_params,)* bump: &'a [u8]) -> [::pinocchio::instruction::Seed<'a>; #seed_count] {
                [::pinocchio::instruction::Seed::from(#seed_const), #(#gen_refs,)* ::pinocchio::instruction::Seed::from(bump)]
            }
        });
    }

    // Generate IDL build test (only when idl-build feature is enabled)
    let idl_tests = generate_idl_tests(&pda_defs);
    output.extend(quote! {
        #[cfg(feature = "idl-build")]
        #idl_tests
    });

    output
}

/// Generate IDL test functions for PDA definitions
fn generate_idl_tests(pda_defs: &[PdaDef]) -> TokenStream2 {
    let mut test_fns = TokenStream2::new();

    for pda in pda_defs {
        let name = &pda.name;
        let seed = &pda.seed;
        let test_name = format_ident!("__idl_build_pda_{}", to_snake_case(&name.to_string()));

        // Build documentation string describing the seeds
        let seed_str = seed.value();
        let field_names: Vec<String> = pda.fields.iter().map(|f| f.name.to_string()).collect();

        // Build docs: if user provided docs, use those; otherwise generate from seeds
        let docs: Vec<String> = if pda.docs.is_empty() {
            let seeds_desc = if field_names.is_empty() {
                format!("Seeds: [\"{seed_str}\"]")
            } else {
                format!("Seeds: [\"{}\", {}]", seed_str, field_names.join(", "))
            };
            vec![seeds_desc]
        } else {
            pda.docs.clone()
        };

        // Build seeds JSON array
        // First seed is always a const (the seed string)
        let seed_bytes: Vec<u8> = seed_str.as_bytes().to_vec();
        let seed_bytes_str = format!("{seed_bytes:?}");

        // Build field seeds as "account" type seeds
        let field_seeds: Vec<String> = pda
            .fields
            .iter()
            .map(|f| {
                let field_name = f.name.to_string();
                format!(r#"{{"kind":"account","path":"{field_name}"}}"#)
            })
            .collect();

        let all_seeds = std::iter::once(format!(r#"{{"kind":"const","value":{seed_bytes_str}}}"#))
            .chain(field_seeds)
            .collect::<Vec<_>>()
            .join(",");

        let docs_json = docs
            .iter()
            .map(|d| format!("\"{}\"", d.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",");

        let name_str = name.to_string();

        test_fns.extend(quote! {
            #[test]
            fn #test_name() {
                println!("--- IDL pda {} ---", #name_str);
                println!(r#"{{"name":"{}","docs":[{}],"seeds":[{}]}}"#, #name_str, #docs_json, #all_seeds);
                println!("--- end ---");
            }
        });
    }

    test_fns
}
