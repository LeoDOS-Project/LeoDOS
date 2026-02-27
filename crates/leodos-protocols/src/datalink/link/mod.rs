use core::future::Future;

/// Asymmetric link combining separate sender and receiver halves.
pub mod asymmetric;
/// CCSDS File Delivery Service link support.
#[cfg(feature = "cfs")]
pub mod cfs;
/// Telecommand link channels for sending and receiving TC frames.
pub mod tc;
/// Telemetry link channels for sending and receiving TM frames.
pub mod tm;

/// Async trait for sending framed data over a link.
pub trait FrameSender {
    /// Error type for send operations.
    type Error: core::error::Error;

    /// Sends a single frame of data.
    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Async trait for receiving framed data from a link.
pub trait FrameReceiver {
    /// Error type for receive operations.
    type Error: core::error::Error;

    /// Receives a frame into the buffer, returning the number of bytes read.
    fn recv(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}
