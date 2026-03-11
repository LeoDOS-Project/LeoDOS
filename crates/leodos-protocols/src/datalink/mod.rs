//! Implements the CCSDS Data Link Protocols (Layer 2).

use core::future::Future;

/// Transfer frame definitions (SDLP, USLP).
pub mod framing;
/// CCSDS Space Packet Protocol (SPP) definitions.
pub mod spp;
/// Async link channels for sending and receiving frames.
pub mod link;
/// Hop-by-hop reliable frame delivery (COP-1).
pub mod reliability;
/// Frame-level encryption and authentication (SDLS).
pub mod security;

// ── Layer boundary traits ──────────────────────────────────────

/// Send direction of the data link layer.
pub trait DatalinkWrite {
    /// Error type for write operations.
    type Error: core::error::Error;

    /// Write data over the link.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Receive direction of the data link layer.
pub trait DatalinkRead {
    /// Error type for read operations.
    type Error: core::error::Error;

    /// Read data from the link into `buffer`.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
