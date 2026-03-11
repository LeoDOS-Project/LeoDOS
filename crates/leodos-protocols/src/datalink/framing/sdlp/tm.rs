//! Telemetry Space Data Link Protocol (TM-SDLP)
//!
//! Spec: https://ccsds.org/Pubs/132x0b3.pdf
//!
//! The Telemetry Transfer Frame is the "envelope" used to package `SpacePacket`s for
//! downlink (sending data from a satellite to the ground).

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::U16;

use crate::coding::randomizer::Randomizer;
use crate::utils::get_bits_u16;
use crate::utils::set_bits_u16;

/// A zero-copy view over a CCSDS Telemetry (TM) Transfer Frame in a raw byte buffer.
///
/// This struct represents the "envelope" used to send one or more `SpacePacket`s
/// from a spacecraft to the ground (the downlink). In addition to routing and sequencing,
/// it includes a crucial "First Header Pointer" to locate the first `SpacePacket`
/// within its data field.
///
/// On the ground, this view is typically created via [`TelemetryTransferFrame::parse()`],
/// which also handles de-randomization of the raw radio data.
///
/// # Layout
///
/// A TM Transfer Frame consists of a 6-byte header followed by a data field.
///
/// ```text
/// +------------------------------------+----------+
/// | Field Name                         | Size     |
/// +------------------------------------+----------+
/// + -- Transfer Frame Header (6 bytes) |          |
/// |                                    |          |
/// | Transfer Frame Version             | 2 bits   |
/// | Spacecraft ID (SCID)               | 10 bits  |
/// | Virtual Channel ID (VCID)          | 3 bits   |
/// | OCF Flag                           | 1 bit    |
/// | Master Channel Frame Count         | 8 bits   |
/// | Virtual Channel Frame Count        | 8 bits   |
/// | Data Field Status                  | 16 bits  |
/// |   ... First Header Pointer         | 11 bits  |
/// |                                    |          |
/// + -- Data Field -------------------- | Variable |
/// |                                    |          |
/// | Contains idle data and/or          |          |
/// | one or more Space Packets          |          |
/// +------------------------------------+----------+
/// | (Optional) OCF                     | 4 bytes  |
/// +------------------------------------+----------+
/// | (Optional) Frame Error Control     | 2 bytes  |
/// +------------------------------------+----------+
/// ```
#[repr(C, packed)]
#[derive(IntoBytes, FromBytes, Unaligned, KnownLayout, Immutable)]
pub struct TelemetryTransferFrame {
    header: TelemetryTransferFrameHeader,
    data_field: [u8],
}

/// The 6-byte header of a Telemetry TransferFrame.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Debug, Copy, Clone)]
pub struct TelemetryTransferFrameHeader {
    /// Contains the 2-bit Version, 10-bit SCID, 3-bit VCID, and 1-bit OCF Flag.
    version_scid_vcid_and_ocf: U16,
    /// Contains the 8-bit Master Channel Frame Count for all frames.
    mc_frame_count: u8,
    /// Contains the 8-bit Virtual Channel Frame Count for this VCID.
    vc_frame_count: u8,
    /// Contains status flags and the 11-bit First Header Pointer to the first Space Packet.
    data_field_status: U16,
}

/// Bitmasks for TM Transfer Frame header fields.
#[rustfmt::skip]
pub mod bitmask {
    /// Bitmask for the 2-bit version field.
    pub const VERSION_MASK: u16 =              0b_1100_0000_0000_0000;
    /// Bitmask for the 10-bit Spacecraft ID field.
    pub const SCID_MASK: u16 =                 0b_0011_1111_1111_0000;
    /// Bitmask for the 3-bit Virtual Channel ID field.
    pub const VCID_MASK: u16 =                 0b_0000_0000_0000_1110;
    /// Bitmask for the 1-bit Operational Control Field flag.
    pub const OCF_FLAG_MASK: u16 =             0b_0000_0000_0000_0001;

    /// Bitmask for the 11-bit First Header Pointer field.
    pub const FIRST_HEADER_POINTER_MASK: u16 = 0b_0000_0111_1111_1111;
}

use bitmask::*;
use crate::ids::{Scid, Vcid};

/// An error that can occur during Telemetry frame construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    /// The provided Spacecraft ID is outside the valid 10-bit range (0-1023).
    InvalidScid(Scid),
    /// The provided Virtual Channel ID is outside the valid 3-bit range (0-7).
    InvalidVcid(Vcid),
    /// The provided buffer is too small to hold the requested frame.
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required_len: usize,
        /// Actual buffer size provided.
        provided_len: usize,
    },
}

/// An error that can occur during Telemetry frame parsing.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    /// The provided buffer is not large enough to hold the de-randomized frame,
    /// or the input and output buffers have different lengths.
    InvalidBufferLength,
    /// The buffer is too small to hold a valid TM frame header.
    TooShortForHeader,
}

#[bon]
impl TelemetryTransferFrame {
    /// The size of the Telemetry Transfer Frame header in bytes.
    pub const HEADER_SIZE: usize = 6;

