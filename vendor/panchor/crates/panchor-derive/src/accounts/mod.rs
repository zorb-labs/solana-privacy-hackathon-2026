//! Accounts derive macro
//!
//! Generates zero-cost account validation for instruction handlers.
//!
//! ## Typed Accounts
//!
//! Fields can use typed wrappers for automatic validation:
//! - `AccountLoader<'info, T>` - validates owner, discriminator, and size for mutable program accounts
//! - `LazyAccount<'info, T>` - validates owner/discriminator at construction, deserializes on demand (immutable)
//! - `Signer<'info>` - validates the account is a signer
//! - `Program<'info, T>` - validates executable and program ID
//! - `&'info AccountInfo` - raw reference with manual validation via `#[account(...)]`

mod constraints;
mod field_kind;
mod metadata;
mod pda;
mod validation;

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Error, Fields, Result};

use crate::utils::extract_docs;

use constraints::parse_field_constraints;
use field_kind::{FieldKind, detect_field_kind};
use metadata::{AccountMeta, generate_idl_build_test, generate_input_struct};
use validation::generate_field_validation_and_conversion;

/// Core implementation for Accounts derive macro
pub fn derive_accounts_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;

    // Extract the lifetime from generics (expect 'info)
    // We keep the original lifetime with its span to preserve hygiene
    let Some(lifetime) = input
        .generics
        .lifetimes()
        .next()
        .map(|lt| lt.lifetime.clone())
    else {
        return Error::new_spanned(
            &input.ident,
            "Accounts requires a lifetime parameter, e.g., struct Accounts<'info>",
        )
        .to_compile_error();
    };

    // Get the struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Error::new_spanned(
                    &input.ident,
                    "Accounts only supports structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return Error::new_spanned(&input.ident, "Accounts only supports structs")
                .to_compile_error();
        }
    };

    let all_fields: Vec<_> = fields.iter().collect();
    let num_accounts = all_fields.len();
    let field_names: Vec<_> = all_fields
        .iter()
        .map(|f| f.ident.as_ref().unwrap())
        .collect();

    // Detect field kinds
    let field_kinds: Vec<_> = all_fields.iter().map(|f| detect_field_kind(f)).collect();

    // Parse constraints for all fields
    let constraints: Vec<_> = match all_fields
        .iter()
        .map(|f| parse_field_constraints(f))
        .collect::<Result<Vec<_>>>()
    {
        Ok(c) => c,
        Err(e) => return e.to_compile_error(),
    };

    // Check if any field has init constraint
    let has_init = constraints.iter().any(|c| c.init);

    // If any field has init, ensure system_program exists
    if has_init {
        let has_system_program = field_names.iter().any(|name| *name == "system_program");

        if !has_system_program {
            return Error::new_spanned(
                &input.ident,
                "When using `init` constraint, a `system_program` field is required for account creation",
            )
            .to_compile_error();
        }
    }

    // Collect account metadata for SDK generation
    // Merge field kind info with constraints for metadata
    let account_metas: Vec<AccountMeta> = all_fields
        .iter()
        .zip(constraints.iter())
        .zip(field_kinds.iter())
        .map(|((f, c), kind)| {
            let is_signer = c.signer || matches!(kind, FieldKind::Signer);

            // Determine the type to use for Id trait (for IDL address generation)
            // - For Program<'info, T>, always use T::ID
            // - For AccountLoader/LazyAccount with #[account(id)], use the type
            let id_type = match kind {
                FieldKind::Program(path) => Some(path.clone()),
                FieldKind::AccountLoader(path) | FieldKind::LazyAccount(path) if c.id => {
                    Some(path.clone())
                }
                _ => None,
            };

            let docs = extract_docs(&f.attrs);
            AccountMeta {
                name: f.ident.clone().unwrap(),
                doc: if docs.is_empty() {
                    None
                } else {
                    Some(docs.join(" "))
                },
                signer: is_signer,
                mutable: c.mutable || c.init || c.init_idempotent, // init/init_idempotent implies writable
                program_expr: c.program.clone(),
                address_expr: c.address.clone(),
                id_type,
            }
        })
        .collect();

    // Get field types for explicit type annotations
    let field_types: Vec<_> = all_fields.iter().map(|f| &f.ty).collect();

    // Collect fields that have pda constraints for bump generation
    // Exclude fields with skip_pda_derivation (those only use pda for IDL generation)
    let bump_fields: Vec<_> = field_names
        .iter()
        .zip(constraints.iter())
        .filter_map(|(name, c)| {
            if c.pda.is_some() && !c.skip_pda_derivation {
                Some(*name)
            } else {
                None
            }
        })
        .collect();

    // Collect fields that have init_idempotent constraint for early return check
    let init_idempotent_fields: Vec<_> = field_names
        .iter()
        .zip(constraints.iter())
        .filter_map(
            |(name, c)| {
                if c.init_idempotent { Some(*name) } else { None }
            },
        )
        .collect();

    // Generate validation and conversion code for each field
    let field_validations: Vec<_> = field_names
        .iter()
        .zip(field_types.iter())
        .zip(field_kinds.iter())
        .zip(constraints.iter())
        .map(|(((name, ty), kind), c)| {
            generate_field_validation_and_conversion(name, ty, kind, c, &field_names)
        })
        .collect();

    // Generate {Name}Input struct for SDK use (behind cfg)
    let input_struct = generate_input_struct(name, &account_metas);

    // Generate IDL build test (only when idl-build feature is enabled)
    let idl_build_test = generate_idl_build_test(name, &account_metas);

    // Generate slice pattern for destructuring
    let slice_pattern: Vec<_> = field_names.iter().map(|n| quote! { #n }).collect();

    // Generate Bumps struct name
    let name_str = name.to_string();
    let bumps_name = if let Some(stripped) = name_str.strip_suffix("Accounts") {
        format_ident!("{}Bumps", stripped)
    } else {
        format_ident!("{}Bumps", name_str)
    };

    // Generate the Bumps struct and trait impl
    let bumps_struct = if bump_fields.is_empty() {
        // No PDA fields, generate empty unit struct
        quote! {
            /// Bump seeds for PDA accounts (none for this instruction)
            #[derive(Debug, Default, Clone, Copy)]
            pub struct #bumps_name;

            impl ::panchor::Bumps for #name<'_> {
                type Bumps = #bumps_name;
            }
        }
    } else {
        // Generate struct with bump fields
        let bump_field_defs: Vec<_> = bump_fields
            .iter()
            .map(|name| quote! { pub #name: u8 })
            .collect();

        let bump_field_defaults: Vec<_> =
            bump_fields.iter().map(|name| quote! { #name: 0 }).collect();

        quote! {
            /// Bump seeds for PDA accounts
            #[derive(Debug, Clone, Copy)]
            pub struct #bumps_name {
                #(#bump_field_defs),*
            }

            impl Default for #bumps_name {
                fn default() -> Self {
                    Self {
                        #(#bump_field_defaults),*
                    }
                }
            }

            impl ::panchor::Bumps for #name<'_> {
                type Bumps = #bumps_name;
            }
        }
    };

    // Generate try_into_context method
    // This requires recalculating bumps and validating accounts in a way that captures the bumps
    // Skip fields with skip_pda_derivation (those only use pda for IDL generation)
    let pda_bump_calcs: Vec<_> = field_names
        .iter()
        .zip(constraints.iter())
        .filter_map(|(name, c)| {
            if let Some(pda) = &c.pda
                && !c.skip_pda_derivation
            {
                let variant = &pda.variant;
                let find_fn = format_ident!(
                    "find_{}_pda",
                    crate::utils::to_snake_case(&variant.to_string())
                );
                let find_args: Vec<_> = pda
                    .fields
                    .iter()
                    .map(|(_, expr)| quote! { #expr })
                    .collect();
                let bump_var = format_ident!("__bump_{}", name);
                Some(quote! {
                    let #bump_var = {
                        let (_, bump) = crate::pda::#find_fn(#(#find_args),*);
                        bump
                    };
                })
            } else {
                None
            }
        })
        .collect();

    let bump_struct_fields: Vec<_> = bump_fields
        .iter()
        .map(|name| {
            let bump_var = format_ident!("__bump_{}", name);
            quote! { #name: #bump_var }
        })
        .collect();

    // Generate init_idempotent early return check code
    let idempotent_early_return_checks: Vec<_> = init_idempotent_fields
        .iter()
        .map(|name| {
            let name_str = name.to_string();
            let log_msg = format!("{name_str} already exists");
            quote! {
                if !#name.data_is_empty() {
                    ::panchor::pinocchio_log::log!(#log_msg);
                    return Ok(::panchor::ParseResult::SkipIdempotent);
                }
            }
        })
        .collect();

    // Generate try_into_context method
    // Returns ParseResult - SkipIdempotent means init_idempotent account already exists (skip instruction)
    let try_into_context_impl = quote! {
        impl<#lifetime> #name<#lifetime> {
            /// Parse accounts and create a Parsed with bump seeds and remaining accounts.
            ///
            /// This method validates all accounts and derives PDA bump seeds,
            /// returning a Parsed that owns the accounts. Use `.as_context()` to get
            /// a Context reference for passing to handlers.
            ///
            /// Returns `ParseResult::SkipIdempotent` if an `init_idempotent` account already exists,
            /// signaling the instruction should return early without processing.
            #[inline]
            pub fn try_into_context(
                accounts: &#lifetime [::panchor::pinocchio::account_info::AccountInfo],
            ) -> ::core::result::Result<::panchor::ParseResult<#lifetime, Self>, ::panchor::pinocchio::program_error::ProgramError> {
                // Destructure the slice
                let [#(#slice_pattern,)* remaining @ ..] = accounts else {
                    return Err(::panchor::pinocchio::program_error::ProgramError::NotEnoughAccountKeys);
                };

                // Check init_idempotent accounts for early return (before any creation)
                #(#idempotent_early_return_checks)*

                // Calculate PDA bumps
                #(#pda_bump_calcs)*

                // Validate and convert each field
                #(#field_validations)*

                // Create bumps struct
                let bumps = #bumps_name {
                    #(#bump_struct_fields),*
                };

                // Create accounts struct
                let accounts = Self {
                    #(#field_names),*
                };

                Ok(::panchor::ParseResult::Parsed(::panchor::Parsed::new(accounts, bumps, remaining)))
            }
        }
    };

    // Note: TryFrom<&[AccountInfo]> is intentionally not generated.
    // All account parsing goes through try_into_context which also provides
    // bumps and remaining accounts. TryFrom would duplicate the validation
    // code and increase binary size without being used.

    quote! {
        impl<#lifetime> #name<#lifetime> {
            /// Number of accounts expected by this instruction.
            pub const LEN: usize = #num_accounts;
        }

        #bumps_struct

        #try_into_context_impl

        #input_struct

        #idl_build_test
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_and_expand(input: TokenStream2) -> TokenStream2 {
        let input = syn::parse2::<DeriveInput>(input).unwrap();
        derive_accounts_impl(input)
    }

    #[test]
    fn test_basic_accounts() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(signer)]
                pub signer: &'info AccountInfo,
                #[account(mut)]
                pub target: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check for try_into_context (TryFrom is not generated to reduce binary size)
        assert!(output_str.contains("try_into_context"));
        // Check for assert_signer_no_trace and assert_writable_no_trace
        assert!(output_str.contains("assert_signer_no_trace"));
        assert!(output_str.contains("assert_writable_no_trace"));
        // Check for inspect_err usage
        assert!(output_str.contains("inspect_err"));
    }

    #[test]
    fn test_type_constraint() {
        // Test that AccountLoader<'info, Mine> uses try_into_context for validation
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub mine: AccountLoader<'info, Mine>,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // AccountLoader handles validation via try_into_context
        assert!(output_str.contains("try_into_context"));
        assert!(output_str.contains("Mine"));
    }

    #[test]
    fn test_program_constraint() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(program = &SYSTEM_PROGRAM_ID)]
                pub system_program: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check for assert_program_no_trace call with the program ID
        assert!(output_str.contains("assert_program_no_trace"));
        assert!(output_str.contains("SYSTEM_PROGRAM_ID"));
    }

    #[test]
    fn test_multiple_constraints() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(signer, mut)]
                pub payer: &'info AccountInfo,
                #[account(mut)]
                pub mine: AccountLoader<'info, Mine>,
                pub readonly: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check all explicit checks are present using no_trace methods
        assert!(
            output_str.contains("assert_signer_no_trace"),
            "Missing assert_signer_no_trace. Output:\n{}",
            output_str
        );
        assert!(output_str.contains("assert_writable_no_trace"));
        // AccountLoader handles owner check via try_into_context
        assert!(output_str.contains("try_into_context"));
    }

    #[test]
    fn test_const_len() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                pub a: &'info AccountInfo,
                pub b: &'info AccountInfo,
                pub c: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        assert!(output_str.contains("const LEN : usize = 3"));
    }

    #[test]
    fn test_input_struct_generation() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(signer)]
                pub signer: &'info AccountInfo,
                #[account(mut)]
                pub target: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check that Input struct is generated for SDK use (behind cfg(feature = "solana-sdk"))
        assert!(output_str.contains("cfg (feature = \"solana-sdk\")"));
        assert!(output_str.contains("TestInput")); // TestAccounts -> TestInput
        assert!(output_str.contains("to_account_metas"));
    }

    #[test]
    fn test_slice_pattern_destructuring() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                pub a: &'info AccountInfo,
                pub b: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check for slice pattern destructuring (no __raw_ prefix)
        // try_into_context uses `let [a , b , remaining @ ..]`
        assert!(output_str.contains("let [a , b , remaining @ ..]"));
        assert!(!output_str.contains("__raw_"));
    }

    #[test]
    fn test_inspect_err_per_field() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(signer)]
                pub payer: &'info AccountInfo,
                #[account(mut)]
                pub target: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check that each field has its own inspect_err with field name
        assert!(output_str.contains("inspect_err"));
        assert!(output_str.contains("\"payer\""));
        assert!(output_str.contains("\"target\""));
    }

    #[test]
    fn test_idl_build_method_generation() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(program = &SYSTEM_PROGRAM_ID)]
                pub system_program: &'info AccountInfo,
                #[account(address = &GLOBAL_STATE_ADDRESS)]
                pub global_state: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Check that IDL build method is generated
        assert!(output_str.contains("__idl_instruction_accounts"));
        assert!(output_str.contains("cfg (feature = \"idl-build\")"));
        assert!(output_str.contains("IdlInstructionAccount"));
        assert!(output_str.contains("pubkey_to_base58"));
    }

    #[test]
    fn test_account_loader_wrapper() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub mine: AccountLoader<'info, Mine>,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should use TryFrom for AccountLoader<T>
        assert!(output_str.contains("try_from"));
    }

    #[test]
    fn test_signer_wrapper() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub payer: Signer<'info>,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should use TryFrom for Signer
        assert!(output_str.contains("try_from"));
        // Should still have writable check for mut using no_trace method
        assert!(output_str.contains("assert_writable_no_trace"));
    }

    #[test]
    fn test_init_constraint() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(signer, mut)]
                pub payer: &'info AccountInfo,
                #[account(init, seeds = [b"mine", slug], payer = payer)]
                pub mine: AccountLoader<'info, Mine>,
                #[account(program = &SYSTEM_PROGRAM_ID)]
                pub system_program: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should generate PDA creation code
        assert!(output_str.contains("create_account_with_pda"));
        assert!(output_str.contains("find_program_address"));
        // Should check writable using no_trace method
        assert!(output_str.contains("assert_writable_no_trace"));
    }

    #[test]
    fn test_init_with_bump() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub payer: Signer<'info>,
                #[account(init, seeds = [b"mine", slug], payer = payer, bump = mine_bump)]
                pub mine: AccountLoader<'info, Mine>,
                pub system_program: Program<'info, System>,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should use provided bump instead of find_program_address
        assert!(output_str.contains("create_account_with_pda"));
        assert!(output_str.contains("mine_bump"));
        // Should NOT derive bump since it's provided
        assert!(!output_str.contains("find_program_address"));
    }

    #[test]
    fn test_id_constraint() {
        // The `id` constraint checks that the account key matches T::ID
        // Works with AccountLoader<'info, T> where T implements Id trait
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(id)]
                pub global_state: AccountLoader<'info, GlobalState>,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should check address matches Id trait using assert_key_no_trace
        assert!(output_str.contains("assert_key_no_trace"));
        assert!(output_str.contains("Id"));
        assert!(output_str.contains("GlobalState"));
    }

    #[test]
    fn test_init_requires_system_program() {
        // The `init` constraint requires a system_program field for account creation
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub payer: Signer<'info>,
                #[account(init, seeds = [b"mine"], payer = payer)]
                pub mine: AccountLoader<'info, Mine>,
                // Missing system_program!
            }
        };

        let input_parsed = syn::parse2::<DeriveInput>(input).unwrap();
        let output = derive_accounts_impl(input_parsed);
        let output_str = output.to_string();

        // Should produce an error about missing system_program
        assert!(output_str.contains("system_program"));
        assert!(output_str.contains("required"));
    }

    #[test]
    fn test_init_mut_mutually_exclusive() {
        // The `init` and `mut` constraints are mutually exclusive
        // (init implies writable, so mut is redundant)
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub payer: Signer<'info>,
                #[account(init, mut, seeds = [b"mine"], payer = payer)]
                pub mine: AccountLoader<'info, Mine>,
                pub system_program: Program<'info, System>,
            }
        };

        let input_parsed = syn::parse2::<DeriveInput>(input).unwrap();
        let output = derive_accounts_impl(input_parsed);
        let output_str = output.to_string();

        // Should produce an error about init and mut being mutually exclusive
        assert!(output_str.contains("mutually exclusive"));
    }

    #[test]
    fn test_pda_constraint_validation() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                pub mine: AccountLoader<'info, Mine>,
                #[account(pda = Miner, pda::mine = mine.key(), pda::authority = authority.key())]
                pub miner: &'info AccountInfo,
                pub authority: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should generate find_miner_pda call
        assert!(output_str.contains("find_miner_pda"));
        // Should generate assert check
        assert!(output_str.contains("assert_key_derived_from_seeds_no_trace"));
    }

    #[test]
    fn test_pda_constraint_init() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub payer: Signer<'info>,
                pub mine: AccountLoader<'info, Mine>,
                #[account(init, pda = Miner, pda::mine = mine.key(), pda::authority = payer.key(), payer = payer)]
                pub miner: AccountLoader<'info, Miner>,
                pub system_program: Program<'info, System>,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should generate find_miner_pda call
        assert!(output_str.contains("find_miner_pda"));
        // Should generate MINER_SEED reference
        assert!(output_str.contains("MINER_SEED"));
        // Should generate create_account_with_pda call
        assert!(output_str.contains("create_account_with_pda"));
    }

    #[test]
    fn test_pda_and_seeds_mutually_exclusive() {
        let input = quote! {
            pub struct TestAccounts<'info> {
                #[account(mut)]
                pub payer: Signer<'info>,
                #[account(init, pda = Miner, pda::mine = mine.key(), seeds = [b"miner"], payer = payer)]
                pub miner: AccountLoader<'info, Miner>,
                pub system_program: Program<'info, System>,
            }
        };

        let input_parsed = syn::parse2::<DeriveInput>(input).unwrap();
        let output = derive_accounts_impl(input_parsed);
        let output_str = output.to_string();

        // Should produce an error about seeds and pda being mutually exclusive
        assert!(output_str.contains("mutually exclusive"));
    }

    #[test]
    fn test_skip_pda_derivation_no_bump() {
        // When skip_pda_derivation is set, the bump should NOT be added to the bumps struct
        // and no PDA validation should be generated. The pda constraint is only for IDL.
        let input = quote! {
            pub struct TestAccounts<'info> {
                pub mine: AccountLoader<'info, Mine>,
                #[account(mut, pda = Stake, pda::mine = mine.key(), pda::authority = authority.key(), skip_pda_derivation)]
                pub stake: &'info AccountInfo,
                pub authority: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should NOT generate find_stake_pda call (no derivation)
        assert!(
            !output_str.contains("find_stake_pda"),
            "skip_pda_derivation should not generate find_*_pda call"
        );

        // Should NOT have stake in bumps struct - should be unit struct
        assert!(
            output_str.contains("pub struct TestBumps ;"),
            "Bumps struct should be empty unit struct when skip_pda_derivation is used. Output:\n{}",
            output_str
        );

        // Should NOT generate bump variable for stake
        assert!(
            !output_str.contains("__bump_stake"),
            "skip_pda_derivation should not generate bump variable"
        );
    }

    #[test]
    fn test_skip_pda_derivation_mixed_with_regular_pda() {
        // When one field has skip_pda_derivation and another has regular pda,
        // only the regular pda field should have bump in the struct
        let input = quote! {
            pub struct TestAccounts<'info> {
                pub mine: AccountLoader<'info, Mine>,
                #[account(pda = Miner, pda::mine = mine.key(), pda::authority = authority.key())]
                pub miner: &'info AccountInfo,
                #[account(mut, pda = Stake, pda::mine = mine.key(), pda::authority = authority.key(), skip_pda_derivation)]
                pub stake: &'info AccountInfo,
                pub authority: &'info AccountInfo,
            }
        };

        let output = parse_and_expand(input);
        let output_str = output.to_string();

        // Should generate find_miner_pda (regular pda validation)
        assert!(
            output_str.contains("find_miner_pda"),
            "Regular pda should generate find_*_pda call"
        );

        // Should NOT generate find_stake_pda (skip_pda_derivation)
        assert!(
            !output_str.contains("find_stake_pda"),
            "skip_pda_derivation should not generate find_*_pda call"
        );

        // Bumps struct should have miner but NOT stake
        assert!(
            output_str.contains("pub miner : u8"),
            "Bumps struct should have miner field"
        );
        assert!(
            !output_str.contains("pub stake : u8"),
            "Bumps struct should NOT have stake field when skip_pda_derivation is used"
        );
    }
}
