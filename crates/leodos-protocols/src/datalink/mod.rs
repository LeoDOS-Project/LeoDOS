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

// ── Layer boundary traits ──────────────────────────────────────

/// Send direction of the data link layer.
pub trait DataLinkWriter {
    /// Error type for send operations.
    type Error: core::error::Error;

    /// Send data over the link.
    fn send(&mut self, data: &[u8]) -> impl Future<Output = Result<(), Self::Error>>;
}

/// Receive direction of the data link layer.
pub trait DataLinkReader {
    /// Error type for receive operations.
    type Error: core::error::Error;

    /// Receive data from the link into `buffer`.
    fn recv(&mut self, buffer: &mut [u8]) -> impl Future<Output = Result<usize, Self::Error>>;
}

// ── Group traits ───────────────────────────────────────────────

/// Builds a transfer frame from payload data.
pub trait FrameBuilder {
    /// Error type for build operations.
    type Error;
    /// Wraps `data` in a transfer frame, writing to `output`.
    fn build(&mut self, data: &[u8], output: &mut [u8]) -> Result<usize, Self::Error>;
}

/// Parses a transfer frame and extracts the payload.
pub trait FrameParser {
    /// Error type for parse operations.
    type Error;
    /// Extracts payload from `frame`, returning the data slice.
    fn parse<'a>(&mut self, frame: &'a [u8]) -> Result<&'a [u8], Self::Error>;
}

/// Applies or removes security (encryption/authentication) on frames.
pub trait SecurityProcessor {
    /// Error type for security operations.
    type Error;
    /// Applies security (encrypt/authenticate) to a frame in-place.
    fn apply(&mut self, frame: &mut [u8]) -> Result<usize, Self::Error>;
    /// Removes security (decrypt/verify) from a frame in-place.
    fn process(&mut self, frame: &mut [u8]) -> Result<usize, Self::Error>;
}

/// COP-1 sender (FOP-1) state machine interface.
pub trait ReliabilitySender {
    /// Action to take after processing a frame.
    type Action;
    /// Processes an outgoing frame through the reliability layer.
    fn send(&mut self, frame: &[u8]) -> Self::Action;
}

/// COP-1 receiver (FARM-1) state machine interface.
pub trait ReliabilityReceiver {
    /// Action to take after processing a frame.
    type Action;
    /// Processes an incoming frame through the reliability layer.
    fn receive(&mut self, frame: &[u8]) -> Self::Action;
}
