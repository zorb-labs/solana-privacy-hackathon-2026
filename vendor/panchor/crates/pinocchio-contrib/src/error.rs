//! Error handling utilities with logging support.
//!
//! This module provides macros for returning errors with automatic logging,
//! including file and line number information for easier debugging.
//!
//! # Usage
//!
//! ```ignore
//! use pinocchio_contrib::bail_err;
//! use pinocchio::program_error::ProgramError;
//!
//! fn check_something() -> Result<(), ProgramError> {
//!     if condition_failed {
//!         bail_err!(ProgramError::InvalidAccountData, "Expected {} but got {}", expected, actual);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! Output:
//! ```text
//! InvalidAccountData
//! Expected foo but got bar
//! @ src/lib.rs:5
//! ```

use pinocchio::program_error::ProgramError;

/// Log the caller's source location.
///
/// Uses `#[track_caller]` to capture the call site location at runtime,
/// allowing for accurate error tracing in logs.
#[track_caller]
pub fn log_caller_location() {
    let caller = core::panic::Location::caller();
    pinocchio_log::log!("@ {}:{}", caller.file(), caller.line());
}

/// Bail with an error, logging the error name, optional message, and source location.
///
/// The error type is auto-detected from the `Type::Variant` pattern. For custom error types,
/// a `name()` method returning `&'static str` is required. For `ProgramError`, the variant
/// name is extracted via stringify. The source file and line are always logged.
///
/// # Usage
///
/// ```ignore
/// // For ProgramError - the variant name is extracted automatically
/// bail_err!(ProgramError::InvalidAccountData);
/// bail_err!(ProgramError::InsufficientFunds, "Not enough SOL");
/// bail_err!(ProgramError::InvalidArgument, "Expected {} got {}", expected, actual);
///
/// // For custom errors with a name() method
/// bail_err!(MinesError::RoundNotEnded);
/// bail_err!(MinesError::GraduationDeadlinePassed, "Custom message here");
/// bail_err!(MinesError::InvalidAmount, "amount={} max={}", amount, max);
/// ```
#[macro_export]
macro_rules! bail_err {
    // ProgramError variant (extracts name via stringify)
    (ProgramError::$variant:ident) => {{
        ::pinocchio_log::log!("{}", stringify!($variant));
        $crate::log_caller_location();
        return Err(::pinocchio::program_error::ProgramError::$variant);
    }};
    // ProgramError variant with format args
    (ProgramError::$variant:ident, $($arg:tt)+) => {{
        ::pinocchio_log::log!("{}", stringify!($variant));
        ::pinocchio_log::log!($($arg)+);
        $crate::log_caller_location();
        return Err(::pinocchio::program_error::ProgramError::$variant);
    }};
    // Custom error Type::Variant - uses name() method for logging
    ($err_ty:ident :: $variant:ident) => {{
        ::pinocchio_log::log!("Error: {}", $err_ty::$variant.name());
        $crate::log_caller_location();
        return Err($err_ty::$variant.into());
    }};
    // Custom error Type::Variant with format args - uses name() method for logging
    ($err_ty:ident :: $variant:ident, $($arg:tt)+) => {{
        ::pinocchio_log::log!("Error: {}", $err_ty::$variant.name());
        ::pinocchio_log::log!($($arg)+);
        $crate::log_caller_location();
        return Err($err_ty::$variant.into());
    }};
}

/// Require a condition to be true, otherwise bail with an error.
///
/// This macro is similar to `bail_err!` but takes a boolean condition as the first argument.
/// If the condition is false, it will bail with the specified error.
///
/// # Usage
///
/// ```ignore
/// // For ProgramError - bails if condition is false
/// require!(account.is_writable, ProgramError::InvalidAccountData);
/// require!(amount > 0, ProgramError::InvalidArgument, "Amount must be positive");
///
/// // For custom errors with a name() method
/// require!(is_valid, MinesError::InvalidAmount);
/// require!(has_permission, MinesError::NotAuthorized, "User not authorized");
/// ```
#[macro_export]
macro_rules! require {
    // ProgramError variant (extracts name via stringify)
    ($cond:expr, ProgramError::$variant:ident) => {{
        if !$cond {
            ::pinocchio_log::log!("{}", stringify!($variant));
            $crate::log_caller_location();
            return Err(::pinocchio::program_error::ProgramError::$variant);
        }
    }};
    // ProgramError variant with format args
    ($cond:expr, ProgramError::$variant:ident, $($arg:tt)+) => {{
        if !$cond {
            ::pinocchio_log::log!("{}", stringify!($variant));
            ::pinocchio_log::log!($($arg)+);
            $crate::log_caller_location();
            return Err(::pinocchio::program_error::ProgramError::$variant);
        }
    }};
    // Custom error Type::Variant - uses name() method for logging
    ($cond:expr, $err_ty:ident :: $variant:ident) => {{
        if !$cond {
            ::pinocchio_log::log!("Error: {}", $err_ty::$variant.name());
            $crate::log_caller_location();
            return Err($err_ty::$variant.into());
        }
    }};
    // Custom error Type::Variant with format args - uses name() method for logging
    ($cond:expr, $err_ty:ident :: $variant:ident, $($arg:tt)+) => {{
        if !$cond {
            ::pinocchio_log::log!("Error: {}", $err_ty::$variant.name());
            ::pinocchio_log::log!($($arg)+);
            $crate::log_caller_location();
            return Err($err_ty::$variant.into());
        }
    }};
}

