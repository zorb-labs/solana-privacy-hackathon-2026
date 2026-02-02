//! End-to-end tests for panchor account validation
//!
//! These tests verify that each constraint type in the `#[derive(Accounts)]` macro
//! correctly validates accounts and returns appropriate errors.

#[cfg(test)]
mod helpers;
#[cfg(test)]
mod tests;
