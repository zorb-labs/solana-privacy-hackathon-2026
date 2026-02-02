//! Instructions attribute macro
//!
//! Generates the `InstructionDispatch` trait implementation for instruction enum types.
//! Uses `TryFromPrimitive` for discriminator parsing.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Error, Expr, Ident, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::utils::{extract_docs, to_snake_case};

/// Parsed handler attribute from #[handler(...)] on enum variants
pub struct HandlerAttr {
    #[allow(dead_code)] // Parsed but not currently used (dispatch moved to derive)
    pub processor: Expr,
    pub data: Option<Expr>,
    pub accounts: Option<Expr>,
    /// If true, pass raw &[u8] to processor instead of parsed data
    pub raw_data: bool,
    /// Optional type for IDL args generation (used when `raw_data` is true)
    pub idl_args: Option<Expr>,
}

/// Single key=value pair in the handler attribute (for explicit form)
enum HandlerParam {
    Processor(Expr),
    Data(Expr),
    Accounts(Expr),
    /// Shorthand `data` keyword without value - indicates auto-derive Data struct
    DataShorthand,
    /// Flag to pass raw &[u8] data to processor instead of parsing
    RawData,
    /// Type for IDL args generation only (doesn't affect runtime)
    IdlArgs(Expr),
}

impl Parse for HandlerParam {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident: Ident = input.parse()?;

        // Check if it's a shorthand (just the keyword without `=`)
        if !input.peek(Token![=]) {
            match ident.to_string().as_str() {
                "data" => return Ok(Self::DataShorthand),
                "raw_data" => return Ok(Self::RawData),
                _ => {
                    return Err(Error::new(
                        ident.span(),
                        format!("Unknown shorthand: {ident}. Use 'data' or 'raw_data' without '='"),
                    ));
                }
            }
        }

        input.parse::<Token![=]>()?;
        let expr: Expr = input.parse()?;

        match ident.to_string().as_str() {
            "processor" => Ok(Self::Processor(expr)),
            "data" => Ok(Self::Data(expr)),
            "accounts" => Ok(Self::Accounts(expr)),
            "idl_args" => Ok(Self::IdlArgs(expr)),
            _ => Err(Error::new(
                ident.span(),
                format!(
                    "Unknown handler parameter: {ident}. Expected processor, data, accounts, idl_args, or raw_data"
                ),
            )),
        }
    }
}

/// Intermediate result from parsing handler attribute
struct ParsedHandler {
    processor: Option<Expr>,
    data: Option<Expr>,
    accounts: Option<Expr>,
    use_data_shorthand: bool,
    raw_data: bool,
    idl_args: Option<Expr>,
}

impl Parse for ParsedHandler {
    fn parse(input: ParseStream) -> Result<Self> {
        // Handle empty attribute (shorthand with no data)
        if input.is_empty() {
            return Ok(Self {
                processor: None,
                data: None,
                accounts: None,
                use_data_shorthand: false,
                raw_data: false,
                idl_args: None,
            });
        }

        let params: Punctuated<HandlerParam, Token![,]> = Punctuated::parse_terminated(input)?;

        let mut processor = None;
        let mut data = None;
        let mut accounts = None;
        let mut use_data_shorthand = false;
        let mut raw_data = false;
        let mut idl_args = None;

        for param in params {
            match param {
                HandlerParam::Processor(expr) => processor = Some(expr),
                HandlerParam::Data(expr) => data = Some(expr),
                HandlerParam::Accounts(expr) => accounts = Some(expr),
                HandlerParam::DataShorthand => use_data_shorthand = true,
                HandlerParam::RawData => raw_data = true,
                HandlerParam::IdlArgs(expr) => idl_args = Some(expr),
            }
        }

        Ok(Self {
            processor,
            data,
            accounts,
            use_data_shorthand,
            raw_data,
            idl_args,
        })
    }
}

