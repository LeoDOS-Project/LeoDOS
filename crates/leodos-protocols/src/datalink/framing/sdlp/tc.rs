//! Telecommand Space Data Link Protocol (TC-SDLP)
//!
//! Spec: https://ccsds.org/Pubs/232x0b4e1c1.pdf
//!
//! The Telecommand Transfer Frame is the "envelope" used to package `SpacePacket`s for
//! uplink (sending commands from the ground to a satellite).

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::U16;

use super::super::{FrameRead, FrameWrite, PushError};
use crate::utils::get_bits_u16;
use crate::utils::set_bits_u16;

/// A zero-copy view over a CCSDS Telecommand (TC) Transfer Frame in a raw byte buffer.
///
/// This struct represents the "envelope" used to send one or more `SpacePacket`s
/// from the ground to a spacecraft (the uplink). It provides the necessary routing
/// (SCID, VCID) and sequencing for the radio link.
///
/// It is typically constructed via the ergonomic [`TelecommandTransferFrame::builder()`].
///
/// # Layout
///
/// A TC Transfer Frame consists of a 5-byte header followed by a data field.
///
/// ```text
/// +------------------------------------+---------------------+
/// | Field Name                         | Size                |
/// +------------------------------------+---------------------+
/// + -- Transfer Frame Header (5 bytes) |                     |
/// |                                    |                     |
/// | Transfer Frame Version             | 2 bits              |
/// | Bypass Flag                        | 1 bit               |
/// | Control Command Flag               | 1 bit               |
/// | Reserved                           | 2 bits              |
/// | Spacecraft ID (SCID)               | 10 bits             |
/// | Virtual Channel ID (VCID)          | 6 bits              |
/// | Frame Length                       | 10 bits             |
/// | Frame Sequence Number              | 8 bits              |
/// |                                    |                     |
/// + -- Data Field -------------------- | 1 - 1019 bytes      |
/// |                                    |                     |
/// | Contains one or more Space Packets |                     |
/// +------------------------------------+---------------------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct TelecommandTransferFrame {
    header: TelecommandTransferFrameHeader,
    data_field: [u8],
}

/// The 5-byte header of a Telecommand Transfer Frame.
#[repr(C)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Debug, Copy, Clone)]
pub struct TelecommandTransferFrameHeader {
    /// Contains the 2-bit Version, 1-bit Bypass Flag, 1-bit Control Flag, and 10-bit SCID.
    id_and_scid: U16,
    /// Contains the 6-bit VCID and 10-bit Frame Length (total length - 1).
    vcid_and_length: U16,
    /// Contains the 8-bit Frame Sequence Number for this Virtual Channel.
    sequence_num: u8,
}

/// Bitmasks for TC Transfer Frame header fields.
#[rustfmt::skip]
pub mod bitmask {
    /// Bitmask for the 2-bit version field.
    pub const VERSION_MASK: u16 =      0b_1100_0000_0000_0000;
    /// Bitmask for the 1-bit bypass flag.
    pub const BYPASS_FLAG_MASK: u16 =  0b_0010_0000_0000_0000;
    /// Bitmask for the 1-bit control command flag.
    pub const CONTROL_FLAG_MASK: u16 = 0b_0001_0000_0000_0000;
    /// Bitmask for the 2-bit reserved field.
    pub const _RESERVED_MASK: u16 =    0b_0000_1100_0000_0000;
    /// Bitmask for the 10-bit Spacecraft ID field.
    pub const SCID_MASK: u16 =         0b_0000_0011_1111_1111;

    /// Bitmask for the 6-bit Virtual Channel ID field.
    pub const VCID_MASK: u16 =         0b_1111_1100_0000_0000;
    /// Bitmask for the 10-bit frame length field.
    pub const FRAME_LEN_MASK: u16 =    0b_0000_0011_1111_1111;
}

use bitmask::*;
use crate::ids::{Scid, Vcid};

/// An error that can occur during Telecommand Transfer Frame construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    /// The provided Spacecraft ID is outside the valid 10-bit range (0-1023).
    InvalidScid(Scid),
    /// The provided Virtual Channel ID is outside the valid 6-bit range (0-63).
    InvalidVcid(Vcid),
    /// The provided data length exceeds the maximum of 1019 bytes.
    DataTooLong(usize),
    /// The provided buffer is too small to hold the requested frame.
    BufferTooSmall {
        /// Minimum number of bytes needed for the frame.
        required: usize,
        /// Actual buffer size provided.
        provided: usize,
    },
}

/// An error that can occur during Telecommand Transfer Frame parsing.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    /// The provided slice is shorter than the 5-byte header.
    TooShortForHeader {
        /// Actual number of bytes provided.
        actual: usize,
    },
    /// The header's length field implies a frame larger than the provided buffer.
    IncompleteFrame {
        /// Total frame length declared in the header.
        header_len: usize,
        /// Actual buffer length provided.
        buffer_len: usize,
    },
}

