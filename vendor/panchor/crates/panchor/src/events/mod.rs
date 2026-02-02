//! Event utilities for Pinocchio programs
//!
//! This module provides traits and macros for working with program events
//! that have discriminators.

pub mod log_helpers;
mod serialization;

pub use serialization::EventBytes;

/// Trait for event types
///
/// Implement this trait on event structs to enable them to be emitted
/// via `emit_mine_event`. Events must also implement `Discriminator` and `Pod`.
///
/// # Example
///
/// ```ignore
/// use panchor::{event, Discriminator, Event};
///
/// #[repr(u64)]
/// pub enum EventType {
///     Reset = 0,
///     Close = 1,
/// }
///
/// #[repr(C)]
/// #[derive(Pod, Zeroable)]
/// pub struct ResetEvent {
///     pub round_id: u64,
///     // ... other fields (no discriminator field!)
/// }
///
/// event!(EventType, Reset);
/// ```
pub trait Event {
    /// Returns the name of this event type
    fn name() -> &'static str;
}

/// Trait for logging events to program logs
///
/// This trait is automatically implemented by the `#[derive(EventLog)]` macro.
/// It provides a `log()` method that logs all fields of the event.
pub trait EventLog {
    /// Log the event to program logs
    fn log(&self);
}
