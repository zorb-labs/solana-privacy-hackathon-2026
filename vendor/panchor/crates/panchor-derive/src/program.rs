//! Program macro for declaring program ID and generating IDL metadata
//!
//! This macro combines `declare_id!` with IDL generation for program metadata.
//! Supports both string literals and const path expressions for the program ID.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Error, Expr, Ident, LitStr, Result, Token,
    parse::{Parse, ParseStream},
};

use crate::utils::to_pascal_case;

/// Program ID can be either a string literal or a const path expression
pub enum ProgramId {
    /// A string literal like `"Ar4QfyyGcZENwwHcYA8d45XcnjtjcaWBSHzEzvyAP5dT"`
    Literal(LitStr),
    /// A const path like `zorb_program_ids::SHIELDED_POOL_ID`
    ConstPath(Expr),
}

impl Parse for ProgramId {
    fn parse(input: ParseStream) -> Result<Self> {
        // Try to parse as a string literal first
        if input.peek(LitStr) {
            Ok(Self::Literal(input.parse()?))
        } else {
            // Otherwise parse as an expression (const path)
            Ok(Self::ConstPath(input.parse()?))
        }
    }
}

/// Parsed arguments for the program! macro
pub struct ProgramArgs {
    pub name: Option<Ident>,
    pub id: ProgramId,
    pub instructions: Expr,
    pub accounts: Option<Expr>,
    pub events: Option<Expr>,
    pub pdas: Option<Expr>,
}

/// Single key=value pair in the program macro
enum ProgramParam {
    Name(Ident),
    Id(ProgramId),
    Instructions(Expr),
    Accounts(Expr),
    Events(Expr),
    Pdas(Expr),
}

impl Parse for ProgramParam {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;
        input.parse::<Token![=]>()?;

        match ident.to_string().as_str() {
            "name" => {
                let name: Ident = input.parse()?;
                Ok(Self::Name(name))
            }
            "id" => {
                let id: ProgramId = input.parse()?;
                Ok(Self::Id(id))
            }
            "instructions" => {
                let expr: Expr = input.parse()?;
                Ok(Self::Instructions(expr))
            }
            "accounts" => {
                let expr: Expr = input.parse()?;
                Ok(Self::Accounts(expr))
            }
            "events" => {
                let expr: Expr = input.parse()?;
                Ok(Self::Events(expr))
            }
            "pdas" => {
                let expr: Expr = input.parse()?;
                Ok(Self::Pdas(expr))
            }
            _ => Err(Error::new(
                ident.span(),
                format!(
                    "Unknown program parameter: {ident}. Expected name, id, instructions, accounts, events, or pdas"
                ),
            )),
        }
    }
}

impl Parse for ProgramArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        let mut id = None;
        let mut instructions = None;
        let mut accounts = None;
        let mut events = None;
        let mut pdas = None;

        while !input.is_empty() {
            let param: ProgramParam = input.parse()?;
            match param {
                ProgramParam::Name(ident) => name = Some(ident),
                ProgramParam::Id(program_id) => id = Some(program_id),
                ProgramParam::Instructions(expr) => instructions = Some(expr),
                ProgramParam::Accounts(expr) => accounts = Some(expr),
                ProgramParam::Events(expr) => events = Some(expr),
                ProgramParam::Pdas(expr) => pdas = Some(expr),
            }

            // Handle optional comma
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let id = id.ok_or_else(|| Error::new(input.span(), "Missing required parameter: id"))?;
        let instructions = instructions
            .ok_or_else(|| Error::new(input.span(), "Missing required parameter: instructions"))?;

        Ok(Self {
            name,
            id,
            instructions,
            accounts,
            events,
            pdas,
        })
    }
}

