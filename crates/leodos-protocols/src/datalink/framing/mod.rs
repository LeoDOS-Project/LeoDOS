//! Transfer frame definitions for the data link layer.
//!
//! Contains the protocol data units (TC, TM, AOS, Proximity-1,
//! USLP) that carry user data and control information across the
//! space link.
//!
//! The [`FrameWriter`] and [`FrameReader`] traits abstract over
//! frame formats, allowing the link layer to work with any frame
//! type (SDLP TC/TM, USLP, Proximity-1). Both own their frame
//! buffers internally, preventing buffer-mismatch bugs.

/// Accumulates packets into a transfer frame.
///
/// Owns the frame buffer internally.
/// [`push()`](FrameWriter::push) writes packet data at the
/// current offset. [`finish()`](FrameWriter::finish) stamps the
/// header and returns a borrow of the completed frame.
pub trait FrameWriter {
    /// Error type for frame construction.
    type Error;

    /// Space remaining in the current frame for packet data.
    fn remaining(&self) -> usize;

    /// Push a packet into the frame at the current offset.
    /// Returns `true` if it fit, `false` otherwise.
    fn push(&mut self, data: &[u8]) -> bool;

    /// Stamp the frame header and return the finished frame.
    /// Resets internal state for the next frame.
    fn finish(&mut self) -> Result<&[u8], Self::Error>;
}

/// Extracts packets from a received transfer frame.
///
/// Owns the frame buffer internally.
/// [`buffer_mut()`](FrameReader::buffer_mut) provides write
/// access for the coding layer to fill.
/// [`feed()`](FrameReader::feed) validates the header.
/// [`next()`](FrameReader::next) yields zero-copy packet
/// sub-slices.
pub trait FrameReader {
    /// Error type for frame parsing.
    type Error;

    /// Returns a mutable reference to the internal buffer
    /// for the coding layer to write received data into.
    fn buffer_mut(&mut self) -> &mut [u8];

    /// Validate the frame header for `len` bytes of data
    /// in the buffer and prepare for packet extraction.
    fn feed(&mut self, len: usize) -> Result<(), Self::Error>;

    /// Return the next packet as a sub-slice of the buffer.
    ///
    /// Returns `None` when all packets have been extracted.
    fn next(&mut self) -> Option<&[u8]>;
}

/// Space Data Link Protocol frame definitions (TC, TM, AOS).
pub mod sdlp;
/// Unified Space Data Link Protocol (CCSDS 732.1-B-3).
pub mod uslp;