    /// Parses a raw, possibly randomized, byte slice into a zero-copy Telemetry Transfer Frame view.
    ///
    /// The incoming `bytes` slice is de-randomized into the `output_buffer`. The
    /// The returned `&TelemetryTransferFrame` is a view over this `output_buffer`.
    pub fn parse<'a>(
        bytes: &[u8],
        output_buffer: &'a mut [u8],
        randomizer: &impl Randomizer,
    ) -> Result<&'a TelemetryTransferFrame, ParseError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(ParseError::TooShortForHeader);
        }
        if output_buffer.len() < bytes.len() {
            return Err(ParseError::InvalidBufferLength);
        }
        let frame_buf = &mut output_buffer[..bytes.len()];
        frame_buf.copy_from_slice(bytes);

        randomizer.apply(frame_buf);

        TelemetryTransferFrame::ref_from_bytes(frame_buf)
            .map_err(|_| ParseError::InvalidBufferLength)
    }

    /// Parses a transfer frame without applying derandomization.
    ///
    /// Use this when the coding pipeline has already handled
    /// derandomization.
    pub fn parse_raw(bytes: &[u8]) -> Result<&TelemetryTransferFrame, ParseError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(ParseError::TooShortForHeader);
        }
        TelemetryTransferFrame::ref_from_bytes(bytes)
            .map_err(|_| ParseError::InvalidBufferLength)
    }

    /// Returns a reference to the frame's header.
    pub fn header(&self) -> &TelemetryTransferFrameHeader {
        &self.header
    }

    /// Returns a mutable reference to the frame's data field.
    pub fn data_field_mut(&mut self) -> &mut [u8] {
        &mut self.data_field
    }

    /// Returns a reference to the frame's data field.
    ///
    /// This slice typically contains one or more `SpacePacket`s that can now be
    /// parsed individually.
    pub fn data_field(&self) -> &[u8] {
        &self.data_field
    }

    /// Constructs a new TM Transfer Frame in the provided buffer.
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        version: u8,
        scid: Scid,
        vcid: Vcid,
        mc_frame_count: u8,
        vc_frame_count: u8,
        first_header_pointer: u16,
    ) -> Result<&mut Self, BuildError> {
        if buffer.len() < Self::HEADER_SIZE {
            return Err(BuildError::BufferTooSmall {
                required_len: Self::HEADER_SIZE,
                provided_len: buffer.len(),
            });
        }
        if scid.num_bits() > 10 {
            return Err(BuildError::InvalidScid(scid));
        }
        if vcid.num_bits() > 3 {
            return Err(BuildError::InvalidVcid(vcid));
        }

        let provided_len = buffer.len();
        if provided_len < Self::HEADER_SIZE {
            return Err(BuildError::BufferTooSmall {
                required_len: Self::HEADER_SIZE,
                provided_len,
            });
        }
        let data_field_len = provided_len - Self::HEADER_SIZE;
        let (frame, _) = TelemetryTransferFrame::mut_from_prefix_with_elems(buffer, data_field_len)
            .map_err(|_| BuildError::BufferTooSmall {
            required_len: Self::HEADER_SIZE,
            provided_len,
        })?;

        frame.header.set_version(version);
        frame.header.set_scid(scid);
        frame.header.set_vcid(vcid);
        frame.header.set_mc_frame_count(mc_frame_count);
        frame.header.set_vc_frame_count(vc_frame_count);
        frame.header.set_first_header_pointer(first_header_pointer);

        Ok(frame)
    }
}

impl TelemetryTransferFrameHeader {
    /// Returns the Transfer Frame Version Number.
    pub fn version(&self) -> u8 {
        get_bits_u16(self.version_scid_vcid_and_ocf, VERSION_MASK) as u8
    }
    /// Sets the Transfer Frame Version Number.
    pub fn set_version(&mut self, version: u8) {
        set_bits_u16(
            &mut self.version_scid_vcid_and_ocf,
            VERSION_MASK,
            version as u16,
        );
    }

    /// Returns the Spacecraft ID (SCID).
    pub fn scid(&self) -> Scid {
        Scid::new(get_bits_u16(self.version_scid_vcid_and_ocf, SCID_MASK) as u32)
    }
    /// Sets the Spacecraft ID (SCID).
    pub fn set_scid(&mut self, scid: Scid) {
        set_bits_u16(&mut self.version_scid_vcid_and_ocf, SCID_MASK, scid.get() as u16);
    }

    /// Returns the Virtual Channel ID (VCID).
    pub fn vcid(&self) -> Vcid {
        Vcid::new(get_bits_u16(self.version_scid_vcid_and_ocf, VCID_MASK) as u32)
    }
    /// Sets the Virtual Channel ID (VCID).
    pub fn set_vcid(&mut self, vcid: Vcid) {
        set_bits_u16(&mut self.version_scid_vcid_and_ocf, VCID_MASK, vcid.get() as u16);
    }

    /// Returns the Master Channel Frame Count.
    pub fn mc_frame_count(&self) -> u8 {
        self.mc_frame_count
    }
    /// Sets the Master Channel Frame Count.
    pub fn set_mc_frame_count(&mut self, count: u8) {
        self.mc_frame_count = count;
    }

