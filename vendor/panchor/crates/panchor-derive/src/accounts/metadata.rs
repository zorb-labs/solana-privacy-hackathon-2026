//! Account metadata and IDL generation for the Accounts derive macro.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{Expr, Ident, Path};

/// Account metadata for IDL/SDK generation
pub struct AccountMeta {
    pub name: Ident,
    pub doc: Option<String>,
    pub signer: bool,
    pub mutable: bool,
    /// Program address expression for IDL (e.g., "& `SYSTEM_PROGRAM_ID`")
    pub program_expr: Option<Expr>,
    /// Address expression for IDL (e.g., "& `GLOBAL_STATE_ADDRESS`")
    pub address_expr: Option<Expr>,
    /// Type to use for Id trait (when #[account(id)] is set) - gets address from `T::ID`
    pub id_type: Option<Path>,
}

/// Generate {Name}Input struct with Pubkey fields for SDK use
/// If the name ends with "Accounts", it's replaced with "Input"
/// e.g., `CreateMineAccounts` -> `CreateMineInput`
pub fn generate_input_struct(name: &Ident, accounts: &[AccountMeta]) -> TokenStream2 {
    // Convert FooAccounts -> FooInput
    let name_str = name.to_string();
    let input_name = if let Some(stripped) = name_str.strip_suffix("Accounts") {
        format_ident!("{}Input", stripped)
    } else {
        format_ident!("{}Input", name_str)
    };

    // Generate struct fields with Pubkey type
    let fields: Vec<_> = accounts
        .iter()
        .map(|a| {
            let field_name = &a.name;
            let doc = a.doc.clone().unwrap_or_else(|| field_name.to_string());
            quote! {
                #[doc = #doc]
                pub #field_name: ::solana_sdk::pubkey::Pubkey
            }
        })
        .collect();

    // Generate to_account_metas method
    let metas: Vec<_> = accounts
        .iter()
        .map(|a| {
            let field_name = &a.name;
            let signer = a.signer;
            let mutable = a.mutable;

            if mutable {
                quote! { ::solana_sdk::instruction::AccountMeta::new(self.#field_name, #signer) }
            } else {
                quote! { ::solana_sdk::instruction::AccountMeta::new_readonly(self.#field_name, #signer) }
            }
        })
        .collect();

    // Generate doc comment for the struct
    let struct_doc =
        format!("Input builder for {name}.\n\nContains Pubkey fields for building instructions.");

    // Note: This struct is exported at the module level but the to_ix() impl is in the client module
    quote! {
        #[cfg(feature = "solana-sdk")]
        #[doc = #struct_doc]
        #[derive(Debug, Clone, Copy)]
        pub struct #input_name {
            #(#fields),*
        }

        #[cfg(feature = "solana-sdk")]
        impl #input_name {
            /// Convert to a vector of AccountMeta for instruction building.
            pub fn to_account_metas(&self) -> ::alloc::vec::Vec<::solana_sdk::instruction::AccountMeta> {
                ::alloc::vec![#(#metas),*]
            }
        }
    }
}

/// Generate IDL build method that returns account metadata when idl-build feature is enabled.
///
/// This generates a static method that returns `Vec<IdlInstructionAccount>` containing
/// all account metadata including resolved addresses.
pub fn generate_idl_build_test(name: &Ident, accounts: &[AccountMeta]) -> TokenStream2 {
    // Build the account metas at compile time with addresses resolved at runtime
    let account_meta_exprs: Vec<TokenStream2> = accounts
        .iter()
        .map(|a| {
            let field_name = a.name.to_string();
            let doc = a.doc.clone().unwrap_or_default();
            let signer = a.signer;
            let mutable = a.mutable;

            // Determine address expression for IDL:
            // 1. If id_type is set, use <Type as Id>::ID
            // 2. If program_expr is set, use that expression
            // 3. If address_expr is set, use that expression
            // 4. Otherwise, None
            let address_expr = if let Some(id_type) = &a.id_type {
                quote! {
                    Some(::panchor::panchor_idl::pubkey_to_base58(&<#id_type as ::panchor::Id>::ID))
                }
            } else if let Some(expr) = &a.program_expr {
                quote! {
                    Some(::panchor::panchor_idl::pubkey_to_base58(&#expr))
                }
            } else if let Some(expr) = &a.address_expr {
                quote! {
                    Some(::panchor::panchor_idl::pubkey_to_base58(&#expr))
                }
            } else {
                quote! { None }
            };

            quote! {
                ::panchor::panchor_idl::IdlInstructionAccount {
                    name: #field_name.to_string(),
                    docs: if #doc.is_empty() {
                        ::alloc::vec::Vec::new()
                    } else {
                        ::alloc::vec![#doc.to_string()]
                    },
                    writable: #mutable,
                    signer: #signer,
                    address: #address_expr,
                    optional: false,
                    pda: None,
                    relations: ::alloc::vec::Vec::new(),
                }
            }
        })
        .collect();

    quote! {
        #[cfg(feature = "idl-build")]
        impl<'info> #name<'info> {
            /// Build IDL instruction accounts for this instruction's accounts.
            pub fn __idl_instruction_accounts() -> ::alloc::vec::Vec<::panchor::panchor_idl::IdlInstructionAccount> {
                extern crate alloc;
                use alloc::string::ToString;
                ::alloc::vec![#(#account_meta_exprs),*]
            }
        }
    }
}
