// In src/tctm/tm.rs

//! A zero-copy view and parser for CCSDS Telemetry (TM) Transfer Frames.
//!
//! The TM Transfer Frame is the "envelope" used to package `SpacePacket`s for
//! downlink (sending data from a satellite to the ground).
use super::randomizer::Randomizer;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::U16;

/// A zero-copy view over a TM Transfer Frame in a raw byte buffer.
///
/// Unlike TC frames, TM frames are typically only parsed, not built, by ground
/// software. They have a fixed length per physical channel, which must be known
/// in advance.
#[repr(C, packed)]
#[derive(FromBytes, Unaligned, KnownLayout, Immutable)]
pub struct TMTransferFrame {
    header: TMHeader,
    data_field: [u8],
}

/// An error that can occur during TM frame parsing.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    /// The provided buffer is not large enough to hold the de-randomized frame,
    /// or the input and output buffers have different lengths.
    InvalidBufferLength,
}

/// The 6-byte header of a TM Transfer Frame.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Debug, Copy, Clone)]
pub struct TMHeader {
    word0: U16,
    mc_frame_count: u8,
    vc_frame_count: u8,
    data_field_status: U16,
}

impl TMTransferFrame {
    /// Parses a raw, possibly randomized, byte slice into a zero-copy TM Transfer Frame view.
    ///
    /// The incoming `bytes` slice is de-randomized into the `output_buffer`. The
    /// The returned `&TMTransferFrame` is a view over this `output_buffer`.
    ///
    /// # Arguments
    /// * `bytes`: The raw bytes of the TM frame received from the radio.
    /// * `output_buffer`: A buffer at least as large as `bytes` to hold the
    ///   de-randomized frame.
    /// * `randomizer`: The specific randomization algorithm to apply.
    pub fn parse<'a>(
        bytes: &[u8],
        output_buffer: &'a mut [u8],
        randomizer: &impl Randomizer,
    ) -> Result<&'a TMTransferFrame, ParseError> {
        if output_buffer.len() < bytes.len() {
            return Err(ParseError::InvalidBufferLength);
        }
        let frame_buf = &mut output_buffer[..bytes.len()];
        frame_buf.copy_from_slice(bytes);

        randomizer.apply(frame_buf);

        TMTransferFrame::ref_from_bytes(frame_buf).map_err(|_| ParseError::InvalidBufferLength)
    }

    /// Returns a reference to the frame's header.
    pub fn header(&self) -> &TMHeader {
        &self.header
    }

    /// Returns a reference to the frame's data field.
    ///
    /// This slice typically contains one or more `SpacePacket`s that can now be
    /// parsed individually.
    pub fn data_field(&self) -> &[u8] {
        &self.data_field
    }
}
