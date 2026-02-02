//! Derive macros for panchor

mod account;
mod account_type;
mod accounts;
mod constant;
mod error_code;
mod event;
mod event_log;
mod find_program_address;
mod idl_type;
mod instruction;
mod instruction_args;
mod instruction_dispatch;
mod instructions;
mod pdas;
mod program;
mod utils;
mod zero_copy;

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemConst, parse_macro_input};

/// Derive macro for `EventLog` trait
///
/// Generates a `log` method that logs all fields of the event using `pinocchio_log`.
/// Pubkey fields are logged using `pinocchio::pubkey::log`, other fields use the standard `log!` macro.
///
/// ## Field Attributes
///
/// - `#[event_log(skip)]` - Skip this field when logging
/// - `#[event_log(with = func)]` - Use custom function to format this field
///
/// Fields starting with `_` are automatically skipped.
///
/// # Example
///
/// ```ignore
/// use panchor_derive::EventLog;
///
/// #[derive(EventLog)]
/// pub struct MyEvent {
///     pub mine: Pubkey,
///     #[event_log(with = slug_to_str)]
///     pub slug: [u8; 32],
///     #[event_log(skip)]
///     pub _padding: [u8; 3],
/// }
/// ```
#[proc_macro_derive(EventLog, attributes(event_log))]
pub fn derive_event_log(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(event_log::derive_event_log_impl(input))
}

/// Attribute macro for defining events with automatic trait implementations.
///
/// This macro adds `#[derive(Clone, Copy, Debug, Pod, Zeroable, EventLog)]` to the struct
/// and implements `Discriminator` and `Event` traits.
///
/// ## Options
///
/// - `#[event(EventType::Variant)]` - Standard usage with `EventLog` derive
/// - `#[event(EventType::Variant, no_log)]` - Skip `EventLog` derive (for manual implementations)
///
/// # Example
///
/// ```ignore
/// use panchor_derive::event;
///
/// #[event(EventType::Bury)]
/// #[repr(C)]
/// pub struct BuryEvent {
///     pub mine: Pubkey,
///     pub amount: u64,
/// }
///
/// // For events with custom log implementations:
/// #[event(EventType::Checkpoint, no_log)]
/// #[repr(C)]
/// pub struct CheckpointEvent {
///     pub data: [u64; 16],
/// }
///
/// impl panchor::EventLog for CheckpointEvent {
///     fn log(&self) { /* custom implementation */ }
/// }
/// ```
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as event::EventArgs);
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(event::event_impl(args, input))
}

/// Attribute macro for defining account structs with automatic trait implementations.
///
/// This macro adds `#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]`
/// to the struct and implements `Discriminator`, `DataLen`, and `ProgramOwned` traits.
///
/// # Example
///
/// ```ignore
/// use panchor_derive::account;
///
/// #[account(MinesAccount::Automation)]
/// #[repr(C)]
/// pub struct Automation {
///     pub mine: Pubkey,
///     pub authority: Pubkey,
///     pub amount: u64,
/// }
/// ```
#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as account::AccountArgs);
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(account::account_impl(args, input))
}

/// Attribute macro for zero-copy compatible structs.
///
/// This macro adds `#[repr(C)]` and derives the necessary traits for zero-copy
/// deserialization: `Clone`, `Copy`, `PartialEq`, `Eq`, `Pod`, and `Zeroable`.
///
/// Use this for data structs that need to be read directly from account data
/// without copying.
///
/// # Example
///
/// ```ignore
/// use panchor_derive::zero_copy;
///
/// #[zero_copy]
/// pub struct Point {
///     pub x: i32,
///     pub y: i32,
/// }
/// ```
///
/// Expands to:
///
/// ```ignore
/// #[repr(C)]
/// #[derive(Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
/// pub struct Point {
///     pub x: i32,
///     pub y: i32,
/// }
/// ```
#[proc_macro_attribute]
pub fn zero_copy(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(zero_copy::zero_copy_impl(input))
}