/// Implementation of the program! macro
pub fn program_impl(args: ProgramArgs) -> TokenStream2 {
    // Get name from args or derive from CARGO_CRATE_NAME
    let name = args.name.unwrap_or_else(|| {
        let crate_name =
            std::env::var("CARGO_CRATE_NAME").unwrap_or_else(|_| "Program".to_string());
        let pascal_name = to_pascal_case(&crate_name);
        format_ident!("{}", pascal_name)
    });
    let instructions = &args.instructions;

    // Generate IDL test module name
    let test_mod_name = format_ident!("__idl_build_program");

    // Generate accounts output if specified
    let accounts_output = if let Some(accounts) = &args.accounts {
        quote! {
            // Output account type name for IDL to extract accounts from
            std::println!("--- IDL accounts {} ---", stringify!(#accounts));
        }
    } else {
        quote! {}
    };

    // Generate events output if specified
    let events_output = if let Some(events) = &args.events {
        quote! {
            // Output event type name for IDL to extract events from
            std::println!("--- IDL events {} ---", stringify!(#events));
        }
    } else {
        quote! {}
    };

    // Generate program module with type aliases
    let account_type_alias = if let Some(accounts) = &args.accounts {
        quote! {
            /// Account discriminator type for this program
            pub type AccountType = #accounts;
        }
    } else {
        quote! {}
    };

    let event_type_alias = if let Some(events) = &args.events {
        quote! {
            /// Event discriminator type for this program
            pub type EventType = #events;
        }
    } else {
        quote! {}
    };

    let pdas_type_alias = if let Some(pdas) = &args.pdas {
        quote! {
            /// PDA definitions type for this program
            pub type Pdas = #pdas;
        }
    } else {
        quote! {}
    };

    // Generate the ID declaration based on whether we have a literal or const path
    let (id_declaration, id_for_idl) = match &args.id {
        ProgramId::Literal(lit) => {
            // For string literals, use the original declare_id! macro
            let declaration = quote! {
                ::panchor::pinocchio_pubkey::declare_id!(#lit);
            };
            let idl_output = quote! { #lit };
            (declaration, idl_output)
        }
        ProgramId::ConstPath(path) => {
            // For const paths, use five8_const to decode at compile time
            // This allows using `zorb_program_ids::SHIELDED_POOL_ID` as the ID
            // Note: pinocchio::pubkey::Pubkey is a type alias for [u8; 32], not a struct
            let declaration = quote! {
                /// Program ID decoded from const string at compile time
                pub static ID: ::panchor::pinocchio::pubkey::Pubkey =
                    ::panchor::five8_const::decode_32_const(#path);

                /// Returns the program ID
                pub fn id() -> ::panchor::pinocchio::pubkey::Pubkey {
                    ID
                }

                /// Checks if the given pubkey matches the program ID
                pub fn check_id(id: &::panchor::pinocchio::pubkey::Pubkey) -> bool {
                    *id == ID
                }
            };
            let idl_output = quote! { #path };
            (declaration, idl_output)
        }
    };

    quote! {
        // Emit the ID declaration (either declare_id! or const-based)
        #id_declaration

        /// Program marker type for use with `Program<'info, #name>`
        pub struct #name;

        impl ::panchor::accounts::Id for #name {
            const ID: ::panchor::pinocchio::pubkey::Pubkey = ID;
        }

        // Generate the program entrypoint (only when no-entrypoint feature is not enabled)
        #[cfg(not(feature = "no-entrypoint"))]
        ::panchor::pinocchio::entrypoint!(__process_instruction);

        /// Program entrypoint - dispatches instructions to handlers
        #[cfg(not(feature = "no-entrypoint"))]
        fn __process_instruction(
            program_id: &::panchor::pinocchio::pubkey::Pubkey,
            accounts: &[::panchor::pinocchio::account_info::AccountInfo],
            instruction_data: &[u8],
        ) -> ::panchor::pinocchio::ProgramResult {
            ::panchor::process_instruction::<#instructions>(program_id, accounts, instruction_data, &ID)
        }

        /// Program types and constants module
        pub mod program {
            use super::*;

            /// Program ID constant
            pub const ID: ::panchor::pinocchio::pubkey::Pubkey = super::ID;

            /// Instruction discriminator type for this program
            pub type InstructionType = #instructions;

            #account_type_alias
            #event_type_alias
            #pdas_type_alias
        }

        /// Client module with solana-sdk compatible types
        #[cfg(feature = "solana-sdk")]
        pub mod client {
            /// Program ID as solana_sdk::pubkey::Pubkey
            pub const PROGRAM_ID: ::solana_sdk::pubkey::Pubkey =
                ::solana_sdk::pubkey::Pubkey::new_from_array(super::ID);
        }

        // Store metadata for use by other macros
        #[doc(hidden)]
        pub const __INSTRUCTIONS_TYPE: &str = stringify!(#instructions);

        /// IDL build test for program metadata
        #[cfg(all(test, feature = "idl-build"))]
        mod #test_mod_name {
            extern crate std;
            extern crate alloc;
            use super::*;
            use alloc::string::ToString;

            #[test]
            fn __idl_build_program() {
                // Output the program ID
                std::println!("--- IDL program_id {} ---", #id_for_idl);
                #accounts_output
                #events_output

                // Build instructions from the instruction enum
                let instructions = <#instructions as ::panchor::InstructionIdl>::__idl_instructions();
                let json = ::serde_json::to_string_pretty(&instructions).expect("Failed to serialize IDL");
                std::println!("--- IDL begin instructions ---");
                std::println!("{}", json);
                std::println!("--- IDL end instructions ---");

                // Output instruction data type names to exclude from types array
                let excluded_types = <#instructions as ::panchor::InstructionIdl>::__idl_excluded_types();
                for type_name in excluded_types {
                    std::println!("--- IDL exclude_type {} ---", type_name);
                }
            }
        }
    }
}
