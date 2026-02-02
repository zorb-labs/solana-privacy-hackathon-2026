//! Helper functions for event logging
//!
//! These functions are used with `#[event_log(with = func)]` attribute
//! to customize how fields are logged.

/// Convert a byte slice to a UTF-8 string, trimming trailing null bytes.
///
/// # Example
/// ```ignore
/// #[event_log(with = panchor::events::log_helpers::slug_to_str)]
/// pub slug: [u8; 32],
/// ```
pub fn slug_to_str(bytes: &[u8]) -> &str {
    // Find the position of the first null byte or use full length
    let len = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    core::str::from_utf8(&bytes[..len]).unwrap_or("<invalid utf8>")
}

/// Format a boolean stored as u8 (0 = false, non-zero = true).
///
/// # Example
/// ```ignore
/// #[event_log(with = panchor::events::log_helpers::bool_u8)]
/// pub is_winner: u8,
/// ```
pub fn bool_u8(value: &u8) -> bool {
    *value != 0
}

/// Wrapper for winning square that handles refund case (`u8::MAX`).
pub struct WinningSquare(pub u8);

impl core::fmt::Display for WinningSquare {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == u8::MAX {
            write!(f, "refund")
        } else {
            write!(f, "{}", self.0)
        }
    }
}

/// Format a winning square, showing "refund" for `u8::MAX`.
///
/// # Example
/// ```ignore
/// #[event_log(with = panchor::events::log_helpers::winning_square)]
/// pub winning_square: u8,
/// ```
pub fn winning_square(value: &u8) -> WinningSquare {
    WinningSquare(*value)
}