/// Derive macro for generating instruction account validation.
///
/// Generates a zero-cost `TryFrom<&[AccountInfo]>` implementation that validates
/// account constraints and destructures accounts into a typed struct.
///
/// ## Constraints
///
/// - `#[account(signer)]` - Account must be a signer
/// - `#[account(mut)]` - Account must be writable
/// - `#[account(account = T)]` - Account must be owned by T's program (via `ProgramOwned` trait)
/// - `#[account(program = expr)]` - Account must be the given program (for CPI)
///
/// Multiple constraints can be combined: `#[account(signer, mut)]`
///
/// ## Documentation
///
/// Doc comments on fields are preserved and can be extracted for IDL generation.
/// Use standard Rust doc comments to document each account.
///
/// # Example
///
/// ```ignore
/// #[instruction(MinesInstruction::Deploy)]
/// pub struct DeployAccounts<'info> {
///     /// Mine account
///     #[account(mut, account = Mine)]
///     pub mine: &'info AccountInfo,
///
///     /// Transaction signer
///     #[account(signer)]
///     pub signer: &'info AccountInfo,
///
///     /// System program for CPI
///     #[account(program = &SYSTEM_PROGRAM_ID)]
///     pub system_program: &'info AccountInfo,
/// }
///
/// // Usage in instruction handler:
/// pub fn process_deploy(
///     accounts: &[AccountInfo],
///     data: &[u8],
/// ) -> ProgramResult {
///     let accounts = DeployAccounts::try_from(accounts)?;
///     // accounts.mine, accounts.signer, etc. are now validated
///     Ok(())
/// }
/// ```
#[proc_macro_derive(Accounts, attributes(account))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(accounts::derive_accounts_impl(input))
}

/// Attribute macro for defining instruction account structs.
///
/// Associates an accounts struct with an instruction discriminator and automatically
/// adds the `Accounts` derive macro.
///
/// # Example
///
/// ```ignore
/// #[instruction(MinesInstruction::Deploy)]
/// pub struct DeployAccounts<'info> {
///     /// Mine account
///     #[account(mut, account = Mine)]
///     pub mine: &'info AccountInfo,
///
///     /// Transaction signer
///     #[account(signer)]
///     pub signer: &'info AccountInfo,
/// }
///
/// // Generated:
/// // - DeployAccounts::DISCRIMINATOR: u8 = 0 (from MinesInstruction::Deploy)
/// // - DeployAccounts::INSTRUCTION: MinesInstruction = MinesInstruction::Deploy
/// // - TryFrom implementation for account validation
/// ```
#[proc_macro_attribute]
pub fn instruction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as instruction::InstructionArgs);
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(instruction::instruction_impl(args, input))
}

/// Attribute macro for instruction enums with automatic dispatch generation.
///
/// This macro adds `TryFromPrimitive` and `AsRefStr` derives and generates a `dispatch`
/// method that routes instructions to their appropriate processors based on
/// `#[handler(...)]` attributes on each variant.
///
/// ## Variant Attributes
///
/// Each variant can have an `#[handler(...)]` attribute with the following parameters:
///
/// - `processor = fn_name` (required) - The processor function to call
/// - `data = DataType` (optional) - The data struct type for parsing (if omitted, processor takes only accounts)
/// - `accounts = AccountsType` (optional) - The accounts struct type for IDL documentation
///
/// ## Example
///
/// ```ignore
/// use panchor_derive::instructions;
///
/// #[instructions]
/// pub enum MyInstruction {
///     /// Transfer tokens between accounts
///     #[handler(processor = process_transfer, data = TransferData, accounts = TransferAccounts)]
///     Transfer = 0,
///
///     /// Close an account (no data required)
///     #[handler(processor = process_close, accounts = CloseAccounts)]
///     Close = 1,
/// }
///
/// // Generated:
/// // - #[repr(u8)]
/// // - #[derive(Debug, Clone, Copy, PartialEq, Eq, AsRefStr, TryFromPrimitive)]
/// // - impl MyInstruction { pub fn dispatch(...) -> ProgramResult { ... } }
/// ```
#[proc_macro_attribute]
pub fn instructions(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(instructions::instructions_impl(input))
}

/// Derive macro for instruction data structs.
///
/// Generates a `TryFrom<&[u8]>` implementation that uses bytemuck for zero-copy
/// deserialization of instruction data. The struct must implement `Pod`.
///
/// # Example
///
/// ```ignore
/// use panchor_derive::InstructionArgs;
/// use bytemuck::{Pod, Zeroable};
///
/// #[repr(C)]
/// #[derive(Clone, Copy, Pod, Zeroable, InstructionArgs)]
/// pub struct TransferData {
///     pub amount: u64,
/// }
///
/// // Usage in instruction handler:
/// pub fn process_transfer(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
///     let args = TransferData::try_from(data)?;
///     // Use args.amount
///     Ok(())
/// }
/// ```
#[proc_macro_derive(InstructionArgs)]
pub fn derive_instruction_args(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(instruction_args::derive_instruction_args_impl(input))
}

/// Derive macro for instruction dispatch implementation.
///
/// Generates the `InstructionDispatch` trait implementation which routes instructions
/// to their handlers based on `#[handler(...)]` attributes on enum variants.
///
/// This derive is automatically added by the `#[instructions]` attribute macro.
/// You typically don't need to use it directly.
///
/// # Handler Attributes
///
/// Each variant can have a `#[handler(...)]` attribute with:
/// - `processor = fn_name` - The processor function (defaults to `process_{snake_case}`)
/// - `data = DataType` - Data struct for parsing (if omitted, processor takes only accounts)
/// - `accounts = AccountsType` - Accounts struct (defaults to `{Variant}Accounts`)
/// - `raw_data` - Pass raw `&[u8]` instead of parsing data
#[proc_macro_derive(InstructionDispatch, attributes(handler))]
pub fn derive_instruction_dispatch(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(instruction_dispatch::derive_instruction_dispatch_impl(
        input,
    ))
}

