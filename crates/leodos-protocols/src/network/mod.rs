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
pub trait NetworkWriter {
    /// Error type for send operations.
    type Error: core::error::Error;

    /// Sends a packet.
    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Receive direction of the network layer.
pub trait NetworkReader {
    /// Error type for receive operations.
    type Error: core::error::Error;

    /// Receives a packet into the provided buffer, returning its length.
    fn recv(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
