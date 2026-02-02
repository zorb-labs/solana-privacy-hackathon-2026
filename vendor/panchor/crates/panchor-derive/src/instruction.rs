//! Instruction attribute macro
//!
//! Marks a struct as an instruction accounts definition and associates it
//! with a discriminator from the instruction enum.

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{DeriveInput, Path, Result, parse::Parse, parse::ParseStream};

/// Arguments for the instruction attribute macro
pub struct InstructionArgs {
    /// The instruction variant path (e.g., `MinesInstruction::Deploy`)
    pub instruction_variant: Path,
}

impl Parse for InstructionArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let instruction_variant: Path = input.parse()?;
        Ok(Self {
            instruction_variant,
        })
    }
}

/// Core implementation for instruction attribute macro
pub fn instruction_impl(args: InstructionArgs, mut input: DeriveInput) -> TokenStream2 {
    let name = &input.ident;
    let instruction_variant = &args.instruction_variant;

    // Extract the enum type and variant from the path (e.g., MinesInstruction::Deploy)
    let segments: Vec<_> = instruction_variant.segments.iter().collect();
    if segments.len() != 2 {
        return syn::Error::new_spanned(
            instruction_variant,
            "Expected EnumType::Variant syntax (e.g., MinesInstruction::Deploy)",
        )
        .to_compile_error();
    }

    // Add Accounts derive if not already present
    let has_derive = input.attrs.iter().any(|attr| {
        if let syn::Meta::List(meta_list) = &attr.meta
            && meta_list.path.is_ident("derive")
        {
            let tokens = meta_list.tokens.to_string();
            return tokens.contains("Accounts");
        }
        false
    });

    if !has_derive {
        let derive: syn::Attribute = syn::parse_quote! {
            #[derive(Accounts)]
        };
        input.attrs.insert(0, derive);
    }

    quote! {
        #input

        impl<'info> #name<'info> {
            /// Compile-time assertion that instruction discriminant fits in u8 (0-255)
            const _: () = {
                let disc = #instruction_variant as usize;
                assert!(disc <= 255, "instruction discriminant exceeds u8::MAX (255)");
            };

            /// The instruction discriminator byte
            pub const DISCRIMINATOR: u8 = #instruction_variant as u8;

            /// The instruction variant
            pub const INSTRUCTION: crate::MinesInstruction = #instruction_variant;
        }
    }
}
