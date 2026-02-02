//! Account attribute macro

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{DeriveInput, Expr, Ident, Path, Token, parse::Parse, parse::ParseStream, parse_quote};

use crate::utils::{extract_docs, to_snake_case};
use crate::zero_copy::zero_copy_impl;

/// PDA specification parsed from `pda = MinesPdas::Miner { mine, authority }`
#[derive(Clone)]
pub struct PdaSpec {
    /// The PDA enum type (e.g., `MinesPdas`)
    pda_type: Ident,
    /// The variant name (e.g., Miner)
    variant: Ident,
    /// Field names for the variant (empty for unit variants)
    fields: Vec<Ident>,
}

impl Parse for PdaSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse Path::Variant or Path::Variant { field1, field2, ... }
        let path: Path = input.parse()?;

        let segments: Vec<_> = path.segments.iter().collect();
        if segments.len() != 2 {
            return Err(syn::Error::new_spanned(
                &path,
                "expected PdaType::Variant syntax (e.g., MinesPdas::Miner)",
            ));
        }

        let pda_type = segments[0].ident.clone();
        let variant = segments[1].ident.clone();

        // Parse optional { field1, field2, ... } for struct variants
        let fields = if input.peek(syn::token::Brace) {
            let content;
            syn::braced!(content in input);
            content
                .parse_terminated(Ident::parse, Token![,])?
                .into_iter()
                .collect()
        } else {
            vec![]
        };

        Ok(Self {
            pda_type,
            variant,
            fields,
        })
    }
}

/// Arguments for the account attribute macro
pub struct AccountArgs {
    /// The account type path (e.g., `MinesAccount::Automation`)
    account_type: Path,
    /// Optional known address for singleton accounts (e.g., id = `GLOBAL_STATE_ADDRESS`)
    id: Option<Expr>,
    /// Whether to derive Bump trait (expects a field named `bump: u8`)
    bump: bool,
    /// Optional PDA specification (e.g., pda = `MinesPdas::Miner(mine`, authority))
    pda: Option<PdaSpec>,
}

impl Parse for AccountArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let account_type: Path = input.parse()?;

        let mut id = None;
        let mut bump = false;
        let mut pda = None;

        // Parse optional parameters: ", id = ADDRESS", ", bump", ", pda = ..."
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let ident: syn::Ident = input.parse()?;

            if ident == "id" {
                input.parse::<Token![=]>()?;
                id = Some(input.parse::<Expr>()?);
            } else if ident == "bump" {
                bump = true;
            } else if ident == "pda" {
                input.parse::<Token![=]>()?;
                pda = Some(input.parse::<PdaSpec>()?);
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("expected 'id', 'bump', or 'pda', found '{ident}'"),
                ));
            }
        }

        Ok(Self {
            account_type,
            id,
            bump,
            pda,
        })
    }
}

