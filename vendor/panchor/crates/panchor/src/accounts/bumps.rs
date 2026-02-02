//! Bumps trait for PDA bump seeds

/// Associated bump seeds for accounts with PDAs.
///
/// This trait is implemented by types generated via `#[derive(Accounts)]`
/// that have fields with `#[account(pda = ...)]` constraints.
pub trait Bumps {
    /// Struct to hold account bump seeds.
    type Bumps: Sized;
}
