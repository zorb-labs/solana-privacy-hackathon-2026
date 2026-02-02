//! `EventLog` derive macro

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Data, DeriveInput, Fields, Type};

/// Field attribute configuration for `EventLog`
#[derive(Default)]
struct FieldConfig {
    skip: bool,
    /// Custom formatter that returns a displayable value
    with: Option<syn::Path>,
    /// Custom logger function that handles all logging for this field
    log: Option<syn::Path>,
}

/// Parse field attributes like #[`event_log(skip)`], #[`event_log(with` = func)], or #[`event_log(log` = func)]
fn parse_field_attrs(field: &syn::Field) -> FieldConfig {
    let mut config = FieldConfig::default();

    for attr in &field.attrs {
        if !attr.path().is_ident("event_log") {
            continue;
        }

        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                config.skip = true;
                return Ok(());
            }

            if meta.path.is_ident("with") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                config.with = Some(path);
                return Ok(());
            }

            if meta.path.is_ident("log") {
                let value = meta.value()?;
                let path: syn::Path = value.parse()?;
                config.log = Some(path);
                return Ok(());
            }

            Err(meta.error("expected `skip`, `with = path`, or `log = path`"))
        });
    }

    config
}

/// Core implementation for `EventLog` derive macro
pub fn derive_event_log_impl(input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("EventLog can only be derived for structs with named fields"),
        },
        _ => panic!("EventLog can only be derived for structs"),
    };

    let log_statements: Vec<_> = fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref().unwrap();
            let field_name_str = field_name.to_string();
            let config = parse_field_attrs(field);

            // Skip if explicitly marked or starts with underscore
            if config.skip || field_name_str.starts_with('_') {
                return None;
            }

            let field_label = format!("  {field_name}:");
            let field_format = format!("  {field_name}: {{}}");

            // If custom logger specified (function handles all logging)
            if let Some(func) = config.log {
                return Some(quote! {
                    #func(&self.#field_name);
                });
            }

            // If custom formatter specified (returns displayable value)
            if let Some(func) = config.with {
                return Some(quote! {
                    ::panchor::pinocchio_log::log!(#field_format, #func(&self.#field_name));
                });
            }

            Some(if is_pubkey_type(&field.ty) {
                quote! {
                    ::panchor::pinocchio_log::log!(#field_label);
                    ::panchor::pinocchio::pubkey::log(&self.#field_name);
                }
            } else if is_byte_array_type(&field.ty) {
                // For byte arrays without custom serializer, log as [bytes]
                quote! {
                    ::panchor::pinocchio_log::log!(#field_label);
                    ::panchor::pinocchio_log::log!("  [bytes]");
                }
            } else {
                // Split into two logs: label (no buffer needed) and value (small buffer)
                // This reduces binary size by avoiding 200-byte Logger buffers
                quote! {
                    ::panchor::pinocchio_log::log!(#field_label);
                    ::panchor::pinocchio_log::log!(25, "{}", self.#field_name);
                }
            })
        })
        .collect();

    let name_label = format!("{name}:");

    quote! {
        impl panchor::events::EventLog for #name {
            fn log(&self) {
                ::panchor::pinocchio_log::log!(#name_label);
                #(#log_statements)*
            }
        }
    }
}

fn is_pubkey_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "Pubkey";
    }
    false
}

fn is_byte_array_type(ty: &Type) -> bool {
    if let Type::Array(array) = ty
        && let Type::Path(type_path) = &*array.elem
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "u8";
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    fn parse_and_derive(input: TokenStream2) -> TokenStream2 {
        let input = syn::parse2::<DeriveInput>(input).unwrap();
        derive_event_log_impl(input)
    }

    #[test]
    fn test_simple_struct() {
        let input = quote! {
            pub struct TestEvent {
                pub amount: u64,
                pub count: u32,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("impl panchor :: events :: EventLog for TestEvent"));
        assert!(output_str.contains("fn log"));
        assert!(output_str.contains("\"TestEvent:\""));
        // Split format: label on one line, value with small buffer on next
        assert!(output_str.contains("\"  amount:\""));
        assert!(output_str.contains("\"  count:\""));
        assert!(output_str.contains("log ! (25"));
    }

    #[test]
    fn test_pubkey_field() {
        let input = quote! {
            pub struct TestEvent {
                pub mine: Pubkey,
                pub amount: u64,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("impl panchor :: events :: EventLog for TestEvent"));
        assert!(output_str.contains("\"  mine:\""));
        assert!(output_str.contains("pinocchio :: pubkey :: log"));
        // Split format for non-pubkey field
        assert!(output_str.contains("\"  amount:\""));
    }

    #[test]
    fn test_multiple_pubkeys() {
        let input = quote! {
            pub struct TestEvent {
                pub from: Pubkey,
                pub to: Pubkey,
                pub amount: u64,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"  from:\""));
        assert!(output_str.contains("\"  to:\""));
        // Should have two pubkey log calls
        assert_eq!(output_str.matches("pinocchio :: pubkey :: log").count(), 2);
    }

    #[test]
    fn test_skips_underscore_fields() {
        let input = quote! {
            pub struct TestEvent {
                pub amount: u64,
                pub _padding: [u8; 3],
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        // Split format for amount field
        assert!(output_str.contains("\"  amount:\""));
        assert!(!output_str.contains("_padding"));
    }

    #[test]
    fn test_skip_attribute() {
        let input = quote! {
            pub struct TestEvent {
                pub amount: u64,
                #[event_log(skip)]
                pub secret: u64,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        // Split format for amount field
        assert!(output_str.contains("\"  amount:\""));
        assert!(!output_str.contains("secret"));
    }

    #[test]
    fn test_with_attribute() {
        let input = quote! {
            pub struct TestEvent {
                #[event_log(with = slug_to_string)]
                pub slug: [u8; 32],
                pub amount: u64,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("slug_to_string (& self . slug)"));
        // Split format for amount field
        assert!(output_str.contains("\"  amount:\""));
    }

    #[test]
    fn test_log_attribute() {
        let input = quote! {
            pub struct TestEvent {
                #[event_log(log = log_deployed)]
                pub deployed: [u64; 16],
                pub amount: u64,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        // log attribute just calls the function directly
        assert!(output_str.contains("log_deployed (& self . deployed)"));
        // Split format for amount field
        assert!(output_str.contains("\"  amount:\""));
    }

    #[test]
    fn test_byte_array_field() {
        let input = quote! {
            pub struct TestEvent {
                pub data: [u8; 32],
                pub amount: u64,
            }
        };

        let output = parse_and_derive(input);
        let output_str = output.to_string();

        assert!(output_str.contains("\"  data:\""));
        assert!(output_str.contains("[bytes]"));
        // Split format for amount field
        assert!(output_str.contains("\"  amount:\""));
    }

    #[test]
    fn test_is_pubkey_type() {
        let pubkey_type: Type = syn::parse_quote!(Pubkey);
        assert!(is_pubkey_type(&pubkey_type));

        let u64_type: Type = syn::parse_quote!(u64);
        assert!(!is_pubkey_type(&u64_type));

        let string_type: Type = syn::parse_quote!(String);
        assert!(!is_pubkey_type(&string_type));
    }

    #[test]
    fn test_is_byte_array_type() {
        let byte_array: Type = syn::parse_quote!([u8; 32]);
        assert!(is_byte_array_type(&byte_array));

        let u64_array: Type = syn::parse_quote!([u64; 4]);
        assert!(!is_byte_array_type(&u64_array));

        let u64_type: Type = syn::parse_quote!(u64);
        assert!(!is_byte_array_type(&u64_type));
    }
}
