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

/// Push failed: the frame cannot accept this packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushError {
    /// The current frame is full but the packet would fit
    /// after a flush.
    Full,
    /// The packet exceeds the maximum data field length and
    /// can never fit in any frame.
    TooLarge,
}

/// Accumulates packets into a transfer frame.
///
/// Owns the frame buffer internally.
/// [`push()`](FrameWriter::push) writes packet data at the
/// current offset. [`finish()`](FrameWriter::finish) stamps the
/// header and returns a borrow of the completed frame.
pub trait FrameWriter {
    /// Error type for frame construction.
    type Error;

    /// Returns `true` if no packets have been pushed yet.
    fn is_empty(&self) -> bool;

    /// Push a packet into the frame at the current offset.
    fn push(&mut self, data: &[u8]) -> Result<(), PushError>;

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