/// Format a trace string with file and line number.
///
/// This is a helper macro used for testing that returns the trace string
/// that would be logged by `bail!` and `bail_msg!`.
///
/// # Example
///
/// ```ignore
/// let trace = trace_string!();
/// assert!(trace.starts_with("@ "));
/// assert!(trace.contains("error.rs:"));
/// ```
#[macro_export]
macro_rules! trace_string {
    () => {
        format!("@ {}:{}", file!(), line!())
    };
}

/// Log a validation error for a specific account field.
///
/// Logs two separate messages to avoid duplicating the prefix string
/// in the binary for each field name.
///
/// Output format:
/// ```text
/// account validation error:
/// field_name
/// ```
#[inline(always)]
pub fn log_account_validation_error(field_name: &str) {
    // Split into two logs to avoid format string overhead per field
    // The prefix is a static string, field name is logged separately
    pinocchio::log::sol_log("account validation error:");
    pinocchio::log::sol_log(field_name);
}

/// Logs the call trace and returns the error.
///
/// This function uses `#[track_caller]` to capture the call site location
/// at runtime, allowing for accurate error tracing in logs.
///
/// # Example
///
/// ```ignore
/// use pinocchio_contrib::trace;
/// use pinocchio::program_error::ProgramError;
///
/// fn check_account() -> Result<(), ProgramError> {
///     if account_already_exists {
///         return Err(trace(
///             "Account already initialized",
///             ProgramError::AccountAlreadyInitialized,
///         ));
///     }
///     Ok(())
/// }
/// ```
///
/// Output:
/// ```text
/// Account already initialized @ src/lib.rs:10:20
/// ```
#[track_caller]
pub fn trace(msg: &str, error: ProgramError) -> ProgramError {
    let caller = core::panic::Location::caller();
    pinocchio_log::log!("{} @ {}:{}", msg, caller.file(), caller.line());
    error
}

#[cfg(test)]
mod tests {
    use pinocchio::program_error::ProgramError;

    // Helper to capture the trace format without actually logging
    fn get_trace_at_callsite() -> (String, u32) {
        (file!().to_string(), line!())
    }

    #[test]
    fn test_trace_string_contains_file() {
        let trace = trace_string!();
        assert!(trace.starts_with("@ "), "Trace should start with '@ '");
        assert!(
            trace.contains("error.rs"),
            "Trace should contain the file name"
        );
    }

    #[test]
    fn test_trace_string_contains_line_number() {
        let trace = trace_string!();
        // Line numbers are numeric and follow the colon
        let parts: Vec<&str> = trace.split(':').collect();
        assert_eq!(parts.len(), 2, "Trace should have format 'file:line'");
        let line_num: u32 = parts[1].parse().expect("Line should be a number");
        assert!(line_num > 0, "Line number should be positive");
    }

    #[test]
    fn test_trace_string_line_number_is_accurate() {
        let expected_line = line!() + 1;
        let trace = trace_string!();
        let actual_line: u32 = trace.split(':').nth(1).unwrap().parse().unwrap();
        assert_eq!(
            actual_line, expected_line,
            "Line number should match the actual source line"
        );
    }

    #[test]
    fn test_trace_format_matches_expected_pattern() {
        let trace = trace_string!();
        // Should match pattern: "@ path/to/file.rs:123"
        let re_pattern = r"^@ .+\.rs:\d+$";
        let re = regex_lite::Regex::new(re_pattern).unwrap();
        assert!(
            re.is_match(&trace),
            "Trace '{trace}' should match pattern '{re_pattern}'"
        );
    }

