//! Error types for validation test program

use panchor::prelude::*;

/// Errors for the validation test program
#[error_code]
pub enum ValidationError {
    /// Invalid value provided
    InvalidValue = 0,
}