/// Derive macro for types that should be included in IDL generation.
///
/// This macro implements the `IdlType` trait and validates at compile time that
/// all fields also implement `IdlType`. This ensures type safety in IDL generation.
///
/// # Supported Types
///
/// The following types implement `IdlType` by default:
/// - Primitive integers: `u8`, `u16`, `u32`, `u64`, `u128`, `i8`, `i16`, `i32`, `i64`, `i128`
/// - `bool`, `()`
/// - `Pubkey`
/// - Arrays `[T; N]` where `T: IdlType`
/// - `Option<T>` where `T: IdlType`
///
/// # Example
///
/// ```ignore
/// use panchor::IdlType;
///
/// #[derive(IdlType)]
/// pub struct TransferData {
///     pub amount: u64,
///     pub recipient: Pubkey,
/// }
/// ```
///
/// # Type Aliases
///
/// For wrapper types (newtypes, bitflags) that should appear as primitives in the IDL,
/// use the `idl_type!` macro instead:
///
/// ```ignore
/// idl_type!(Bps, alias = u16);
/// ```
///
/// # Compile-time Validation
///
/// The macro will fail to compile if any field doesn't implement `IdlType`:
///
/// ```ignore,compile_fail
/// struct CustomType; // Doesn't implement IdlType
///
/// #[derive(IdlType)]
/// pub struct InvalidData {
///     pub field: CustomType, // Error!
/// }
/// ```
#[proc_macro_derive(IdlType)]
pub fn derive_idl_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(idl_type::derive_idl_type_impl(input))
}

/// Attribute macro for error enums with automatic IDL generation.
///
/// This macro generates the `IdlBuildErrors` trait implementation which extracts
/// error codes, names, and documentation for IDL generation.
///
/// Each variant's doc comment becomes the error message in the IDL.
/// Variants must have explicit discriminant values (e.g., `Foo = 0`).
///
/// # Example
///
/// ```ignore
/// use panchor_derive::error_code;
///
/// #[error_code]
/// #[repr(u32)]
/// pub enum MyError {
///     /// Round has not ended yet
///     RoundNotEnded = 0,
///     /// Insufficient balance
///     InsufficientFunds = 1,
/// }
///
/// // Generated IDL:
/// // [
/// //   { "code": 0, "name": "RoundNotEnded", "msg": "Round has not ended yet" },
/// //   { "code": 1, "name": "InsufficientFunds", "msg": "Insufficient balance" }
/// // ]
/// ```
#[proc_macro_attribute]
pub fn error_code(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(error_code::errors_impl(input))
}

/// Attribute macro for constants that should be included in IDL generation.
///
/// This macro marks a constant for inclusion in the generated IDL file.
/// The constant's name, type, and value will be extracted during IDL generation.
///
/// # Example
///
/// ```ignore
/// use panchor_derive::constant;
///
/// #[constant]
/// pub const MAX_SQUARES: usize = 16;
///
/// #[constant]
/// pub const DEFAULT_FEE: u64 = 1_000_000;
/// ```
///
/// The IDL generator will include these constants in the `constants` array:
/// ```json
/// {
///   "constants": [
///     { "name": "MAX_SQUARES", "type": "usize", "value": 16 },
///     { "name": "DEFAULT_FEE", "type": "u64", "value": 1000000 }
///   ]
/// }
/// ```
#[proc_macro_attribute]
pub fn constant(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemConst);
    TokenStream::from(constant::constant_impl(input))
}

/// Attribute macro for account type enums.
///
/// This macro adds proper derives and implements the `from_u64` method for
/// account discriminator enums.
///
/// Adds:
/// - `#[repr(u64)]` for proper discriminator layout
/// - Derives: `Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, TryFromPrimitive`
/// - `from_u64(value: u64) -> Option<Self>` method using `TryFromPrimitive`
///
/// # Example
///
/// ```ignore
/// use panchor_derive::account_type;
///
/// #[account_type]
/// pub enum MinesAccount {
///     Automation = 100,
///     Mine = 101,
///     Miner = 103,
/// }
///
/// // Generated:
/// // - #[repr(u64)]
/// // - All standard derives
/// // - MinesAccount::from_u64(100) == Some(MinesAccount::Automation)
/// ```
#[proc_macro_attribute]
pub fn account_type(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(account_type::account_type_impl(input))
}

