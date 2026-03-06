//! Implements the CCSDS Data Link Protocols (Layer 2).

use core::future::Future;

/// COP-1 (CCSDS 232.1-B-2) hop-by-hop reliable frame delivery.
pub mod cop1;
/// Async frame sender/receiver traits and TC/TM link channels.
pub mod link;
/// Space Data Link Protocol frame definitions (TC, TM, AOS).
pub mod sdlp;
/// Space Data Link Security (CCSDS 355.0-B-2).
pub mod sdls;
/// Unified Space Data Link Protocol (CCSDS 732.1-B-3).
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
