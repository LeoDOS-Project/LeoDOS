//! A zero-copy view and parser for CCSDS Telemetry (TM) Transfer Frames.
//!
//! The Telemetry Transfer Frame is the "envelope" used to package `SpacePacket`s for
//! downlink (sending data from a satellite to the ground).
use super::randomizer::Randomizer;
use bon::bon;
use zerocopy::byteorder::network_endian::U16;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

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

/// An error that can occur during Telemetry frame construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    /// The provided Spacecraft ID is outside the valid 10-bit range (0-1023).
    InvalidScid(u16),
    /// The provided Virtual Channel ID is outside the valid 3-bit range (0-7).
    InvalidVcid(u8),
    /// The provided buffer is too small to hold the requested frame.
    BufferTooSmall { required: usize, provided: usize },
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

#[bon]
impl TelemetryTransferFrame {
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

    #[builder]
    pub fn new(
        buffer: &mut [u8],
        scid: u16,
        vcid: u8,
        mc_frame_count: u8,
        vc_frame_count: u8,
    ) -> Result<&mut Self, BuildError> {
        if buffer.len() < Self::HEADER_SIZE {
            return Err(BuildError::BufferTooSmall {
                required: Self::HEADER_SIZE,
                provided: buffer.len(),
            });
        }
        if scid > 0x3FF {
            return Err(BuildError::InvalidScid(scid));
        }
        if vcid > 0x07 {
            return Err(BuildError::InvalidVcid(vcid));
        }

        let frame = TelemetryTransferFrame::mut_from_bytes(buffer).unwrap();

        let tfvn = 0u16; // Version 1 is '00'
        let word0 = (tfvn << 14) | (scid & 0x3FF) << 4 | ((vcid & 0x07) as u16) << 1;
        frame.header.version_scid_vcid_and_ocf.set(word0);
        frame.header.mc_frame_count = mc_frame_count;
        frame.header.vc_frame_count = vc_frame_count;

        Ok(frame)
    }
}

impl TelemetryTransferFrameHeader {
    /// Returns the Spacecraft ID (SCID).
    pub fn scid(&self) -> u16 {
        (self.version_scid_vcid_and_ocf.get() >> 4) & 0x3FF
    }

    /// Returns the Virtual Channel ID (VCID).
    pub fn vcid(&self) -> u8 {
        ((self.version_scid_vcid_and_ocf.get() >> 1) & 0x07) as u8
    }

    /// Returns the Master Channel Frame Count.
    pub fn mc_frame_count(&self) -> u8 {
        self.mc_frame_count
    }

    /// Returns the Virtual Channel Frame Count.
    pub fn vc_frame_count(&self) -> u8 {
        self.vc_frame_count
    }
}
