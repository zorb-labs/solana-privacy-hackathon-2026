//! PDA constraint handling for the Accounts derive macro.

use syn::{Expr, Ident};

/// PDA constraint parsed from `pda = Variant, pda::field1 = expr1, pda::field2 = expr2`.
///
/// This generates a call to `crate::pda::find_{variant}_pda(expr1, expr2, ...)`
/// for validation, avoiding any struct allocation.
#[derive(Clone)]
pub struct PdaConstraint {
    /// The variant name (e.g., "Miner", "Round")
    pub variant: Ident,
    /// Field assignments in order: [(`field_name`, expr), ...]
    pub fields: Vec<(Ident, Expr)>,
}