/// Function-like macro for declaring program metadata.
///
/// This macro combines `declare_id!` with IDL generation for program metadata.
/// It declares the program ID and generates tests for the IDL generator to extract
/// the program ID, accounts type, and events type.
///
/// # Required Parameters
///
/// - `id = "..."` - The base58-encoded program ID
/// - `instructions = EnumType` - The instructions enum type
///
/// # Optional Parameters
///
/// - `accounts = EnumType` - The account type discriminator enum
/// - `events = EnumType` - The event type discriminator enum
/// - `pdas = EnumType` - The PDA definitions enum (created with `#[pdas]`)
///
/// # Example
///
/// ```ignore
/// use panchor::program;
///
/// program! {
///     id = "MinehoBnqZqg7o3tMGyR5GcfzA3CzfT4FDhXDeWr28f",
///     instructions = MinesInstruction,
///     accounts = MinesAccount,
///     events = EventType,
///     pdas = pda::MinesPda,
/// }
/// ```
///
/// This generates:
/// - `pinocchio_pubkey::declare_id!("...")`
/// - IDL test that outputs program metadata markers
/// - `program::Pdas` type alias for the PDA definitions
#[proc_macro]
pub fn program(input: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(input as program::ProgramArgs);
    TokenStream::from(program::program_impl(args))
}

/// Attribute macro for defining PDAs with automatic generation of constants,
/// finder functions, and seed generator functions.
///
/// Apply this to an enum where each variant represents a PDA type.
/// Each variant must have a `#[seeds("...")]` attribute specifying the seed string,
/// and named fields with types for the PDA arguments.
///
/// # Generated Items
///
/// For each variant `Foo`:
/// - `FOO_SEED: &[u8]` - The seed constant
/// - `find_foo_pda(args...) -> (Pubkey, u8)` - PDA finder function
/// - `gen_foo_seeds(args..., bump) -> [Seed; N]` - Function for generating seeds for signing
///
/// # Example
///
/// ```ignore
/// use panchor::pdas;
/// use pinocchio::pubkey::Pubkey;
///
/// #[pdas]
/// enum Pda {
///     #[seeds("pool")]
///     Pool { mint: Pubkey },
///
///     #[seeds("stake")]
///     Stake { pool: Pubkey, authority: Pubkey },
/// }
///
/// // Generated:
/// // pub const POOL_SEED: &[u8] = b"pool";
/// // pub fn find_pool_pda(mint: &Pubkey) -> (Pubkey, u8) { ... }
/// // pub fn gen_pool_seeds<'a>(mint: &'a Pubkey, bump: &'a [u8]) -> [Seed<'a>; 3] { ... }
/// //
/// // pub const STAKE_SEED: &[u8] = b"stake";
/// // pub fn find_stake_pda(pool: &Pubkey, authority: &Pubkey) -> (Pubkey, u8) { ... }
/// // pub fn gen_stake_seeds<'a>(pool: &'a Pubkey, authority: &'a Pubkey, bump: &'a [u8]) -> [Seed<'a>; 4] { ... }
/// ```
///
/// # Usage
///
/// ```ignore
/// // Find a PDA
/// let (pool_pda, bump) = pda::find_pool_pda(&mint);
///
/// // Generate seeds for CPI signing
/// let bump_bytes = [bump];
/// let seeds = pda::gen_pool_seeds(&mint, &bump_bytes);
/// invoke_signed(&ix, &accounts, &[(&seeds).into()])?;
/// ```
#[proc_macro_attribute]
pub fn pdas(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    TokenStream::from(pdas::pdas_impl(input))
}

/// Derive macro for implementing `FindProgramAddress` on PDA structs.
///
/// Apply this to a struct with a `#[seeds("prefix")]` attribute to generate
/// the `FindProgramAddress` trait implementation and a `to_signer_seeds` method.
///
/// # Example
///
/// ```ignore
/// use panchor_derive::FindProgramAddress;
/// use pinocchio::pubkey::Pubkey;
///
/// #[derive(Clone, Copy, Debug, FindProgramAddress)]
/// #[seeds("pool")]
/// pub struct Pool {
///     pub mint: Pubkey,
/// }
///
/// // Generated:
/// // - impl FindProgramAddress { find_program_address(&self, program_id) -> (Pubkey, u8) }
/// // - impl Pool { to_signer_seeds(&self, bump: &[u8; 1]) -> SignerSeeds<3> }
/// ```
///
/// # Note on u64 fields
///
/// PDAs with u64 fields (e.g., `round_id`) won't have `to_signer_seeds` generated
/// because the u64 to bytes conversion creates local variables that cannot
/// be returned. Use the `gen_*_seeds` macros for such PDAs instead.
#[proc_macro_derive(FindProgramAddress, attributes(seeds))]
pub fn derive_find_program_address(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(find_program_address::derive_find_program_address_impl(
        input,
    ))
}
