//! `PdaAccount` traits for associating accounts with their PDA definitions
//!
//! These traits provide a way to get the PDA enum variant and bump from an account.

/// Trait for accounts that are associated with a PDA definition.
///
/// This trait allows getting the PDA enum variant from an account's data.
/// The account struct must have fields that match the PDA variant's seeds.
///
/// # Example
///
/// ```ignore
/// // With #[account(MinesAccount::Miner, pda = MinesPdas::Miner(mine, authority))]
/// impl PdaAccount for Miner {
///     type Pdas = MinesPdas;
///     fn pda_seed_args(&self) -> MinesPdas {
///         MinesPdas::Miner { mine: self.mine, authority: self.authority }
///     }
/// }
/// ```
pub trait PdaAccount {
    /// The PDA enum type (e.g., `MinesPdas`)
    type Pdas;

    /// Returns the PDA enum variant for this account.
    fn pda_seed_args(&self) -> Self::Pdas;
}

/// Trait for accounts that can provide their PDA and bump seed.
///
/// This trait extends `PdaAccount` by also returning the bump seed.
/// - For accounts with `#[account(..., bump)]`, uses the stored bump field
/// - For accounts without bump, calls `find_program_address` to calculate it
///
/// # Example
///
/// ```ignore
/// // With bump field stored:
/// impl PdaAccountWithBump for Miner {
///     type Pdas = MinesPdas;
///     fn pda_seed_args_with_bump(&self) -> (MinesPdas, u8) {
///         (self.pda_seed_args(), self.bump)
///     }
/// }
///
/// // Without bump field (calculates via find_program_address):
/// impl PdaAccountWithBump for SomeAccount {
///     type Pdas = MinesPdas;
///     fn pda_seed_args_with_bump(&self) -> (MinesPdas, u8) {
///         let (_, bump) = find_some_pda(&self.field1, &self.field2);
///         (self.pda_seed_args(), bump)
///     }
/// }
/// ```
pub trait PdaAccountWithBump {
    /// The PDA enum type (e.g., `MinesPdas`)
    type Pdas;

    /// Returns the PDA enum variant and its bump seed.
    fn pda_seed_args_with_bump(&self) -> (Self::Pdas, u8);
}
