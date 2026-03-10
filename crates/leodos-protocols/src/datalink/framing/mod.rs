//! Transfer frame definitions for the data link layer.
//!
//! Contains the protocol data units (TC, TM, AOS, Proximity-1,
//! USLP) that carry user data and control information across the
//! space link.

/// Builds a transfer frame from payload data.
pub trait FrameBuilder {
    /// Error type for build operations.
    type Error;
    /// Wraps `data` in a transfer frame, writing to `output`.
    fn build(
        &mut self,
        data: &[u8],
        output: &mut [u8],
    ) -> Result<usize, Self::Error>;
}

/// Parses a transfer frame and extracts the payload.
pub trait FrameParser {
    /// Error type for parse operations.
    type Error;
    /// Extracts payload from `frame`, returning the data slice.
    fn parse<'a>(
        &mut self,
        frame: &'a [u8],
    ) -> Result<&'a [u8], Self::Error>;
}

/// Space Data Link Protocol frame definitions (TC, TM, AOS).
pub mod sdlp;
/// Unified Space Data Link Protocol (CCSDS 732.1-B-3).
pub mod uslp;
