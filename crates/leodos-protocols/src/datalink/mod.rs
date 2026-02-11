//! Implements the CCSDS Data Link Protocols (Layer 2).

use core::future::Future;

pub mod cop1;
pub mod link;
pub mod sdlp;
pub mod sdls;
pub mod uslp;

/// Trait for the underlying link.
///
/// Implement this for your physical/network layer (UDP, serial, etc).
pub trait DataLink {
    /// Error type for link operations.
    type Error: core::error::Error;

    /// Send data over the link.
    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;

    /// Receive data packet from the link.
    /// Returns the number of bytes received.
    fn recv(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
