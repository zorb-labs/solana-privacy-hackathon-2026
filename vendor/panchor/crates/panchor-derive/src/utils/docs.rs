//! Documentation extraction utilities for panchor-derive macros

use syn::{Attribute, Expr, Meta};

/// Extract doc comments from attributes as a vector of strings.
///
/// This function filters the attributes to find `#[doc = "..."]` attributes
/// and extracts the documentation text, trimming whitespace.
pub fn extract_docs(attrs: &[Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if let Meta::NameValue(meta) = &attr.meta
                && meta.path.is_ident("doc")
                && let Expr::Lit(expr_lit) = &meta.value
                && let syn::Lit::Str(lit_str) = &expr_lit.lit
            {
                return Some(lit_str.value().trim().to_string());
            }
            None
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Extract the first doc comment from attributes.
///
/// Returns `Some(doc)` if there's at least one non-empty doc comment,
/// `None` otherwise.
pub fn extract_doc(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if let Meta::NameValue(meta) = &attr.meta
            && meta.path.is_ident("doc")
            && let Expr::Lit(expr_lit) = &meta.value
            && let syn::Lit::Str(lit_str) = &expr_lit.lit
        {
            let doc = lit_str.value().trim().to_string();
            if !doc.is_empty() {
                return Some(doc);
            }
        }
        None
    })
}
