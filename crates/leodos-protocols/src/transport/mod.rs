//! The Transport Layer of the CCSDS Protocol Stack.

use core::future::Future;

/// CCSDS File Delivery Protocol (CFDP).
pub mod cfdp;
/// Packet transport trait for L3/L2 wrapping.
pub mod packet;
/// Satellite Reliable SPP Protocol (SRSPP).
pub mod srspp;

/// Reliable message sender. Implemented by transport protocols (SRSPP, CFDP, etc.).
pub trait TransportSender {
    /// Error type returned by send operations.
    type Error;

    /// Send a message reliably to the remote endpoint.
    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Reliable message receiver. Implemented by transport protocols (SRSPP, CFDP, etc.).
pub trait TransportReceiver {
    /// Error type returned by receive operations.
    type Error;

    /// Receive a message into the provided buffer, returning its length.
    fn recv(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
