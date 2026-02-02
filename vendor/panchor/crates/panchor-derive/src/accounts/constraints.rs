//! Constraint parsing for the Accounts derive macro.

use syn::{
    Error, Expr, Field, Ident, Result, Token,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
};

use super::pda::PdaConstraint;

/// Parsed constraints from `#[account(...)]` attribute on a field
#[derive(Default, Clone)]
pub struct AccountConstraints {
    pub signer: bool,
    pub mutable: bool,
    pub init: bool,
    /// Init only if account is empty, return early if already initialized
    pub init_idempotent: bool,
    /// Check address matches `T::ID` from the Id trait
    pub id: bool,
    /// Check that the account is executable (for program accounts that aren't typed)
    pub exec: bool,
    /// Check that the account is empty (zero data length)
    pub zero: bool,
    pub program: Option<Expr>,
    pub address: Option<Expr>,
    /// Owner expression for custom owner validation
    pub owner: Option<Expr>,
    /// Seeds for PDA derivation (required if init is set)
    pub seeds: Option<Vec<Expr>>,
    /// Payer account name (required if init is set)
    pub payer: Option<Ident>,
    /// Bump seed for PDA derivation (optional, derived if not specified)
    pub bump: Option<Ident>,
    /// PDA constraint for automatic validation via find_*_pda
    pub pda: Option<PdaConstraint>,
    /// Skip PDA bump derivation. The bump won't be derived and PDA address won't be validated.
    /// The pda constraint will only be used for generating the IDL (documenting the PDA structure).
    pub skip_pda_derivation: bool,
}

/// Parse a single constraint like `signer`, `mut`, `init`, `init_idempotent`, `id`, `exec`, `zero`,
/// `program = expr`, `address = expr`, `owner = expr`, `seeds = [...]`, `payer = field`,
/// `bump = field`, `pda = Variant`, `pda::field = expr`, `skip_pda_derivation`
pub enum Constraint {
    Signer,
    Mutable,
    Init,
    /// Init only if account is empty, return early if already initialized
    InitIdempotent,
    /// Check address matches `T::ID` from the Id trait
    Id,
    /// Check that the account is executable
    Exec,
    /// Check that the account is empty (zero data length)
    Zero,
    Program(Expr),
    Address(Expr),
    /// Custom owner validation
    Owner(Expr),
    Seeds(Vec<Expr>),
    Payer(Ident),
    Bump(Ident),
    /// PDA variant name: `pda = Miner`
    PdaVariant(Ident),
    /// PDA field assignment: `pda::mine = expr`
    PdaField(Ident, Expr),
    /// Skip PDA bump derivation (pda constraint only used for IDL generation)
    SkipPdaDerivation,
}

impl Parse for Constraint {
    fn parse(input: ParseStream) -> Result<Self> {
        // Handle `mut` keyword specially since it's a Rust keyword, not an identifier
        if input.peek(Token![mut]) {
            input.parse::<Token![mut]>()?;
            return Ok(Self::Mutable);
        }

        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "signer" => Ok(Self::Signer),
            "init" => Ok(Self::Init),
            "init_idempotent" => Ok(Self::InitIdempotent),
            "id" => Ok(Self::Id),
            "exec" => Ok(Self::Exec),
            "zero" => Ok(Self::Zero),
            "program" => {
                input.parse::<Token![=]>()?;
                let expr: Expr = input.parse()?;
                Ok(Self::Program(expr))
            }
            "address" => {
                input.parse::<Token![=]>()?;
                let expr: Expr = input.parse()?;
                Ok(Self::Address(expr))
            }
            "owner" => {
                input.parse::<Token![=]>()?;
                let expr: Expr = input.parse()?;
                Ok(Self::Owner(expr))
            }
            "seeds" => {
                input.parse::<Token![=]>()?;
                let content;
                syn::bracketed!(content in input);
                let seeds: Punctuated<Expr, Comma> = Punctuated::parse_terminated(&content)?;
                Ok(Self::Seeds(seeds.into_iter().collect()))
            }
            "payer" => {
                input.parse::<Token![=]>()?;
                let payer_ident: Ident = input.parse()?;
                Ok(Self::Payer(payer_ident))
            }
            "bump" => {
                input.parse::<Token![=]>()?;
                let bump_ident: Ident = input.parse()?;
                Ok(Self::Bump(bump_ident))
            }
            "skip_pda_derivation" => Ok(Self::SkipPdaDerivation),
            "pda" => {
                // Check for pda::field vs pda = Variant
                if input.peek(Token![::]) {
                    input.parse::<Token![::]>()?;
                    let field_name: Ident = input.parse()?;
                    // pda::field = expr
                    input.parse::<Token![=]>()?;
                    let expr: Expr = input.parse()?;
                    return Ok(Self::PdaField(field_name, expr));
                }

                // pda = Variant
                input.parse::<Token![=]>()?;
                let variant: Ident = input.parse()?;
                Ok(Self::PdaVariant(variant))
            }
            _ => Err(Error::new(
                ident.span(),
                format!(
                    "Unknown constraint: {ident}. Expected signer, mut, init, init_idempotent, id, exec, zero, program, address, owner, seeds, payer, bump, pda, or skip_pda_derivation"
                ),
            )),
        }
    }
}

