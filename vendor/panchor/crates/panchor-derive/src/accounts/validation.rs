//! Validation and conversion code generation for the Accounts derive macro.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Ident, Path, Type};

use super::constraints::AccountConstraints;
use super::field_kind::{FieldKind, get_account_type};
use crate::utils::to_snake_case;

/// Generated code components for PDA initialization
struct PdaInitCode {
    /// Code to derive the bump (and validate PDA address for pda-based init)
    bump_derivation: TokenStream2,
    /// Code to build signer seeds array
    signer_seeds: TokenStream2,
}

/// Generate bump derivation and signer seeds for PDA-based init (using `pda` constraint)
fn generate_pda_based_init(
    field_name: &Ident,
    constraints: &AccountConstraints,
) -> Option<PdaInitCode> {
    let pda = constraints.pda.as_ref()?;
    let variant = &pda.variant;
    let find_fn = format_ident!("find_{}_pda", to_snake_case(&variant.to_string()));

    // Generate arguments for finder function
    let find_args: Vec<_> = pda
        .fields
        .iter()
        .map(|(_, expr)| quote! { #expr })
        .collect();

    // Generate inline seeds that use .as_ref() on everything
    let signer_seed_refs: Vec<_> = pda
        .fields
        .iter()
        .map(|(_, expr)| {
            quote! { ::pinocchio::instruction::Seed::from((#expr).as_ref()) }
        })
        .collect();

    let seed_const = format_ident!(
        "{}_SEED",
        crate::utils::to_screaming_snake_case(&variant.to_string())
    );
    let seed_count = pda.fields.len() + 2; // seed prefix + fields + bump

    let bump_derivation = quote! {
        let (__expected_pda, __bump) = crate::pda::#find_fn(#(#find_args),*);
        ::panchor::AccountAssertionsNoTrace::assert_key_derived_from_seeds_no_trace(#field_name, &__expected_pda)?;
    };

    let signer_seeds = quote! {
        let __bump_bytes = [__bump];
        let __signer_seeds: [::pinocchio::instruction::Seed; #seed_count] = [
            ::pinocchio::instruction::Seed::from(crate::pda::#seed_const),
            #(#signer_seed_refs,)*
            ::pinocchio::instruction::Seed::from(__bump_bytes.as_ref()),
        ];
    };

    Some(PdaInitCode {
        bump_derivation,
        signer_seeds,
    })
}

/// Generate bump derivation and signer seeds for seeds-based init (using `seeds` constraint)
fn generate_seeds_based_init(
    account_type: &Path,
    constraints: &AccountConstraints,
) -> Option<PdaInitCode> {
    let seeds = constraints.seeds.as_ref()?;

    // Bind each seed expression to a variable to extend its lifetime
    let seed_bindings: Vec<_> = seeds
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let var = format_ident!("__seed_{}", i);
            quote! { let #var = #s; }
        })
        .collect();

    let seed_vars: Vec<_> = (0..seeds.len())
        .map(|i| format_ident!("__seed_{}", i))
        .collect();

    // Build the seeds array for find_program_address (without bump)
    let seeds_for_find: Vec<_> = seed_vars
        .iter()
        .map(|v| quote! { ::core::convert::AsRef::<[u8]>::as_ref(&#v) })
        .collect();

    // Generate bump derivation code
    let bump_derivation = if let Some(ref bump_field) = constraints.bump {
        quote! {
            #(#seed_bindings)*
            let __bump = #bump_field;
        }
    } else {
        quote! {
            #(#seed_bindings)*
            let (_, __bump) = ::panchor::pinocchio::pubkey::find_program_address(
                &[#(#seeds_for_find),*],
                &<#account_type as ::panchor::ProgramOwned>::PROGRAM_ID,
            );
        }
    };

    // Build the signer seeds (seeds + bump)
    let seeds_with_bump: Vec<_> = seed_vars
        .iter()
        .map(|v| {
            quote! { ::panchor::pinocchio::instruction::Seed::from(::core::convert::AsRef::<[u8]>::as_ref(&#v)) }
        })
        .collect();

    let signer_seeds = quote! {
        let __bump_bytes = [__bump];
        let __signer_seeds: &[::panchor::pinocchio::instruction::Seed] = &[
            #(#seeds_with_bump,)*
            ::panchor::pinocchio::instruction::Seed::from(__bump_bytes.as_ref()),
        ];
    };

    Some(PdaInitCode {
        bump_derivation,
        signer_seeds,
    })
}

/// Generate the common PDA account creation code (create account, set discriminator, set bump)
fn generate_pda_creation_code(
    field_name: &Ident,
    account_type: &Path,
    payer: &Ident,
) -> TokenStream2 {
    quote! {
        ::panchor::CreatePda::create_account_with_pda::<#account_type>(
            #field_name,
            ::panchor::accounts::AsAccountInfo::account_info(&#payer),
            &__signer_seeds,
            ::panchor::accounts::AsAccountInfo::account_info(&system_program),
            __bump,
        )?;
    }
}

/// Generate the conversion code from raw `AccountInfo` to the field type
fn generate_conversion_code(field_kind: &FieldKind, field_name: &Ident) -> TokenStream2 {
    match field_kind {
        FieldKind::RawAccountInfo => {
            quote! { Ok(#field_name) }
        }
        FieldKind::AccountLoader(_) | FieldKind::LazyAccount(_) => {
            quote! { ::core::convert::TryFrom::try_from(#field_name) }
        }
        FieldKind::Signer => {
            quote! { ::core::convert::TryFrom::try_from(#field_name) }
        }
        FieldKind::Program(_) => {
            quote! { ::core::convert::TryFrom::try_from(#field_name) }
        }
    }
}

/// Generate all validation, PDA creation, and conversion code for a single field.
/// Returns code wrapped in a closure with `inspect_err` for the field name.
pub fn generate_field_validation_and_conversion(
    field_name: &Ident,
    field_type: &Type,
    field_kind: &FieldKind,
    constraints: &AccountConstraints,
    all_field_names: &[&Ident],
) -> TokenStream2 {
    let field_name_str = field_name.to_string();
    let mut checks = Vec::new();
    let mut pda_creation = None;

    // For init or init_idempotent constraint, generate PDA creation code
    let is_init_variant = constraints.init || constraints.init_idempotent;
    if is_init_variant
        && let Some(account_type) = get_account_type(field_kind)
        && let Some(payer) = &constraints.payer
    {
        // Verify payer and system_program exist
        let has_payer = all_field_names.contains(&payer);
        let has_system_program = all_field_names.iter().any(|n| *n == "system_program");

        if has_payer && has_system_program {
            // Auto-validate payer is signer (required for CPI to system program)
            checks.push(quote! {
                #payer.assert_signer_no_trace()?;
            });

            // Try PDA-based init first, then seeds-based init
            let init_code = generate_pda_based_init(field_name, constraints)
                .or_else(|| generate_seeds_based_init(account_type, constraints));

            if let Some(PdaInitCode {
                bump_derivation,
                signer_seeds,
            }) = init_code
            {
                let creation_code = generate_pda_creation_code(field_name, account_type, payer);

                if constraints.init_idempotent {
                    // For init_idempotent: derive bump first, then only create if empty
                    pda_creation = Some(quote! {
                        #bump_derivation
                        if #field_name.data_is_empty() {
                            #signer_seeds
                            #creation_code
                        }
                    });
                } else {
                    pda_creation = Some(quote! {
                        if !#field_name.data_is_empty() {
                            return Err(::panchor::pinocchio::program_error::ProgramError::AccountAlreadyInitialized);
                        }
                        #bump_derivation
                        #signer_seeds
                        #creation_code
                    });
                }
            }
        }
    }

    // For non-init accounts with pda constraint, generate PDA validation code
    // Skip if skip_pda_derivation is set
    if !constraints.init
        && !constraints.init_idempotent
        && !constraints.skip_pda_derivation
        && let Some(pda) = &constraints.pda
    {
        let variant = &pda.variant;
        let find_fn = format_ident!("find_{}_pda", to_snake_case(&variant.to_string()));

        // Generate arguments for finder function
        let find_args: Vec<_> = pda
            .fields
            .iter()
            .map(|(_, expr)| quote! { #expr })
            .collect();

        checks.push(quote! {
            {
                let (__expected_pda, _) = crate::pda::#find_fn(#(#find_args),*);
                ::panchor::AccountAssertionsNoTrace::assert_key_derived_from_seeds_no_trace(#field_name, &__expected_pda)?;
            }
        });
    }

    // Generate explicit checks using assert_*_no_trace methods
    // For typed wrappers (Signer, Program), the TryFrom handles signer/program checks
    // Only add explicit signer check for raw AccountInfo with signer constraint
    if constraints.signer {
        checks.push(quote! {
            #field_name.assert_signer_no_trace()?;
        });
    }

    // Writable check (init/init_idempotent implies writable)
    if constraints.init || constraints.init_idempotent || constraints.mutable {
        checks.push(quote! {
            #field_name.assert_writable_no_trace()?;
        });
    }

    // Program check
    if let Some(ref program_expr) = constraints.program {
        checks.push(quote! {
            #field_name.assert_program_no_trace(&#program_expr)?;
        });
    }

    // Address check
    if let Some(ref address_expr) = constraints.address {
        checks.push(quote! {
            #field_name.assert_key_no_trace(&#address_expr)?;
        });
    }

    // Owner check (custom owner expression)
    if let Some(ref owner_expr) = constraints.owner {
        checks.push(quote! {
            #field_name.assert_owner_no_trace(&#owner_expr)?;
        });
    }

    // Id constraint - check address matches T::ID from the Id trait
    if constraints.id
        && let Some(account_type) = get_account_type(field_kind)
    {
        checks.push(quote! {
            #field_name.assert_key_no_trace(&<#account_type as ::panchor::Id>::ID)?;
        });
    }

    // Executable check (for program accounts without typed wrapper)
    if constraints.exec {
        checks.push(quote! {
            #field_name.assert_executable_no_trace()?;
        });
    }

    // Empty/zero check (account must have no data)
    if constraints.zero {
        checks.push(quote! {
            #field_name.assert_empty_no_trace()?;
        });
    }

    // Generate conversion code
    let conversion = generate_conversion_code(field_kind, field_name);

    // Wrap everything in a closure with inspect_err
    let pda_code = pda_creation.unwrap_or_default();

    quote! {
        let #field_name: #field_type = (|| -> ::core::result::Result<_, ::panchor::pinocchio::program_error::ProgramError> {
            #pda_code
            #(#checks)*
            #conversion
        })().inspect_err(|_| {
            ::panchor::log_account_validation_error(#field_name_str);
        })?;
    }
}
