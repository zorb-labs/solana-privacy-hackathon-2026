//! `FindProgramAddress` derive macro
//!
//! Generates the `FindProgramAddress` trait implementation for PDA structs and enums.
//! Use `#[seeds("prefix")]` to specify the seed prefix.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error, Fields, Ident, LitStr, Type};

use crate::utils::{extract_seeds_attr, is_u64_type, to_screaming_snake_case};

/// PDA variant info for enum code generation (used by pdas macro)
pub struct PdaVariant {
    pub name: Ident,
    pub seed: LitStr,
    pub fields: Vec<PdaField>,
}

/// Field info for PDA variants
pub struct PdaField {
    pub name: Ident,
    pub ty: Type,
}

/// Field info for code generation
struct FieldInfo {
    name: syn::Ident,
    ty: Type,
}

/// Implementation of derive(FindProgramAddress)
pub fn derive_find_program_address_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;

    match &input.data {
        Data::Enum(data_enum) => {
            // For enums, generate the enum implementation
            derive_find_program_address_enum(name, data_enum)
        }
        Data::Struct(data) => {
            // For structs, generate the struct implementation
            derive_find_program_address_struct(&input, name, data)
        }
        Data::Union(_) => {
            Error::new_spanned(&input, "FindProgramAddress cannot be derived for unions")
                .to_compile_error()
        }
    }
}

/// Generate `FindProgramAddress` implementation for an enum
fn derive_find_program_address_enum(name: &Ident, data_enum: &syn::DataEnum) -> TokenStream2 {
    // Extract PDA variants from the enum
    let mut variants: Vec<PdaVariant> = Vec::new();

    for variant in &data_enum.variants {
        let variant_name = variant.ident.clone();

        // Extract #[seeds("...")] attribute
        let Some(seed) = extract_seeds_attr(&variant.attrs) else {
            return Error::new_spanned(
                variant,
                format!(
                    "FindProgramAddress enum variant {variant_name} is missing #[seeds(\"...\")]"
                ),
            )
            .to_compile_error();
        };

        // Extract fields
        let fields: Vec<PdaField> = match &variant.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .filter_map(|f| {
                    f.ident.as_ref().map(|name| PdaField {
                        name: name.clone(),
                        ty: f.ty.clone(),
                    })
                })
                .collect(),
            Fields::Unit => vec![],
            Fields::Unnamed(_) => {
                return Error::new_spanned(
                    variant,
                    "FindProgramAddress enum variants must use named fields: Pool { mint: Pubkey }",
                )
                .to_compile_error();
            }
        };

        variants.push(PdaVariant {
            name: variant_name,
            seed,
            fields,
        });
    }

    generate_find_program_address_for_enum(name, &variants)
}