/// Parse the contents of #[account(...)]
pub struct AccountAttr {
    pub constraints: Punctuated<Constraint, Comma>,
}

impl Parse for AccountAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            constraints: Punctuated::parse_terminated(input)?,
        })
    }
}

/// Extract constraints from a field's attributes
pub fn parse_field_constraints(field: &Field) -> Result<AccountConstraints> {
    let mut result = AccountConstraints::default();

    // Collect PDA parts separately
    let mut pda_variant: Option<Ident> = None;
    let mut pda_fields: Vec<(Ident, Expr)> = Vec::new();

    for attr in &field.attrs {
        if attr.path().is_ident("account") {
            let parsed: AccountAttr = attr.parse_args()?;
            for constraint in parsed.constraints {
                match constraint {
                    Constraint::Signer => result.signer = true,
                    Constraint::Mutable => result.mutable = true,
                    Constraint::Init => result.init = true,
                    Constraint::InitIdempotent => result.init_idempotent = true,
                    Constraint::Id => result.id = true,
                    Constraint::Exec => result.exec = true,
                    Constraint::Zero => result.zero = true,
                    Constraint::Program(expr) => result.program = Some(expr),
                    Constraint::Address(expr) => result.address = Some(expr),
                    Constraint::Owner(expr) => result.owner = Some(expr),
                    Constraint::Seeds(seeds) => result.seeds = Some(seeds),
                    Constraint::Payer(payer) => result.payer = Some(payer),
                    Constraint::Bump(bump) => result.bump = Some(bump),
                    Constraint::PdaVariant(variant) => pda_variant = Some(variant),
                    Constraint::PdaField(name, expr) => pda_fields.push((name, expr)),
                    Constraint::SkipPdaDerivation => result.skip_pda_derivation = true,
                }
            }
        }
    }

    let span = field
        .ident
        .as_ref()
        .map_or_else(proc_macro2::Span::call_site, proc_macro2::Ident::span);

    // Build PDA constraint if variant is specified
    if let Some(variant) = pda_variant {
        result.pda = Some(PdaConstraint {
            variant,
            fields: pda_fields,
        });
    } else if !pda_fields.is_empty() {
        return Err(Error::new(
            span,
            "pda::field constraints require pda = Variant to be specified first",
        ));
    }

    // Validate that init/init_idempotent and mut are mutually exclusive
    if (result.init || result.init_idempotent) && result.mutable {
        return Err(Error::new(
            span,
            "`init`/`init_idempotent` and `mut` are mutually exclusive. Use `init` for account creation (implies writable)",
        ));
    }

    // Validate that init and init_idempotent are mutually exclusive
    if result.init && result.init_idempotent {
        return Err(Error::new(
            span,
            "`init` and `init_idempotent` are mutually exclusive. Use one or the other",
        ));
    }

    // Validate that init/init_idempotent requires (seeds or pda) and payer
    if result.init || result.init_idempotent {
        if result.seeds.is_none() && result.pda.is_none() {
            return Err(Error::new(
                span,
                "`init`/`init_idempotent` requires `seeds = [...]` or `pda = <Variant>` for PDA derivation",
            ));
        }
        if result.payer.is_none() {
            return Err(Error::new(
                span,
                "`init`/`init_idempotent` requires `payer = <account>` to pay for account creation",
            ));
        }
    }

    // Validate that seeds and pda are mutually exclusive
    if result.seeds.is_some() && result.pda.is_some() {
        return Err(Error::new(
            span,
            "`seeds` and `pda` constraints are mutually exclusive. Use one or the other",
        ));
    }

    Ok(result)
}
