use core::future::Future;

/// CCSDS core Flight Executive (cFE) command and telemetry headers.
pub mod cfe;
/// Inter-Satellite Link (ISL) addressing, routing, and gossip protocols.
pub mod isl;
/// A point-to-point network layer that forwards directly to the datalink.
pub mod ptp;
/// CCSDS Space Packet Protocol (SPP) — re-exported from
/// [`datalink::spp`](crate::datalink::spp).
pub use crate::datalink::spp;

/// Send direction of the network layer.
pub trait NetworkWrite {
    /// Error type for write operations.
    type Error: core::error::Error;

    /// Writes a packet to the network.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Receive direction of the network layer.
pub trait NetworkRead {
    /// Error type for read operations.
    type Error: core::error::Error;

    /// Reads a packet into the provided buffer, returning its length.
    fn read(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