    #[test]
    fn test_file_macro_returns_this_file() {
        let (file, _) = get_trace_at_callsite();
        assert!(
            file.ends_with("error.rs"),
            "file!() should return error.rs, got: {file}"
        );
    }

    #[test]
    fn test_nested_function_trace() {
        fn outer() -> String {
            inner()
        }

        fn inner() -> String {
            trace_string!()
        }

        let trace = outer();
        // The trace should point to the inner function where trace_string! is called
        assert!(trace.contains("error.rs"));
    }

    // Tests for bail_err! macro
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    #[repr(u32)]
    enum TestError {
        InvalidAmount = 0,
        NotAuthorized = 1,
    }

    impl TestError {
        #[allow(clippy::trivially_copy_pass_by_ref)]
        fn name(&self) -> &'static str {
            match self {
                Self::InvalidAmount => "InvalidAmount",
                Self::NotAuthorized => "NotAuthorized",
            }
        }
    }

    impl From<TestError> for ProgramError {
        fn from(e: TestError) -> Self {
            Self::Custom(e as u32)
        }
    }

    #[test]
    fn test_bail_err_returns_correct_error() {
        fn inner() -> Result<(), ProgramError> {
            bail_err!(TestError::InvalidAmount);
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::Custom(0));
    }

    #[test]
    fn test_bail_err_with_different_variants() {
        fn test_invalid() -> Result<(), ProgramError> {
            bail_err!(TestError::InvalidAmount);
        }

        fn test_not_auth() -> Result<(), ProgramError> {
            bail_err!(TestError::NotAuthorized);
        }

        assert_eq!(test_invalid().unwrap_err(), ProgramError::Custom(0));
        assert_eq!(test_not_auth().unwrap_err(), ProgramError::Custom(1));
    }

    #[test]
    fn test_bail_err_early_return() {
        fn inner() -> Result<i32, ProgramError> {
            bail_err!(TestError::InvalidAmount);
            #[allow(unreachable_code)]
            Ok(42) // This should never be reached
        }

        assert!(inner().is_err());
    }

    #[test]
    fn test_bail_err_with_program_error() {
        fn inner() -> Result<(), ProgramError> {
            bail_err!(ProgramError::InvalidAccountData);
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn test_bail_err_with_message() {
        fn inner() -> Result<(), ProgramError> {
            bail_err!(ProgramError::InvalidAccountData, "test message");
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn test_bail_err_with_format_args() {
        fn inner() -> Result<(), ProgramError> {
            let expected = 5;
            let actual = 3;
            bail_err!(
                ProgramError::InvalidArgument,
                "expected {} got {}",
                expected,
                actual
            );
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::InvalidArgument);
    }

    // Tests for require! macro
    #[test]
    fn test_require_passes_when_true() {
        fn inner() -> Result<(), ProgramError> {
            require!(true, ProgramError::InvalidAccountData);
            Ok(())
        }

        assert!(inner().is_ok());
    }

    #[test]
    fn test_require_fails_when_false() {
        fn inner() -> Result<(), ProgramError> {
            require!(false, ProgramError::InvalidAccountData);
            #[allow(unreachable_code)]
            Ok(())
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn test_require_with_expression() {
        fn inner(value: u64) -> Result<(), ProgramError> {
            require!(value > 0, ProgramError::InvalidArgument);
            Ok(())
        }

        assert!(inner(5).is_ok());
        assert!(inner(0).is_err());
    }

    #[test]
    fn test_require_with_message() {
        fn inner() -> Result<(), ProgramError> {
            require!(false, ProgramError::InvalidAccountData, "test message");
            #[allow(unreachable_code)]
            Ok(())
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::InvalidAccountData);
    }

    #[test]
    fn test_require_with_custom_error() {
        fn inner() -> Result<(), ProgramError> {
            require!(false, TestError::InvalidAmount);
            #[allow(unreachable_code)]
            Ok(())
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::Custom(0));
    }

    #[test]
    fn test_require_with_custom_error_and_message() {
        fn inner() -> Result<(), ProgramError> {
            require!(false, TestError::NotAuthorized, "user={}", "alice");
            #[allow(unreachable_code)]
            Ok(())
        }

        let result = inner();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ProgramError::Custom(1));
    }
}