/// Resolve handler attribute with shorthand support
fn resolve_handler_attr(parsed: ParsedHandler, variant_name: &Ident) -> HandlerAttr {
    let variant_str = variant_name.to_string();
    let snake_name = to_snake_case(&variant_str);

    // Generate processor if not provided: process_{snake_case}
    let processor = parsed.processor.unwrap_or_else(|| {
        let proc_name = format_ident!("process_{}", snake_name);
        syn::parse_quote!(#proc_name)
    });

    // Generate accounts if not provided: {Variant}Accounts
    // But NOT if raw_data is set without explicit accounts (legacy raw handlers)
    let accounts = if parsed.accounts.is_some() {
        parsed.accounts
    } else {
        // Auto-generate accounts only when not using raw_data
        let accounts_name = format_ident!("{}Accounts", variant_str);
        Some(syn::parse_quote!(#accounts_name))
    };

    // Generate data if shorthand was used: {Variant}Data
    let data = if parsed.use_data_shorthand {
        let data_name = format_ident!("{}Data", variant_str);
        Some(syn::parse_quote!(#data_name))
    } else {
        parsed.data
    };

    HandlerAttr {
        processor,
        data,
        accounts,
        raw_data: parsed.raw_data,
        idl_args: parsed.idl_args,
    }
}

/// Information about a single enum variant
struct VariantInfo {
    ident: Ident,
    attr: HandlerAttr,
}

/// Parse handler attribute from a variant's attributes.
/// If no #[handler] attribute is present, uses all defaults.
fn parse_handler_attr(attrs: &[syn::Attribute], variant_name: &Ident) -> Result<HandlerAttr> {
    for attr in attrs {
        if attr.path().is_ident("handler") {
            // Empty handler attribute #[handler] is allowed - uses defaults
            if let Ok(parsed) = attr.parse_args::<ParsedHandler>() {
                return Ok(resolve_handler_attr(parsed, variant_name));
            }
            // Empty attribute - use all defaults
            return Ok(resolve_handler_attr(
                ParsedHandler {
                    processor: None,
                    data: None,
                    accounts: None,
                    use_data_shorthand: false,
                    raw_data: false,
                    idl_args: None,
                },
                variant_name,
            ));
        }
    }
    // No #[handler] attribute - use all defaults
    Ok(resolve_handler_attr(
        ParsedHandler {
            processor: None,
            data: None,
            accounts: None,
            use_data_shorthand: false,
            raw_data: false,
            idl_args: None,
        },
        variant_name,
    ))
}

/// Core implementation for instructions attribute macro
pub fn instructions_impl(mut input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;

    // Get enum variants
    let variants = match &mut input.data {
        Data::Enum(data) => &mut data.variants,
        _ => {
            return Error::new_spanned(&input.ident, "instructions attribute only supports enums")
                .to_compile_error();
        }
    };

    // Parse handler attributes from each variant and collect infos
    // Note: We do NOT remove #[handler] attributes - they're needed by #[derive(InstructionDispatch)]
    let mut variant_infos: Vec<VariantInfo> = Vec::new();
    // Also collect docs for each variant
    let mut variant_docs: Vec<Vec<String>> = Vec::new();
    for variant in variants.iter() {
        let docs = extract_docs(&variant.attrs);
        match parse_handler_attr(&variant.attrs, &variant.ident) {
            Ok(attr) => {
                variant_infos.push(VariantInfo {
                    ident: variant.ident.clone(),
                    attr,
                });
                variant_docs.push(docs);
            }
            Err(e) => return e.to_compile_error(),
        }
    }

    // Add the required attributes
    // InstructionDispatch is generated by the derive macro (reads #[handler] attributes)
    let repr: syn::Attribute = syn::parse_quote! {
        #[repr(u8)]
    };
    let derive: syn::Attribute = syn::parse_quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, ::panchor::strum::AsRefStr, ::panchor::num_enum::TryFromPrimitive, ::panchor::InstructionDispatch)]
    };
    input.attrs.insert(0, repr);
    input.attrs.insert(1, derive);

    // Generate IDL build test (only when idl-build feature is enabled)
    let idl_build_test = generate_idl_build_test(name, &variant_infos, &variant_docs);

    // Generate client module (behind cfg feature)
    let client_module = generate_client_module(name, &variant_infos);

    quote! {
        #input

        #idl_build_test

        #client_module
    }
}

/// Generate `InstructionIdl` trait implementation for the instruction enum.
///
/// This generates an implementation that:
/// 1. Returns `IdlInstruction` for each variant with handler attributes
/// 2. Calls `AccountsType::__idl_instruction_accounts()` to get account metadata
/// 3. Calls `DataType::__idl_args()` to get args fields (if data type is specified)
/// 4. Returns the list of instruction data type names to exclude from the types array
fn generate_idl_build_test(
    enum_name: &Ident,
    variant_infos: &[VariantInfo],
    variant_docs: &[Vec<String>],
) -> TokenStream2 {
    // Collect instruction data type names to exclude from types array
    let excluded_type_names: Vec<String> = variant_infos
        .iter()
        .filter_map(|info| {
            // Get the data type (either from idl_args or data attribute)
            info.attr
                .idl_args
                .as_ref()
                .or(info.attr.data.as_ref())
                .map(|expr| {
                    // Convert the expression to a string and extract the type name
                    use quote::ToTokens;
                    let type_str = expr.to_token_stream().to_string();
                    // Handle generic types by taking just the base name
                    type_str
                        .split('<')
                        .next()
                        .unwrap_or(&type_str)
                        .trim()
                        .to_string()
                })
        })
        .collect();

    // Generate instruction builders for all variants
    let instruction_builders: Vec<TokenStream2> = variant_infos
        .iter()
        .zip(variant_docs.iter())
        .map(|(info, docs)| {
            let attr = &info.attr;
            let variant_ident = &info.ident;
            let snake_name = to_snake_case(&variant_ident.to_string());

            // Get accounts if accounts type is specified
            // Convert IdlInstructionAccount to IdlInstructionAccountItem::Single
            let accounts_expr = if let Some(accounts_type) = &attr.accounts {
                quote! {
                    <#accounts_type>::__idl_instruction_accounts()
                        .into_iter()
                        .map(::panchor::panchor_idl::IdlInstructionAccountItem::Single)
                        .collect::<Vec<_>>()
                }
            } else {
                quote! { Vec::new() }
            };

            // Convert docs to expression
            let docs_expr = if docs.is_empty() {
                quote! { Vec::new() }
            } else {
                quote! { alloc::vec![#(#docs.to_string()),*] }
            };

            // Handle args:
            // - If idl_args is set, call __idl_args() on that type
            // - If data is set, call __idl_args() on that type
            // - If raw_data is set (without idl_args), generate a bytes arg
            // - Otherwise, no args
            let args_expr = if let Some(args_type) = attr.idl_args.as_ref().or(attr.data.as_ref()) {
                quote! {
                    <#args_type as ::panchor::panchor_idl::IdlBuildArgs>::__idl_args()
                }
            } else if attr.raw_data {
                // raw_data without idl_args: generate a single bytes argument
                quote! {
                    alloc::vec![::panchor::panchor_idl::IdlField {
                        name: "data".to_string(),
                        docs: alloc::vec!["Raw instruction data bytes".to_string()],
                        ty: ::panchor::panchor_idl::IdlType::Bytes,
                    }]
                }
            } else {
                quote! { Vec::new() }
            };

            quote! {
                instructions.push(::panchor::panchor_idl::IdlInstruction {
                    name: #snake_name.to_string(),
                    docs: #docs_expr,
                    discriminator: alloc::vec![#enum_name::#variant_ident as u8],
                    accounts: #accounts_expr,
                    args: #args_expr,
                    returns: None,
                });
            }
        })
        .collect();

    quote! {
        #[cfg(feature = "idl-build")]
        impl ::panchor::InstructionIdl for #enum_name {
            fn __idl_instructions() -> Vec<::panchor::panchor_idl::IdlInstruction> {
                extern crate alloc;
                use alloc::string::ToString;
                use alloc::vec::Vec;

                let mut instructions: Vec<::panchor::panchor_idl::IdlInstruction> = Vec::new();

                #(#instruction_builders)*

                instructions
            }

            fn __idl_excluded_types() -> Vec<String> {
                extern crate alloc;
                use alloc::string::{String, ToString};
                use alloc::vec::Vec;
                alloc::vec![#(#excluded_type_names.to_string()),*]
            }
        }
    }
}

/// Generate client module with `to_ix()` implementations for Input structs.
///
/// This generates a `client` module containing:
/// - `to_ix()` method implementations for `{Variant}Input` structs
fn generate_client_module(enum_name: &Ident, variant_infos: &[VariantInfo]) -> TokenStream2 {
    // Generate to_ix impl for each variant with accounts
    let to_ix_impls: Vec<TokenStream2> = variant_infos
        .iter()
        .filter_map(|info| {
            let variant_ident = &info.ident;
            let attr = &info.attr;

            // Only generate for variants with accounts
            let accounts_type = attr.accounts.as_ref()?;

            // Get the input type name: {Variant}Accounts -> {Variant}Input
            use quote::ToTokens;
            let accounts_str = accounts_type.to_token_stream().to_string();
            let input_type = if let Some(stripped) = accounts_str.strip_suffix("Accounts") {
                format_ident!("{}Input", stripped)
            } else {
                format_ident!("{}Input", accounts_str)
            };

            // Generate the to_ix implementation based on data type
            let to_ix_impl = if attr.raw_data {
                // raw_data: takes &[u8] directly
                quote! {
                    impl #input_type {
                        /// Build a complete instruction with raw data bytes.
                        pub fn to_ix(&self, data: &[u8]) -> ::solana_sdk::instruction::Instruction {
                            extern crate alloc;
                            let mut instruction_data = alloc::vec![super::#enum_name::#variant_ident as u8];
                            instruction_data.extend_from_slice(data);
                            ::solana_sdk::instruction::Instruction {
                                program_id: ::solana_sdk::pubkey::Pubkey::new_from_array(crate::ID),
                                accounts: self.to_account_metas(),
                                data: instruction_data,
                            }
                        }
                    }
                }
            } else if let Some(data_ty) = attr.data.as_ref() {
                // Has data type: use bytemuck for Pod types
                quote! {
                    impl #input_type {
                        /// Build a complete instruction with the given data.
                        pub fn to_ix(&self, data: &#data_ty) -> ::solana_sdk::instruction::Instruction {
                            extern crate alloc;
                            let mut instruction_data = alloc::vec![super::#enum_name::#variant_ident as u8];
                            instruction_data.extend_from_slice(::bytemuck::bytes_of(data));
                            ::solana_sdk::instruction::Instruction {
                                program_id: ::solana_sdk::pubkey::Pubkey::new_from_array(crate::ID),
                                accounts: self.to_account_metas(),
                                data: instruction_data,
                            }
                        }
                    }
                }
            } else {
                // No data
                quote! {
                    impl #input_type {
                        /// Build a complete instruction (no data).
                        pub fn to_ix(&self) -> ::solana_sdk::instruction::Instruction {
                            extern crate alloc;
                            let instruction_data = alloc::vec![super::#enum_name::#variant_ident as u8];
                            ::solana_sdk::instruction::Instruction {
                                program_id: ::solana_sdk::pubkey::Pubkey::new_from_array(crate::ID),
                                accounts: self.to_account_metas(),
                                data: instruction_data,
                            }
                        }
                    }
                }
            };

            Some(to_ix_impl)
        })
        .collect();

    quote! {
        /// Client module for building instructions with solana-sdk types.
        #[cfg(feature = "solana-sdk")]
        pub mod client {
            use super::*;

            #(#to_ix_impls)*
        }
    }
}