/// Core implementation for account attribute macro
pub fn account_impl(args: AccountArgs, input: DeriveInput) -> TokenStream2 {
    // Extract the enum type and variant from the path
    // Supports both:
    // - Full path: #[account(MinesAccount::Automation)] -> MinesAccount::Automation
    // - Shorthand: #[account(Automation)] -> crate::program::AccountType::Automation
    let segments: Vec<_> = args.account_type.segments.iter().collect();
    let (enum_type, variant) = match segments.len() {
        1 => {
            // Shorthand: use crate::program::AccountType
            let variant = &segments[0].ident;
            (format_ident!("AccountType"), variant.clone())
        }
        2 => {
            // Full path: EnumType::Variant
            let enum_type = &segments[0].ident;
            let variant = &segments[1].ident;
            (enum_type.clone(), variant.clone())
        }
        _ => panic!(
            "Expected Variant or EnumType::Variant syntax (e.g., Automation or MinesAccount::Automation)"
        ),
    };
    let enum_path = if segments.len() == 1 {
        quote! { crate::program::#enum_type }
    } else {
        let enum_type = &segments[0].ident;
        quote! { #enum_type }
    };

    // Extract docs from the struct
    let docs = extract_docs(&input.attrs);

    // Apply zero_copy transformation first (adds repr(C), Clone, Copy, PartialEq, Eq, Pod, Zeroable)
    let zero_copy_output = zero_copy_impl(input);

    // Parse back to DeriveInput to add more derives
    let mut input: DeriveInput =
        syn::parse2(zero_copy_output).expect("zero_copy_impl should produce valid DeriveInput");

    // Add additional derives for account types: Debug and IdlType
    // Note: Default is NOT included since not all account types support it
    // Add #[derive(Default)] manually if needed
    let extra_derives: syn::Attribute = parse_quote! {
        #[derive(Debug, ::panchor::IdlType)]
    };
    // Insert after the zero_copy derives (position 2 = after repr(C) and first derive)
    input.attrs.insert(2, extra_derives);

    let name = &input.ident;
    let name_str = name.to_string();

    // Generate docs expression for IDL
    let docs_expr = if docs.is_empty() {
        quote! { ::alloc::vec::Vec::new() }
    } else {
        quote! { ::alloc::vec![#(#docs.to_string()),*] }
    };

    // Generate test module name
    let test_mod_name = format_ident!("__idl_account_{}", name_str.to_lowercase());

    // Generate optional Id trait impl for singleton accounts
    let id_impl = args.id.map(|addr| {
        quote! {
            impl ::panchor::Id for #name {
                const ID: ::panchor::pinocchio::pubkey::Pubkey = #addr;
            }
        }
    });

    // Generate SetBump trait impl for all account types
    // For types with `bump` flag, this sets the bump field
    // For types without `bump`, this is a no-op
    let set_bump_impl = if args.bump {
        quote! {
            impl ::panchor::SetBump for #name {
                #[inline]
                fn set_bump(&mut self, value: u8) {
                    self.bump = value;
                }
            }
        }
    } else {
        quote! {
            impl ::panchor::SetBump for #name {
                #[inline]
                fn set_bump(&mut self, _value: u8) {}
            }
        }
    };

    // Generate PdaAccount trait impl if pda is specified
    let pda_account_impl = args.pda.as_ref().map(|pda_spec| {
        let pda_type = &pda_spec.pda_type;
        let pda_variant = &pda_spec.variant;
        let fields = &pda_spec.fields;

        // Generate the PDA variant construction
        // For unit variants: MinesPdas::GlobalState
        // For struct variants: MinesPdas::Miner { mine: self.mine, authority: self.authority }
        let variant_construction = if fields.is_empty() {
            quote! { #pda_type::#pda_variant }
        } else {
            let field_inits: Vec<_> = fields.iter().map(|f| quote! { #f: self.#f }).collect();
            quote! { #pda_type::#pda_variant { #(#field_inits),* } }
        };

        quote! {
            impl ::panchor::PdaAccount for #name {
                type Pdas = #pda_type;

                fn pda_seed_args(&self) -> Self::Pdas {
                    #variant_construction
                }
            }
        }
    });

    // Generate PdaAccountWithBump trait impl if pda is specified
    let pda_account_with_bump_impl = args.pda.as_ref().map(|pda_spec| {
        let pda_type = &pda_spec.pda_type;
        let pda_variant = &pda_spec.variant;
        let fields = &pda_spec.fields;

        // Generate the bump retrieval
        // If bump flag is set, use self.bump directly
        // Otherwise, call find_{variant}_pda(...) to calculate the bump
        let bump_expr = if args.bump {
            quote! { self.bump }
        } else {
            // Generate the finder function name: find_{snake_case_variant}_pda
            let variant_snake = to_snake_case(&pda_variant.to_string());
            let find_fn = format_ident!("find_{}_pda", variant_snake);

            // Generate the finder function arguments
            // For each field, pass a reference (or value for u64)
            let find_args: Vec<_> = fields.iter().map(|f| quote! { &self.#f }).collect();

            quote! {
                {
                    let (_, bump) = crate::pda::#find_fn(#(#find_args),*);
                    bump
                }
            }
        };

        quote! {
            impl ::panchor::PdaAccountWithBump for #name {
                type Pdas = #pda_type;

                fn pda_seed_args_with_bump(&self) -> (Self::Pdas, u8) {
                    (::panchor::PdaAccount::pda_seed_args(self), #bump_expr)
                }
            }
        }
    });

    // Generate the trait implementations
    // Note: crate::ID is used intentionally - it resolves in the caller's crate context
    quote! {
        #input

        impl panchor::Discriminator for #name {
            const DISCRIMINATOR: u64 = #enum_path::#variant as u64;
        }

        // Note: InnerSize is automatically implemented via blanket impl for Pod types

        impl panchor::ProgramOwned for #name {
            const PROGRAM_ID: ::panchor::pinocchio::pubkey::Pubkey = crate::ID;
        }

        #id_impl

        #set_bump_impl

        #pda_account_impl

        #pda_account_with_bump_impl

        #[cfg(feature = "idl-build")]
        impl ::panchor::panchor_idl::IdlBuildAccount for #name {
            fn __idl_account_name() -> &'static str {
                #name_str
            }

            fn __idl_account_discriminator() -> u64 {
                #enum_path::#variant as u64
            }

            fn __idl_account_docs() -> ::alloc::vec::Vec<::alloc::string::String> {
                extern crate alloc;
                use alloc::string::ToString;
                #docs_expr
            }
        }

        #[cfg(all(test, feature = "idl-build"))]
        mod #test_mod_name {
            extern crate std;
            extern crate alloc;
            use super::*;
            use alloc::string::ToString;

            #[test]
            fn __idl_build_account() {
                use ::panchor::panchor_idl::IdlBuildAccount;
                let account_def = ::panchor::panchor_idl::IdlAccount {
                    name: <#name as IdlBuildAccount>::__idl_account_name().to_string(),
                    discriminator: {
                        let disc = <#name as IdlBuildAccount>::__idl_account_discriminator();
                        disc.to_le_bytes().to_vec()
                    },
                };
                let json = ::serde_json::to_string_pretty(&account_def).expect("Failed to serialize account");
                std::println!("--- IDL account {} ---", #name_str);
                std::println!("{}", json);
                std::println!("--- end ---");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_and_expand(attr: TokenStream2, input: TokenStream2) -> TokenStream2 {
        let args = syn::parse2::<AccountArgs>(attr).unwrap();
        let input = syn::parse2::<DeriveInput>(input).unwrap();
        account_impl(args, input)
    }

    #[test]
    fn test_account_attribute_basic() {
        let attr = quote!(MinesAccount::Automation);
        let input = quote! {
            pub struct Automation {
                pub mine: Pubkey,
                pub authority: Pubkey,
                pub amount: u64,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check that repr(C) was added
        assert!(output_str.contains("repr (C)"));

        // Check that derives were added
        assert!(output_str.contains("derive"));
        assert!(output_str.contains("Clone"));
        assert!(output_str.contains("Copy"));
        assert!(output_str.contains("Debug"));
        assert!(output_str.contains("PartialEq"));
        assert!(output_str.contains("Eq"));
        assert!(output_str.contains("Pod"));
        assert!(output_str.contains("Zeroable"));
        assert!(output_str.contains("IdlType"));
        // Default is NOT automatically added
        assert!(!output_str.contains("Default"));

        // Check trait implementations
        assert!(output_str.contains("impl panchor :: Discriminator for Automation"));
        assert!(
            output_str.contains("const DISCRIMINATOR : u64 = MinesAccount :: Automation as u64")
        );
        // InnerSize is now provided via blanket impl for Pod types (not generated here)
        assert!(output_str.contains("impl panchor :: ProgramOwned for Automation"));
        assert!(output_str.contains("const PROGRAM_ID"));
        assert!(output_str.contains("crate :: ID"));
    }

    #[test]
    fn test_account_attribute_preserves_attrs() {
        let attr = quote!(MinesAccount::Mine);
        let input = quote! {
            /// My account docs
            pub struct Mine {
                pub creator: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check that doc comments are preserved
        assert!(output_str.contains("My account docs"));
        // Check that repr(C) was added
        assert!(output_str.contains("repr (C)"));
    }

    #[test]
    fn test_account_attribute_shorthand() {
        // Single segment path should use crate::program::AccountType
        let attr = quote!(Automation);
        let input = quote! {
            pub struct Automation {
                pub amount: u64,
            }
        };
        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Should use crate::program::AccountType::Automation for discriminator
        assert!(output_str.contains("crate :: program :: AccountType :: Automation"));
    }

    #[test]
    #[should_panic(expected = "Expected Variant or EnumType::Variant syntax")]
    fn test_account_attribute_invalid_path() {
        let attr = quote!(foo::bar::baz);
        let input = quote! {
            pub struct Baz {
                pub amount: u64,
            }
        };
        parse_and_expand(attr, input);
    }

    #[test]
    fn test_account_all_variants() {
        // Test each account type to ensure the macro works for all variants
        let variants = [
            ("MinesAccount::Automation", "Automation"),
            ("MinesAccount::Mine", "Mine"),
            ("MinesAccount::Miner", "Miner"),
            ("MinesAccount::Stake", "Stake"),
            ("MinesAccount::Round", "Round"),
            ("MinesAccount::GlobalState", "GlobalState"),
            ("MinesAccount::ReferrerState", "ReferrerState"),
        ];

        for (attr_str, struct_name) in variants {
            let attr: TokenStream2 = attr_str.parse().unwrap();
            let input = format!(
                r#"
                #[repr(C)]
                pub struct {struct_name} {{
                    pub data: u64,
                }}
            "#
            );
            let input: TokenStream2 = input.parse().unwrap();

            let output = parse_and_expand(attr, input);
            let output_str = output.to_string();

            assert!(
                output_str.contains(&format!("impl panchor :: Discriminator for {struct_name}")),
                "Missing Discriminator impl for {struct_name}"
            );
            // InnerSize is now provided via blanket impl for Pod types
            assert!(
                output_str.contains(&format!("impl panchor :: ProgramOwned for {struct_name}")),
                "Missing ProgramOwned impl for {struct_name}"
            );
        }
    }

    #[test]
    fn test_account_with_id_constraint() {
        let attr = quote!(MinesAccount::GlobalState, id = GLOBAL_STATE_ADDRESS);
        let input = quote! {
            pub struct GlobalState {
                pub admin: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check standard implementations are present
        assert!(output_str.contains("impl panchor :: Discriminator for GlobalState"));
        assert!(output_str.contains("impl panchor :: ProgramOwned for GlobalState"));

        // Check Id trait implementation is generated
        assert!(output_str.contains("impl :: panchor :: Id for GlobalState"));
        assert!(output_str.contains("GLOBAL_STATE_ADDRESS"));
    }

    #[test]
    fn test_account_without_id_constraint() {
        let attr = quote!(MinesAccount::Mine);
        let input = quote! {
            pub struct Mine {
                pub creator: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check standard implementations are present
        assert!(output_str.contains("impl panchor :: Discriminator for Mine"));
        assert!(output_str.contains("impl panchor :: ProgramOwned for Mine"));

        // Check Id trait implementation is NOT generated
        assert!(!output_str.contains("impl :: panchor :: Id for Mine"));
    }

    #[test]
    fn test_account_with_bump_constraint() {
        let attr = quote!(MinesAccount::Mine, bump);
        let input = quote! {
            pub struct Mine {
                pub bump: u8,
                pub creator: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check standard implementations are present
        assert!(output_str.contains("impl panchor :: Discriminator for Mine"));
        assert!(output_str.contains("impl panchor :: ProgramOwned for Mine"));

        // Check SetBump trait implementation is generated
        assert!(output_str.contains("impl :: panchor :: SetBump for Mine"));
        assert!(output_str.contains("fn set_bump (& mut self , value : u8)"));
    }

    #[test]
    fn test_account_with_bump_and_id() {
        let attr = quote!(MinesAccount::GlobalState, id = GLOBAL_STATE_ADDRESS, bump);
        let input = quote! {
            pub struct GlobalState {
                pub bump: u8,
                pub admin: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check all implementations are present
        assert!(output_str.contains("impl panchor :: Discriminator for GlobalState"));
        assert!(output_str.contains("impl panchor :: ProgramOwned for GlobalState"));
        assert!(output_str.contains("impl :: panchor :: Id for GlobalState"));
        assert!(output_str.contains("impl :: panchor :: SetBump for GlobalState"));
    }

    #[test]
    fn test_account_without_bump_constraint() {
        let attr = quote!(MinesAccount::Mine);
        let input = quote! {
            pub struct Mine {
                pub creator: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check SetBump trait implementation IS generated (as no-op)
        assert!(output_str.contains("impl :: panchor :: SetBump for Mine"));
        // But it should use _value (unused parameter) since there's no bump field
        assert!(output_str.contains("_value"));
    }

    #[test]
    fn test_account_with_pda_struct_variant() {
        let attr = quote!(
            MinesAccount::Miner,
            pda = MinesPdas::Miner { mine, authority }
        );
        let input = quote! {
            #[repr(C)]
            pub struct Miner {
                pub mine: Pubkey,
                pub authority: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check PdaAccount trait implementation is generated
        assert!(
            output_str.contains("impl :: panchor :: PdaAccount for Miner"),
            "Missing PdaAccount impl"
        );
        assert!(
            output_str.contains("type Pdas = MinesPdas"),
            "Missing Pdas type alias"
        );
        assert!(
            output_str.contains("MinesPdas :: Miner"),
            "Missing variant construction"
        );
        assert!(
            output_str.contains("mine : self . mine"),
            "Missing mine field init"
        );
        assert!(
            output_str.contains("authority : self . authority"),
            "Missing authority field init"
        );

        // Check PdaAccountWithBump trait implementation is generated
        assert!(
            output_str.contains("impl :: panchor :: PdaAccountWithBump for Miner"),
            "Missing PdaAccountWithBump impl"
        );
        // Without bump flag, should call find_miner_pda
        assert!(
            output_str.contains("find_miner_pda"),
            "Missing find_miner_pda call for bump calculation"
        );
    }

    #[test]
    fn test_account_with_pda_unit_variant() {
        let attr = quote!(MinesAccount::GlobalState, pda = MinesPdas::GlobalState);
        let input = quote! {
            #[repr(C)]
            pub struct GlobalState {
                pub admin: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check PdaAccount trait implementation is generated
        assert!(
            output_str.contains("impl :: panchor :: PdaAccount for GlobalState"),
            "Missing PdaAccount impl"
        );
        // For unit variants, should NOT have field initialization
        assert!(
            output_str.contains("MinesPdas :: GlobalState"),
            "Missing unit variant construction"
        );

        // Check PdaAccountWithBump trait implementation is generated
        assert!(
            output_str.contains("impl :: panchor :: PdaAccountWithBump for GlobalState"),
            "Missing PdaAccountWithBump impl"
        );
    }

    #[test]
    fn test_account_with_pda_and_bump() {
        let attr = quote!(
            MinesAccount::Miner,
            bump,
            pda = MinesPdas::Miner { mine, authority }
        );
        let input = quote! {
            #[repr(C)]
            pub struct Miner {
                pub bump: u8,
                pub mine: Pubkey,
                pub authority: Pubkey,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check PdaAccount trait implementation is generated
        assert!(
            output_str.contains("impl :: panchor :: PdaAccount for Miner"),
            "Missing PdaAccount impl"
        );

        // Check PdaAccountWithBump uses self.bump instead of find_*_pda
        assert!(
            output_str.contains("impl :: panchor :: PdaAccountWithBump for Miner"),
            "Missing PdaAccountWithBump impl"
        );
        assert!(
            output_str.contains("self . bump"),
            "Should use self.bump when bump flag is set"
        );
        // Should NOT call find_miner_pda when bump is stored
        assert!(
            !output_str.contains("find_miner_pda"),
            "Should not call find_miner_pda when bump flag is set"
        );
    }

    #[test]
    fn test_account_without_pda() {
        let attr = quote!(MinesAccount::Automation);
        let input = quote! {
            #[repr(C)]
            pub struct Automation {
                pub amount: u64,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Should NOT have PdaAccount or PdaAccountWithBump implementations
        assert!(
            !output_str.contains("impl :: panchor :: PdaAccount"),
            "Should not have PdaAccount impl without pda attribute"
        );
        assert!(
            !output_str.contains("impl :: panchor :: PdaAccountWithBump"),
            "Should not have PdaAccountWithBump impl without pda attribute"
        );
    }

    #[test]
    fn test_pda_spec_parsing() {
        // Test parsing of PdaSpec for struct variant with fields
        let pda_spec: PdaSpec = syn::parse_str("MinesPdas::Miner { mine, authority }").unwrap();
        assert_eq!(pda_spec.pda_type.to_string(), "MinesPdas");
        assert_eq!(pda_spec.variant.to_string(), "Miner");
        assert_eq!(pda_spec.fields.len(), 2);
        assert_eq!(pda_spec.fields[0].to_string(), "mine");
        assert_eq!(pda_spec.fields[1].to_string(), "authority");

        // Test parsing of PdaSpec for unit variant
        let pda_spec: PdaSpec = syn::parse_str("MinesPdas::GlobalState").unwrap();
        assert_eq!(pda_spec.pda_type.to_string(), "MinesPdas");
        assert_eq!(pda_spec.variant.to_string(), "GlobalState");
        assert!(pda_spec.fields.is_empty());
    }
}
