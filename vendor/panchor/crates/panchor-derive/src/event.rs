//! Event attribute macro

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{DeriveInput, Path, Token, parse::Parse, parse::ParseStream, parse_quote};

use crate::utils::extract_docs;

/// Arguments for the event attribute macro
pub struct EventArgs {
    /// The event type path (e.g., `EventType::Bury`)
    event_type: Path,
    /// Whether to skip deriving `EventLog` (for events with custom log implementations)
    no_log: bool,
}

impl Parse for EventArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let event_type: Path = input.parse()?;

        let mut no_log = false;

        while !input.is_empty() {
            input.parse::<Token![,]>()?;
            let ident: syn::Ident = input.parse()?;
            if ident == "no_log" {
                no_log = true;
            } else {
                return Err(syn::Error::new(ident.span(), "expected `no_log`"));
            }
        }

        Ok(Self { event_type, no_log })
    }
}

/// Core implementation for event attribute macro
pub fn event_impl(args: EventArgs, mut input: DeriveInput) -> TokenStream2 {
    // Extract the enum type and variant from the path (e.g., EventType::Bury)
    let segments: Vec<_> = args.event_type.segments.iter().collect();
    if segments.len() != 2 {
        return syn::Error::new_spanned(
            &args.event_type,
            "Expected EventType::Variant syntax (e.g., EventType::Bury)",
        )
        .to_compile_error();
    }
    let enum_type = &segments[0].ident;
    let variant = &segments[1].ident;

    // Extract docs from the struct
    let docs = extract_docs(&input.attrs);

    // Add derives to the struct
    // Note: Pod, Zeroable, EventLog, and IdlType must be in scope (via panchor::prelude::*)
    // Note: Debug is intentionally omitted to reduce binary size
    let derives: syn::Attribute = if args.no_log {
        parse_quote! {
            #[derive(Clone, Copy, Pod, Zeroable, ::panchor::IdlType)]
        }
    } else {
        parse_quote! {
            #[derive(Clone, Copy, Pod, Zeroable, EventLog, ::panchor::IdlType)]
        }
    };
    input.attrs.insert(0, derives);

    let name = &input.ident;
    let name_str = name.to_string();

    // Generate docs expression for IDL
    let docs_expr = if docs.is_empty() {
        quote! { ::alloc::vec::Vec::new() }
    } else {
        quote! { ::alloc::vec![#(#docs.to_string()),*] }
    };

    // Generate test module name
    let test_mod_name = format_ident!("__idl_event_{}", name_str.to_lowercase());

    // Generate the trait implementations
    quote! {
        #input

        impl panchor::Discriminator for #name {
            const DISCRIMINATOR: u64 = #enum_type::#variant as u64;
        }

        impl panchor::Event for #name {
            fn name() -> &'static str {
                #enum_type::#variant.into()
            }
        }

        #[cfg(feature = "idl-build")]
        impl ::panchor::panchor_idl::IdlBuildEvent for #name {
            fn __idl_event_name() -> &'static str {
                #name_str
            }

            fn __idl_event_discriminator() -> u64 {
                #enum_type::#variant as u64
            }

            fn __idl_event_docs() -> ::alloc::vec::Vec<::alloc::string::String> {
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
            fn __idl_build_event() {
                use ::panchor::panchor_idl::IdlBuildEvent;
                let event_def = ::panchor::panchor_idl::IdlEvent {
                    name: <#name as IdlBuildEvent>::__idl_event_name().to_string(),
                    discriminator: {
                        let disc = <#name as IdlBuildEvent>::__idl_event_discriminator();
                        disc.to_le_bytes().to_vec()
                    },
                };
                let json = ::serde_json::to_string_pretty(&event_def).expect("Failed to serialize event");
                std::println!("--- IDL event {} ---", #name_str);
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
        let args = syn::parse2::<EventArgs>(attr).unwrap();
        let input = syn::parse2::<DeriveInput>(input).unwrap();
        event_impl(args, input)
    }

    #[test]
    fn test_event_attribute_basic() {
        let attr = quote!(EventType::Bury);
        let input = quote! {
            #[repr(C)]
            pub struct BuryEvent {
                pub mine: Pubkey,
                pub amount: u64,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check that derives were added (Debug intentionally omitted to reduce binary size)
        assert!(output_str.contains("derive"));
        assert!(output_str.contains("Clone"));
        assert!(output_str.contains("Copy"));
        assert!(!output_str.contains("Debug"));
        assert!(output_str.contains("Pod"));
        assert!(output_str.contains("Zeroable"));
        assert!(output_str.contains("EventLog"));

        // Check trait implementations
        assert!(output_str.contains("impl panchor :: Discriminator for BuryEvent"));
        assert!(output_str.contains("const DISCRIMINATOR : u64 = EventType :: Bury as u64"));
        assert!(output_str.contains("impl panchor :: Event for BuryEvent"));
        assert!(output_str.contains("fn name () -> & 'static str"));
        assert!(output_str.contains("EventType :: Bury . into ()"));
    }

    #[test]
    fn test_event_attribute_no_log() {
        let attr = quote!(EventType::Checkpoint, no_log);
        let input = quote! {
            #[repr(C)]
            pub struct CheckpointEvent {
                pub amount: u64,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check that EventLog is NOT derived
        assert!(output_str.contains("Pod"));
        assert!(output_str.contains("Zeroable"));
        assert!(!output_str.contains("EventLog"));
    }

    #[test]
    fn test_event_attribute_preserves_attrs() {
        let attr = quote!(EventType::Close);
        let input = quote! {
            /// My event docs
            #[repr(C)]
            pub struct CloseEvent {
                pub amount: u64,
            }
        };

        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Check that doc comments are preserved
        assert!(output_str.contains("My event docs"));
        assert!(output_str.contains("repr (C)"));
    }

    #[test]
    fn test_event_attribute_invalid_path_returns_compile_error() {
        let attr = quote!(Bury);
        let input = quote! {
            pub struct BuryEvent {
                pub amount: u64,
            }
        };
        let output = parse_and_expand(attr, input);
        let output_str = output.to_string();

        // Should contain a compile error, not panic
        assert!(
            output_str.contains("compile_error"),
            "expected compile_error in output: {output_str}"
        );
    }
}