/// The Bypass Flag, controlling the type of frame acceptance checks performed
/// by the receiving spacecraft.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum BypassFlag {
    /// The normal acceptance checks shall be performed (Type-A).
    TypeA = 0,
    /// The acceptance checks are bypassed (Type-B).
    TypeB = 1,
}

/// The Control Command Flag, indicating whether the frame contains user data or
/// control information for the receiver.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum ControlFlag {
    /// The frame contains user data (e.g., a `SpacePacket`).
    TypeD = 0,
    /// The frame contains control information (Type-C).
    TypeC = 1,
}

#[bon]
impl TelecommandTransferFrame {
    /// The size of the Telecommand Transfer Frame header in bytes.
    pub const HEADER_SIZE: usize = 5;
    /// The maximum allowed size of the data field in bytes.
    pub const MAX_DATA_FIELD_LEN: usize = 1019;

    /// Parses a raw byte slice into a zero-copy Telecommand Transfer Frame view.
    pub fn parse(bytes: &[u8]) -> Result<&Self, ParseError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(ParseError::TooShortForHeader {
                actual: bytes.len(),
            });
        }
        let (header, _) = TelecommandTransferFrameHeader::ref_from_prefix(bytes).unwrap();
        let specified_len = header.frame_len();

        if specified_len > bytes.len() {
            return Err(ParseError::IncompleteFrame {
                header_len: specified_len,
                buffer_len: bytes.len(),
            });
        }

        Ok(TelecommandTransferFrame::ref_from_bytes(&bytes[..specified_len]).unwrap())
    }

    /// Returns a reference to the frame's header.
    pub fn header(&self) -> &TelecommandTransferFrameHeader {
        &self.header
    }

    /// Returns a mutable reference to the frame's data field.
    pub fn data_field_mut(&mut self) -> &mut [u8] {
        &mut self.data_field
    }

    /// Returns a reference to the frame's data field.
    pub fn data_field(&self) -> &[u8] {
        &self.data_field
    }

    /// Returns the total length of the frame (header + data field) in bytes.
    pub fn frame_len(&self) -> usize {
        Self::HEADER_SIZE + self.data_field.len()
    }

    /// Constructs a new TC Transfer Frame in the provided buffer.
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        scid: Scid,
        vcid: Vcid,
        bypass_flag: BypassFlag,
        control_flag: ControlFlag,
        seq: u8,
        data_field_len: usize,
    ) -> Result<&mut Self, BuildError> {
        if scid.num_bits() > 10 {
            return Err(BuildError::InvalidScid(scid));
        }
        if vcid.num_bits() > 6 {
            return Err(BuildError::InvalidVcid(vcid));
        }
        if data_field_len > Self::MAX_DATA_FIELD_LEN {
            return Err(BuildError::DataTooLong(data_field_len));
        }

        let total_len = Self::HEADER_SIZE + data_field_len;
        if buffer.len() < total_len {
            return Err(BuildError::BufferTooSmall {
                required: total_len,
                provided: buffer.len(),
            });
        }

        let frame_buf = &mut buffer[..total_len];
        let frame = TelecommandTransferFrame::mut_from_bytes(frame_buf).unwrap();

        frame.header.set_scid(scid);
        frame.header.set_vcid(vcid);
        frame.header.set_bypass_flag(bypass_flag);
        frame.header.set_control_flag(control_flag);
        frame.header.set_sequence_num(seq);
        frame.header.set_frame_len(total_len);

        Ok(frame)
    }
}

impl TelecommandTransferFrameHeader {
    /// Returns the Spacecraft ID (SCID).
    pub fn scid(&self) -> Scid {
        Scid::new(get_bits_u16(self.id_and_scid, SCID_MASK) as u32)
    }
    /// Sets the Spacecraft ID (SCID).
    pub fn set_scid(&mut self, scid: Scid) {
        set_bits_u16(&mut self.id_and_scid, SCID_MASK, scid.get() as u16);
    }

    /// Returns the Virtual Channel ID (VCID).
    pub fn vcid(&self) -> Vcid {
        Vcid::new(get_bits_u16(self.vcid_and_length, VCID_MASK) as u32)
    }
    /// Sets the Virtual Channel ID (VCID).
    pub fn set_vcid(&mut self, vcid: Vcid) {
        set_bits_u16(&mut self.vcid_and_length, VCID_MASK, vcid.get() as u16);
    }

    /// Returns the total frame length in bytes as specified by the header.
    pub fn frame_len(&self) -> usize {
        get_bits_u16(self.vcid_and_length, FRAME_LEN_MASK) as usize + 1
    }
    /// Sets the total frame length in bytes as stored in the header.
    pub fn set_frame_len(&mut self, length: usize) {
        let len_field = (length - 1) as u16;
        set_bits_u16(&mut self.vcid_and_length, FRAME_LEN_MASK, len_field);
    }

