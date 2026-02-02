//! Field kind detection for the Accounts derive macro.

use syn::{Field, GenericArgument, Path, PathArguments, Type};

/// The kind of field type we detected
#[derive(Clone)]
pub enum FieldKind {
    /// Raw `&'info AccountInfo` - uses #[account(...)] attributes
    RawAccountInfo,
    /// `AccountLoader<'info, T>` - zero-copy account loader that validates owner, discriminator, size via `TryFrom`
    AccountLoader(Path),
    /// `LazyAccount<'info, T>` - lazy account wrapper that validates owner/discriminator at construction, deserializes on demand
    LazyAccount(Path),
    /// `Signer<'info>` - validates `is_signer` via `TryFrom`
    Signer,
    /// `Program<'info, T>` - validates `is_executable` and program ID via `TryFrom`
    /// The Path is the program type T (e.g., System) for IDL generation
    Program(Path),
}

/// Detect the field kind from its type
pub fn detect_field_kind(field: &Field) -> FieldKind {
    // Check if it's a path type (AccountLoader<T>, LazyAccount<T>, Signer, or Program<T>)
    if let Type::Path(type_path) = &field.ty {
        let segments = &type_path.path.segments;
        if let Some(last_segment) = segments.last() {
            let ident_str = last_segment.ident.to_string();

            // Check for AccountLoader<'info, T>
            if ident_str == "AccountLoader"
                && let PathArguments::AngleBracketed(args) = &last_segment.arguments
            {
                // Find the type argument (skip lifetime)
                for arg in &args.args {
                    if let GenericArgument::Type(Type::Path(inner_path)) = arg {
                        return FieldKind::AccountLoader(inner_path.path.clone());
                    }
                }
            }

            // Check for LazyAccount<'info, T>
            if ident_str == "LazyAccount"
                && let PathArguments::AngleBracketed(args) = &last_segment.arguments
            {
                // Find the type argument (skip lifetime)
                for arg in &args.args {
                    if let GenericArgument::Type(Type::Path(inner_path)) = arg {
                        return FieldKind::LazyAccount(inner_path.path.clone());
                    }
                }
            }

            // Check for Program<'info, T>
            if ident_str == "Program"
                && let PathArguments::AngleBracketed(args) = &last_segment.arguments
            {
                // Find the type argument (skip lifetime) for IDL generation
                for arg in &args.args {
                    if let GenericArgument::Type(Type::Path(inner_path)) = arg {
                        return FieldKind::Program(inner_path.path.clone());
                    }
                }
            }

            // Check for Signer<'info>
            if ident_str == "Signer" {
                return FieldKind::Signer;
            }
        }
    }

    // Default to raw AccountInfo
    FieldKind::RawAccountInfo
}

/// Get the account type from the field kind (`AccountLoader`<'info, T> or `LazyAccount`<'info, T>)
pub fn get_account_type(field_kind: &FieldKind) -> Option<&Path> {
    match field_kind {
        FieldKind::AccountLoader(path) | FieldKind::LazyAccount(path) => Some(path),
        _ => None,
    }
}
