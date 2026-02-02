//! `InstructionDispatch` derive macro
//!
//! Generates the `InstructionDispatch` trait implementation for instruction enum types.
//! This derive is automatically added by the `#[instructions]` attribute macro.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Data, DeriveInput, Error, Expr, Ident, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
};

use crate::utils::to_snake_case;

/// Parsed handler attribute from #[handler(...)] on enum variants
pub struct HandlerAttr {
    pub processor: Expr,
    pub data: Option<Expr>,
    pub accounts: Option<Expr>,
    /// If true, pass raw &[u8] to processor instead of parsed data
    pub raw_data: bool,
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
    /// IDL args type (ignored for dispatch, only used for IDL generation)
    IdlArgs,
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
            "idl_args" => Ok(Self::IdlArgs),
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
            });
        }

        let params: Punctuated<HandlerParam, Token![,]> = Punctuated::parse_terminated(input)?;

        let mut processor = None;
        let mut data = None;
        let mut accounts = None;
        let mut use_data_shorthand = false;
        let mut raw_data = false;

        for param in params {
            match param {
                HandlerParam::Processor(expr) => processor = Some(expr),
                HandlerParam::Data(expr) => data = Some(expr),
                HandlerParam::Accounts(expr) => accounts = Some(expr),
                HandlerParam::DataShorthand => use_data_shorthand = true,
                HandlerParam::RawData => raw_data = true,
                HandlerParam::IdlArgs => {} // Ignored for dispatch (only used for IDL)
            }
        }

        Ok(Self {
            processor,
            data,
            accounts,
            use_data_shorthand,
            raw_data,
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
    let accounts = if parsed.accounts.is_some() {
        parsed.accounts
    } else {
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
    }
}

/// Information about a single enum variant for dispatch
struct VariantDispatchInfo {
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
        },
        variant_name,
    ))
}

/// Core implementation for `InstructionDispatch` derive macro
pub fn derive_instruction_dispatch_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;

    // Get enum variants
    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => {
            return Error::new_spanned(
                &input.ident,
                "InstructionDispatch can only be derived for enums",
            )
            .to_compile_error();
        }
    };

    // Parse handler attributes from each variant
    let mut variant_infos: Vec<VariantDispatchInfo> = Vec::new();
    for variant in variants {
        match parse_handler_attr(&variant.attrs, &variant.ident) {
            Ok(attr) => {
                variant_infos.push(VariantDispatchInfo {
                    ident: variant.ident.clone(),
                    attr,
                });
            }
            Err(e) => return e.to_compile_error(),
        }
    }

    // Generate match arms for dispatch
    let match_arms: Vec<TokenStream2> = variant_infos
        .iter()
        .map(|info| {
            let variant = &info.ident;
            let attr = &info.attr;
            let processor = &attr.processor;

            // Generate the match arm based on what's available
            // All accounts with try_into_context return ParseResult to support init_idempotent
            match (&attr.accounts, &attr.data, attr.raw_data) {
                (Some(accounts_type), Some(data_type), false) => {
                    // Both accounts and data - parse both and pass Context
                    quote! {
                        Self::#variant => {
                            match <#accounts_type>::try_into_context(accounts)? {
                                ::panchor::ParseResult::Parsed(parsed) => {
                                    let parsed_data: #data_type = ::panchor::parse_instruction_data(data)?;
                                    #processor(parsed.as_context(), parsed_data)
                                }
                                ::panchor::ParseResult::SkipIdempotent => Ok(()),
                            }
                        }
                    }
                }
                (Some(accounts_type), _, true) => {
                    // Accounts with raw data - parse accounts, pass Context and raw data
                    quote! {
                        Self::#variant => {
                            match <#accounts_type>::try_into_context(accounts)? {
                                ::panchor::ParseResult::Parsed(parsed) => {
                                    #processor(parsed.as_context(), data)
                                }
                                ::panchor::ParseResult::SkipIdempotent => Ok(()),
                            }
                        }
                    }
                }
                (Some(accounts_type), None, false) => {
                    // Only accounts, no data - parse accounts and pass Context only
                    quote! {
                        Self::#variant => {
                            match <#accounts_type>::try_into_context(accounts)? {
                                ::panchor::ParseResult::Parsed(parsed) => {
                                    #processor(parsed.as_context())
                                }
                                ::panchor::ParseResult::SkipIdempotent => Ok(()),
                            }
                        }
                    }
                }
                (None, Some(data_type), false) => {
                    // Only data, no accounts (unusual but supported)
                    quote! {
                        Self::#variant => {
                            let parsed_data: #data_type = ::panchor::parse_instruction_data(data)?;
                            #processor(accounts, parsed_data)
                        }
                    }
                }
                (None, _, true) | (None, None, false) => {
                    // Neither accounts nor data, or raw_data without accounts - legacy raw call
                    quote! {
                        Self::#variant => #processor(accounts, data),
                    }
                }
            }
        })
        .collect();

    quote! {
        impl ::panchor::InstructionDispatch for #name {
            fn dispatch(
                &self,
                accounts: &[::panchor::pinocchio::account_info::AccountInfo],
                data: &[u8],
            ) -> ::panchor::pinocchio::ProgramResult {
                match self {
                    #(#match_arms)*
                    _ => Err(::panchor::pinocchio::program_error::ProgramError::InvalidInstructionData),
                }
            }
        }
    }
}
