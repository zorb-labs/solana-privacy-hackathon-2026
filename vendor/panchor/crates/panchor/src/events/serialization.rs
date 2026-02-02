//! Event serialization utilities

use super::Event;
use crate::Discriminator;
use bytemuck::Pod;

/// Extension trait for serializing events to bytes with discriminator
pub trait EventBytes: Discriminator + Event + Pod + Sized {
    /// Serialize the event to bytes with the discriminator prepended
    ///
    /// Returns a Vec containing [discriminator (8 bytes), `event_data`...]
    fn to_event_bytes(&self) -> alloc::vec::Vec<u8> {
        let mut bytes = alloc::vec::Vec::with_capacity(8 + core::mem::size_of::<Self>());
        bytes.extend_from_slice(&Self::DISCRIMINATOR.to_le_bytes());
        bytes.extend_from_slice(bytemuck::bytes_of(self));
        bytes
    }

    /// Get the discriminator as bytes
    fn discriminator_bytes() -> [u8; 8] {
        Self::DISCRIMINATOR.to_le_bytes()
    }
}

// Blanket implementation for all types implementing Discriminator + Event + Pod
impl<T: Discriminator + Event + Pod> EventBytes for T {}