/// Generate `FindProgramAddress` implementation for a struct
fn derive_find_program_address_struct(
    input: &DeriveInput,
    name: &Ident,
    data: &syn::DataStruct,
) -> TokenStream2 {
    // Verify #[seeds("prefix")] attribute exists
    if extract_seeds_attr(&input.attrs).is_none() {
        return Error::new_spanned(
            input,
            "FindProgramAddress derive requires #[seeds(\"prefix\")] attribute",
        )
        .to_compile_error();
    }

    // Get struct fields
    let fields: Vec<FieldInfo> = match &data.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|f| {
                f.ident.as_ref().map(|name| FieldInfo {
                    name: name.clone(),
                    ty: f.ty.clone(),
                })
            })
            .collect(),
        Fields::Unit => vec![],
        Fields::Unnamed(_) => {
            return Error::new_spanned(
                input,
                "FindProgramAddress only supports structs with named fields or unit structs",
            )
            .to_compile_error();
        }
    };

    // Generate SEED_PREFIX constant name (refers to existing constant from pdas macro)
    let seed_const_name = format_ident!("{}_SEED", to_screaming_snake_case(&name.to_string()));

    // For u64 fields, we need to convert to bytes first
    let u64_conversions: Vec<_> = fields
        .iter()
        .filter(|f| is_u64_type(&f.ty))
        .map(|f| {
            let name = &f.name;
            let bytes_name = format_ident!("{}_bytes", name);
            quote! { let #bytes_name = self.#name.to_le_bytes(); }
        })
        .collect();

    let seed_refs: Vec<_> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            if is_u64_type(&f.ty) {
                let bytes_name = format_ident!("{}_bytes", name);
                quote! { #bytes_name.as_ref() }
            } else {
                quote! { self.#name.as_ref() }
            }
        })
        .collect();

    // Check if we have u64 fields (affects to_signer_seeds generation)
    let has_u64_fields = fields.iter().any(|f| is_u64_type(&f.ty));
    let seed_count = fields.len() + 2; // seed prefix + fields + bump

    // Generate to_signer_seeds as an inherent method (not part of the trait)
    // For PDAs with u64 fields, we can't generate it because u64 byte conversion
    // creates local variables that can't be returned. Use gen_*_seeds macros instead.
    let to_signer_seeds_impl = if has_u64_fields {
        quote! {}
    } else {
        let all_seed_refs: Vec<_> = fields
            .iter()
            .map(|f| {
                let name = &f.name;
                quote! { ::pinocchio::instruction::Seed::from(self.#name.as_ref()) }
            })
            .collect();

        quote! {
            impl #name {
                /// Generate signer seeds for CPI invocations.
                #[inline]
                pub fn to_signer_seeds<'a>(&'a self, bump: &'a [u8; 1]) -> ::panchor::SignerSeeds<'a, #seed_count> {
                    ::panchor::SignerSeeds::new([
                        ::pinocchio::instruction::Seed::from(#seed_const_name),
                        #(#all_seed_refs,)*
                        ::pinocchio::instruction::Seed::from(bump.as_ref())
                    ])
                }
            }
        }
    };

    // Note: The seed constant is generated by the #[pdas] macro, not here.
    // This derive generates both the trait impl and the to_signer_seeds inherent method.
    quote! {
        impl ::panchor::FindProgramAddress for #name {
            fn find_program_address(&self, program_id: &::pinocchio::pubkey::Pubkey) -> (::pinocchio::pubkey::Pubkey, u8) {
                #(#u64_conversions)*
                ::pinocchio::pubkey::find_program_address(
                    &[#seed_const_name, #(#seed_refs),*],
                    program_id
                )
            }
        }

        #to_signer_seeds_impl
    }
}

/// Generate `FindProgramAddress` implementation for a PDA enum.
///
/// This is called by the `#[pdas]` macro to generate the trait impl on the enum.
pub fn generate_find_program_address_for_enum(
    enum_name: &Ident,
    variants: &[PdaVariant],
) -> TokenStream2 {
    let match_arms: Vec<_> = variants
        .iter()
        .map(|pda| {
            let name = &pda.name;
            let seed = &pda.seed;
            let fields = &pda.fields;

            let field_names: Vec<_> = fields.iter().map(|f| &f.name).collect();

            // For u64 fields, we need to convert to bytes first
            let u64_conversions: Vec<_> = fields
                .iter()
                .filter(|f| is_u64_type(&f.ty))
                .map(|f| {
                    let name = &f.name;
                    let bytes_name = format_ident!("{}_bytes", name);
                    quote! { let #bytes_name = #name.to_le_bytes(); }
                })
                .collect();

            let seed_refs: Vec<_> = fields
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

            let pattern = if field_names.is_empty() {
                quote! { Self::#name }
            } else {
                quote! { Self::#name { #(#field_names),* } }
            };

            quote! {
                #pattern => {
                    #(#u64_conversions)*
                    ::pinocchio::pubkey::find_program_address(
                        &[#seed.as_bytes(), #(#seed_refs),*],
                        program_id
                    )
                }
            }
        })
        .collect();

    quote! {
        impl ::panchor::FindProgramAddress for #enum_name {
            fn find_program_address(&self, program_id: &::pinocchio::pubkey::Pubkey) -> (::pinocchio::pubkey::Pubkey, u8) {
                match self {
                    #(#match_arms),*
                }
            }
        }
    }
}