    /// Returns the Virtual Channel Frame Count.
    pub fn vc_frame_count(&self) -> u8 {
        self.vc_frame_count
    }
    /// Sets the Virtual Channel Frame Count.
    pub fn set_vc_frame_count(&mut self, count: u8) {
        self.vc_frame_count = count;
    }

    /// Returns the First Header Pointer to the first Space Packet in the data field.
    pub fn first_header_pointer(&self) -> u16 {
        get_bits_u16(self.data_field_status, FIRST_HEADER_POINTER_MASK)
    }
    /// Sets the First Header Pointer value.
    pub fn set_first_header_pointer(&mut self, fhp: u16) {
        set_bits_u16(&mut self.data_field_status, FIRST_HEADER_POINTER_MASK, fhp);
    }
}

// ── FrameWrite / FrameRead implementations ──

use super::super::{FrameRead, FrameWrite, PushError};

/// Configuration for building TM transfer frames.
#[derive(Debug, Clone)]
pub struct TmFrameWriterConfig {
    /// Spacecraft ID.
    pub scid: Scid,
    /// Virtual Channel ID.
    pub vcid: Vcid,
    /// Maximum data field length in bytes.
    pub max_data_field_len: usize,
}

/// Accumulates packets into TM transfer frames.
///
/// Owns its frame buffer internally (sized by `BUF`). Packets
/// are pushed directly into the buffer at the correct offset.
/// [`finish()`](FrameWrite::finish) stamps the header and
/// returns a borrow of the completed frame.
pub struct TmFrameWriter<const BUF: usize> {
    config: TmFrameWriterConfig,
    mc_frame_count: u8,
    vc_frame_count: u8,
    data_len: usize,
    buf: [u8; BUF],
}

impl<const BUF: usize> TmFrameWriter<BUF> {
    /// Creates a new TM frame writer.
    pub fn new(config: TmFrameWriterConfig) -> Self {
        Self {
            config,
            mc_frame_count: 0,
            vc_frame_count: 0,
            data_len: 0,
            buf: [0u8; BUF],
        }
    }
}

impl<const BUF: usize> TmFrameWriter<BUF> {
    fn remaining(&self) -> usize {
        self.config
            .max_data_field_len
            .saturating_sub(self.data_len)
    }
}

impl<const BUF: usize> FrameWrite for TmFrameWriter<BUF> {
    type Error = BuildError;

    fn is_empty(&self) -> bool {
        self.data_len == 0
    }

    fn push(&mut self, data: &[u8]) -> Result<(), PushError> {
        if data.len() > self.config.max_data_field_len {
            return Err(PushError::TooLarge);
        }
        if data.len() > self.remaining() {
            return Err(PushError::Full);
        }
        let off =
            TelemetryTransferFrame::HEADER_SIZE + self.data_len;
        self.buf[off..off + data.len()].copy_from_slice(data);
        self.data_len += data.len();
        Ok(())
    }

    fn finish(&mut self) -> Result<&[u8], BuildError> {
        let total =
            TelemetryTransferFrame::HEADER_SIZE + self.data_len;
        let mc = self.mc_frame_count;
        let vc = self.vc_frame_count;
        self.mc_frame_count =
            self.mc_frame_count.wrapping_add(1);
        self.vc_frame_count =
            self.vc_frame_count.wrapping_add(1);

        TelemetryTransferFrame::builder()
            .buffer(&mut self.buf[..total])
            .version(0)
            .scid(self.config.scid)
            .vcid(self.config.vcid)
            .mc_frame_count(mc)
            .vc_frame_count(vc)
            .first_header_pointer(0)
            .build()?;

        self.data_len = 0;
        Ok(&self.buf[..total])
    }
}

/// Extracts packets from a received TM transfer frame.
///
/// Owns its frame buffer internally (sized by `BUF`). The
/// coding layer writes into
/// [`buffer_mut()`](FrameRead::buffer_mut),
/// [`feed()`](FrameRead::feed) validates the header, and
/// [`next()`](FrameRead::next) returns zero-copy sub-slices.
pub struct TmFrameReader<const BUF: usize> {
    buf: [u8; BUF],
    data_start: usize,
    data_end: usize,
}

impl<const BUF: usize> TmFrameReader<BUF> {
    /// Creates a new TM frame reader.
    pub fn new() -> Self {
        Self {
            buf: [0u8; BUF],
            data_start: 0,
            data_end: 0,
        }
    }
}

impl<const BUF: usize> FrameRead for TmFrameReader<BUF> {
    type Error = ParseError;

    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn feed(&mut self, len: usize) -> Result<(), ParseError> {
        let parsed =
            TelemetryTransferFrame::parse_raw(&self.buf[..len])?;
        let data = parsed.data_field();
        self.data_start =
            TelemetryTransferFrame::HEADER_SIZE;
        self.data_end = self.data_start + data.len();
        Ok(())
    }

    fn data_field(&self) -> &[u8] {
        &self.buf[self.data_start..self.data_end]
    }
}
