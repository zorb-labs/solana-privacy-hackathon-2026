//! Type utility functions for panchor-derive macros

use syn::{Attribute, LitStr, Type};

/// Check if a type is `u64`
pub fn is_u64_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "u64";
    }
    false
}

/// Parse the `#[seeds("...")]` attribute from a list of attributes
pub fn extract_seeds_attr(attrs: &[Attribute]) -> Option<LitStr> {
    for attr in attrs {
        if attr.path().is_ident("seeds")
            && let Ok(lit) = attr.parse_args::<LitStr>()
        {
            return Some(lit);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_is_u64_type() {
        let u64_type: Type = parse_quote!(u64);
        assert!(is_u64_type(&u64_type));

        let pubkey_type: Type = parse_quote!(Pubkey);
        assert!(!is_u64_type(&pubkey_type));

        let bytes_type: Type = parse_quote!([u8; 32]);
        assert!(!is_u64_type(&bytes_type));
    }
}