    /// Returns the Frame Sequence Number.
    pub fn sequence_num(&self) -> u8 {
        self.sequence_num
    }
    /// Sets the Frame Sequence Number.
    pub fn set_sequence_num(&mut self, seq: u8) {
        self.sequence_num = seq;
    }

    /// Returns the Bypass Flag.
    pub fn bypass_flag(&self) -> BypassFlag {
        if get_bits_u16(self.id_and_scid, BYPASS_FLAG_MASK) == 1 {
            BypassFlag::TypeB
        } else {
            BypassFlag::TypeA
        }
    }
    /// Sets the Bypass Flag.
    pub fn set_bypass_flag(&mut self, flag: BypassFlag) {
        set_bits_u16(&mut self.id_and_scid, BYPASS_FLAG_MASK, flag as u16);
    }

    /// Returns the Control Command Flag.
    pub fn control_flag(&self) -> ControlFlag {
        if get_bits_u16(self.id_and_scid, CONTROL_FLAG_MASK) == 1 {
            ControlFlag::TypeC
        } else {
            ControlFlag::TypeD
        }
    }
    /// Sets the Control Command Flag.
    pub fn set_control_flag(&mut self, flag: ControlFlag) {
        set_bits_u16(&mut self.id_and_scid, CONTROL_FLAG_MASK, flag as u16);
    }
}

// ── FrameWrite / FrameRead implementations ──

/// Configuration for building TC transfer frames.
#[derive(Debug, Clone)]
pub struct TcFrameWriterConfig {
    /// Spacecraft ID.
    pub scid: Scid,
    /// Virtual Channel ID.
    pub vcid: Vcid,
    /// Bypass flag (Type-A or Type-B).
    pub bypass: BypassFlag,
    /// Control flag (data or control command).
    pub control: ControlFlag,
    /// Maximum data field length in bytes.
    pub max_data_field_len: usize,
}

/// Accumulates packets into TC transfer frames.
///
/// Owns its frame buffer internally (sized by `BUF`). Packets
/// are pushed directly into the buffer at the correct offset.
/// [`finish()`](FrameWrite::finish) stamps the header and
/// returns a borrow of the completed frame.
pub struct TcFrameWriter<const BUF: usize> {
    config: TcFrameWriterConfig,
    sequence: u8,
    data_len: usize,
    buf: [u8; BUF],
}

impl<const BUF: usize> TcFrameWriter<BUF> {
    /// Creates a new TC frame writer.
    pub fn new(config: TcFrameWriterConfig) -> Self {
        Self {
            config,
            sequence: 0,
            data_len: 0,
            buf: [0u8; BUF],
        }
    }
}

impl<const BUF: usize> TcFrameWriter<BUF> {
    fn remaining(&self) -> usize {
        self.config.max_data_field_len.saturating_sub(self.data_len)
    }
}

impl<const BUF: usize> FrameWrite for TcFrameWriter<BUF> {
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
        let off = TelecommandTransferFrame::HEADER_SIZE + self.data_len;
        self.buf[off..off + data.len()].copy_from_slice(data);
        self.data_len += data.len();
        Ok(())
    }

    fn finish(&mut self) -> Result<&[u8], BuildError> {
        let total = TelecommandTransferFrame::HEADER_SIZE + self.data_len;
        let seq = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);

        TelecommandTransferFrame::builder()
            .buffer(&mut self.buf[..total])
            .scid(self.config.scid)
            .vcid(self.config.vcid)
            .bypass_flag(self.config.bypass)
            .control_flag(self.config.control)
            .seq(seq)
            .data_field_len(self.data_len)
            .build()?;

        self.data_len = 0;
        Ok(&self.buf[..total])
    }
}

/// Extracts packets from a received TC transfer frame.
///
/// Owns its frame buffer internally (sized by `BUF`). The
/// coding layer writes into
/// [`buffer_mut()`](FrameRead::buffer_mut),
/// [`feed()`](FrameRead::feed) validates the header, and
/// [`next()`](FrameRead::next) returns zero-copy sub-slices.
pub struct TcFrameReader<const BUF: usize> {
    buf: [u8; BUF],
    data_start: usize,
    data_end: usize,
}

impl<const BUF: usize> TcFrameReader<BUF> {
    /// Creates a new TC frame reader.
    pub fn new() -> Self {
        Self {
            buf: [0u8; BUF],
            data_start: 0,
            data_end: 0,
        }
    }
}

impl<const BUF: usize> FrameRead for TcFrameReader<BUF> {
    type Error = ParseError;

    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn feed(&mut self, len: usize) -> Result<(), ParseError> {
        let parsed =
            TelecommandTransferFrame::parse(&self.buf[..len])?;
        let data = parsed.data_field();
        self.data_start = TelecommandTransferFrame::HEADER_SIZE;
        self.data_end = self.data_start + data.len();
        Ok(())
    }

    fn data_field(&self) -> &[u8] {
        &self.buf[self.data_start..self.data_end]
    }
}
