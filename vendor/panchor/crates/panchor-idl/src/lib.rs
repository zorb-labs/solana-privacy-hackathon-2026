//! IDL types for Panchor-based Solana programs.
//!
//! This crate provides IDL types that extend anchor-lang-idl-spec with
//! additional Panchor-specific features like PDA definitions.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

// Re-export all anchor-lang-idl-spec types
pub use anchor_lang_idl_spec::*;

// ============================================================================
// Panchor IDL Extension Types
// ============================================================================

/// Extended IDL that includes Panchor-specific fields like PDAs.
///
/// This wraps the standard Anchor IDL and adds a `pdas` field.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PanchorIdl {
    pub address: String,
    pub metadata: IdlMetadata,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub docs: Vec<String>,
    pub instructions: Vec<IdlInstruction>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub accounts: Vec<IdlAccount>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub events: Vec<IdlEvent>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub errors: Vec<IdlErrorCode>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub types: Vec<IdlTypeDef>,
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub constants: Vec<IdlConst>,
    /// PDA definitions for this program
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub pdas: Vec<IdlPdaDefinition>,
}

/// A PDA definition in the IDL.
///
/// Describes a Program Derived Address with its seeds and documentation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct IdlPdaDefinition {
    /// The name of the PDA (e.g., "Pool", "Stake")
    pub name: String,
    /// Documentation describing the PDA and its purpose
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    pub docs: Vec<String>,
    /// The seeds used to derive this PDA
    pub seeds: Vec<IdlSeed>,
}

// ============================================================================
// Build-time types for IDL generation
// ============================================================================

// All types are re-exported from anchor-lang-idl-spec:
// - Idl: the root IDL type
// - IdlInstruction: for instructions (name, docs, discriminator, accounts, args, returns)
// - IdlInstructionAccount: for instruction account metadata
// - IdlAccount: for account type definitions (name + discriminator)
// - IdlEvent: for event definitions (name + discriminator)
// - IdlErrorCode: for error definitions (code, name, msg)
// - IdlTypeDef: for type definitions
// - IdlField: for struct fields

// ============================================================================
// Utility functions
// ============================================================================

/// Convert a 32-byte pubkey to base58 string (no_std compatible).
pub fn pubkey_to_base58(key: &[u8; 32]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    let leading_zeros = key.iter().take_while(|&&b| b == 0).count();
    let mut digits: Vec<u8> = Vec::new();

    for &byte in key.iter() {
        let mut carry = u32::from(byte);
        for digit in &mut digits {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }

    let mut result = String::with_capacity(leading_zeros + digits.len());
    for _ in 0..leading_zeros {
        result.push('1');
    }
    for digit in digits.into_iter().rev() {
        result.push(ALPHABET[digit as usize] as char);
    }

    result
}

/// Helper to create an IdlType::Array with a value length.
pub fn idl_array(inner: IdlType, len: usize) -> IdlType {
    IdlType::Array(Box::new(inner), IdlArrayLen::Value(len))
}

/// Convert a Rust type string to anchor IdlType.
pub fn rust_type_to_idl_type(ty: &str) -> IdlType {
    let ty = ty.trim();

    match ty {
        "u8" => IdlType::U8,
        "u16" => IdlType::U16,
        "u32" => IdlType::U32,
        "u64" | "usize" => IdlType::U64,
        "u128" => IdlType::U128,
        "i8" => IdlType::I8,
        "i16" => IdlType::I16,
        "i32" => IdlType::I32,
        "i64" | "isize" => IdlType::I64,
        "i128" => IdlType::I128,
        "f32" => IdlType::F32,
        "f64" => IdlType::F64,
        "bool" => IdlType::Bool,
        "Pubkey" | "pubkey::Pubkey" | "[u8; 32]" | "[u8;32]" => IdlType::Pubkey,
        "String" | "string" | "&str" => IdlType::String,
        s if s.starts_with("&") && s.contains("str") => IdlType::String,
        s if s.starts_with("Vec<") && s.ends_with('>') => {
            let inner = &s[4..s.len() - 1];
            IdlType::Vec(Box::new(rust_type_to_idl_type(inner)))
        }
        s if s.starts_with("Option<") && s.ends_with('>') => {
            let inner = &s[7..s.len() - 1];
            IdlType::Option(Box::new(rust_type_to_idl_type(inner)))
        }
        s if s.starts_with('[') && s.ends_with(']') => {
            if let Some(semi_pos) = s.rfind(';') {
                let inner = &s[1..semi_pos];
                let len_str = s[semi_pos + 1..s.len() - 1].trim();
                if let Ok(len) = len_str.parse::<usize>() {
                    return IdlType::Array(
                        Box::new(rust_type_to_idl_type(inner)),
                        IdlArrayLen::Value(len),
                    );
                }
            }
            IdlType::Defined {
                name: s.to_string(),
                generics: vec![],
            }
        }
        _ => IdlType::Defined {
            name: ty.to_string(),
            generics: vec![],
        },
    }
}

// ============================================================================
// Build traits
// ============================================================================

/// Trait for types that can provide IDL argument metadata.
pub trait IdlBuildArgs {
    fn __idl_args() -> Vec<IdlField>;
}

/// Trait for types that can provide their IDL type definition.
pub trait IdlBuildType {
    fn __idl_type_def() -> IdlTypeDef;
}

/// Trait for account types that can provide their IDL metadata.
pub trait IdlBuildAccount {
    fn __idl_account_name() -> &'static str;
    fn __idl_account_discriminator() -> u64;
    fn __idl_account_docs() -> Vec<String>;
}

/// Trait for error enums that can provide their IDL metadata.
pub trait IdlBuildErrors {
    fn __idl_errors() -> Vec<IdlErrorCode>;
}

/// Trait for event types that can provide their IDL metadata.
pub trait IdlBuildEvent {
    fn __idl_event_name() -> &'static str;
    fn __idl_event_discriminator() -> u64;
    fn __idl_event_docs() -> Vec<String>;
}
