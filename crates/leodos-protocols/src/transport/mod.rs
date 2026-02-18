//! The Transport Layer of the CCSDS Protocol Stack.

use core::future::Future;

pub mod cfdp;
pub mod packet;
pub mod srspp;

/// Reliable message sender. Implemented by transport protocols (SRSPP, CFDP, etc.).
pub trait TransportSender {
    type Error;

    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Reliable message receiver. Implemented by transport protocols (SRSPP, CFDP, etc.).
pub trait TransportReceiver {
    type Error;

    fn recv(&mut self, buf: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
