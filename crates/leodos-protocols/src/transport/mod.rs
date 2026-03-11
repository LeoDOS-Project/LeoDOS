//! The Transport Layer of the CCSDS Protocol Stack.

use core::future::Future;

/// CCSDS File Delivery Protocol (CFDP).
pub mod cfdp;
/// Packet transport trait for L3/L2 wrapping.
pub mod packet;
/// Satellite Reliable SPP Protocol (SRSPP).
pub mod srspp;

/// Reliable message writer. Implemented by transport protocols (SRSPP, CFDP, etc.).
pub trait TransportWrite {
    /// Error type for write operations.
    type Error;

    /// Writes a message reliably to the remote endpoint.
    fn write(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Reliable message reader. Implemented by transport protocols (SRSPP, CFDP, etc.).
pub trait TransportRead {
    /// Error type for read operations.
    type Error;

    /// Reads a message into the provided buffer, returning its length.
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
