//! CCSDS Proximity-1 Space Link Protocol — Data Link Layer (CCSDS 211.0-B-6)
//!
//! Implements the Version-3 Transfer Frame used for short-range,
//! bi-directional space links (e.g., orbiter-to-lander, rover-to-relay).

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::U16;

use crate::utils::get_bits_u16;
use crate::utils::set_bits_u16;

/// A zero-copy view over a Proximity-1 Version-3 Transfer Frame.
///
/// # Layout
///
/// ```text
/// +--------------------------------------+---------------------+
/// | Field Name                           | Size                |
/// +--------------------------------------+---------------------+
/// | -- Transfer Frame Header (5 bytes) --|                     |
/// |                                      |                     |
/// | Transfer Frame Version Number        | 2 bits              |
/// | Quality of Service (QoS) Indicator   | 1 bit               |
/// | PDU Type ID                          | 1 bit               |
/// | Data Field Construction ID (DFC ID)  | 2 bits              |
/// | Spacecraft Identifier (SCID)         | 10 bits             |
/// | Physical Channel Identifier (PCID)   | 1 bit               |
/// | Port Identifier                      | 3 bits              |
/// | Source-or-Destination Identifier     | 1 bit               |
/// | Frame Length                         | 11 bits             |
/// | Frame Sequence Number (FSN)          | 8 bits              |
/// |                                      |                     |
/// | -- Data Field ------------------------| 0 - 2043 bytes      |
/// +--------------------------------------+---------------------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct Proximity1TransferFrame {
    header: Proximity1Header,
    data_field: [u8],
}

/// The 5-byte header of a Proximity-1 Version-3 Transfer Frame.
#[repr(C)]
#[derive(
    FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable,
    Debug, Copy, Clone,
)]
pub struct Proximity1Header {
    /// Version(2) | QoS(1) | PDU Type(1) | DFC ID(2) | SCID(10).
    version_qos_pdu_dfc_scid: U16,
    /// PCID(1) | Port ID(3) | Src/Dest(1) | Frame Length(11).
    pcid_port_srcdst_len: U16,
    /// Frame Sequence Number (8 bits).
    fsn: u8,
}

/// Bitmasks for the first header word.
#[rustfmt::skip]
pub mod bitmask {
    // -- first U16: version_qos_pdu_dfc_scid --
    /// 2-bit Transfer Frame Version Number (bits 0–1).
    pub const VERSION_MASK: u16  = 0b_1100_0000_0000_0000;
    /// 1-bit Quality of Service indicator (bit 2).
    pub const QOS_MASK: u16      = 0b_0010_0000_0000_0000;
    /// 1-bit PDU Type ID (bit 3).
    pub const PDU_TYPE_MASK: u16 = 0b_0001_0000_0000_0000;
    /// 2-bit Data Field Construction ID (bits 4–5).
    pub const DFC_ID_MASK: u16   = 0b_0000_1100_0000_0000;
    /// 10-bit Spacecraft Identifier (bits 6–15).
    pub const SCID_MASK: u16     = 0b_0000_0011_1111_1111;

    // -- second U16: pcid_port_srcdst_len --
    /// 1-bit Physical Channel Identifier (bit 16).
    pub const PCID_MASK: u16     = 0b_1000_0000_0000_0000;
    /// 3-bit Port Identifier (bits 17–19).
    pub const PORT_ID_MASK: u16  = 0b_0111_0000_0000_0000;
    /// 1-bit Source-or-Destination Identifier (bit 20).
    pub const SRC_DEST_MASK: u16 = 0b_0000_1000_0000_0000;
    /// 11-bit Frame Length field (bits 21–31).
    pub const FRAME_LEN_MASK: u16 = 0b_0000_0111_1111_1111;
}

use bitmask::*;

/// Quality of Service indicator.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum QoS {
    /// Sequence Controlled service (reliable, in-order delivery).
    SequenceControlled = 0,
    /// Expedited service (best-effort, no ARQ).
    Expedited = 1,
}

/// PDU Type — distinguishes user data from supervisory frames.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum PduType {
    /// U-frame: Transfer Frame Data field contains user data.
    UserData = 0,
    /// P-frame: Transfer Frame Data field contains SPDUs.
    Supervisory = 1,
}

/// Data Field Construction Identifier.
///
/// Indicates how the data field of a U-frame is organized.
/// In a P-frame, this shall be set to `Packets` (binary '00').
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum DfcId {
    /// Integer number of unsegmented packets (binary '00').
    Packets = 0b00,
    /// A complete or segmented packet (binary '01').
    Segments = 0b01,
    /// Reserved for future CCSDS definition (binary '10').
    Reserved = 0b10,
    /// User-defined data (binary '11').
    UserDefined = 0b11,
}

/// Source-or-Destination Identifier.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum SrcDest {
    /// SCID identifies the source spacecraft.
    Source = 0,
    /// SCID identifies the destination spacecraft.
    Destination = 1,
}

/// An error that can occur during frame construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    /// SCID exceeds the 10-bit range (0–1023).
    InvalidScid(u16),
    /// Port ID exceeds the 3-bit range (0–7).
    InvalidPortId(u8),
    /// Data field exceeds the maximum of 2043 bytes.
    DataTooLong(usize),
    /// The provided buffer is too small for the frame.
    BufferTooSmall {
        /// Minimum number of bytes needed for the frame.
        required: usize,
        /// Actual buffer size provided.
        provided: usize,
    },
}

/// An error that can occur during frame parsing.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    /// Slice is shorter than the 5-byte header.
    TooShortForHeader {
        /// Actual number of bytes provided.
        actual: usize,
    },
    /// Header length field implies a larger frame than provided.
    IncompleteFrame {
        /// Total frame length declared in the header.
        header_len: usize,
        /// Actual buffer length provided.
        buffer_len: usize,
    },
    /// Version field is not binary '10'.
    InvalidVersion(u8),
}

/// The Proximity-1 Version-3 Transfer Frame version (binary '10').
pub const PROXIMITY1_VERSION: u8 = 0b10;

/// Maximum total frame size in bytes (header + data).
pub const MAX_FRAME_LEN: usize = 2048;

/// Maximum data field size in bytes.
pub const MAX_DATA_FIELD_LEN: usize = 2043;

#[bon]
impl Proximity1TransferFrame {
    /// Header size in bytes.
    pub const HEADER_SIZE: usize = 5;

    /// Parses a raw byte slice into a Proximity-1 Transfer Frame.
    pub fn parse(bytes: &[u8]) -> Result<&Self, ParseError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(ParseError::TooShortForHeader {
                actual: bytes.len(),
            });
        }

        let (header, _) =
            Proximity1Header::ref_from_prefix(bytes).unwrap();
        let total = header.frame_len();

        if total > bytes.len() {
            return Err(ParseError::IncompleteFrame {
                header_len: total,
                buffer_len: bytes.len(),
            });
        }

        let version = header.version();
        if version != PROXIMITY1_VERSION {
            return Err(ParseError::InvalidVersion(version));
        }

        Ok(Self::ref_from_bytes(&bytes[..total]).unwrap())
    }

    /// Parses a mutable byte slice into a Proximity-1 Transfer Frame.
    pub fn parse_mut(
        bytes: &mut [u8],
    ) -> Result<&mut Self, ParseError> {
        if bytes.len() < Self::HEADER_SIZE {
            return Err(ParseError::TooShortForHeader {
                actual: bytes.len(),
            });
        }

        let (header, _) =
            Proximity1Header::ref_from_prefix(bytes).unwrap();
        let total = header.frame_len();
        let version = header.version();

        if total > bytes.len() {
            return Err(ParseError::IncompleteFrame {
                header_len: total,
                buffer_len: bytes.len(),
            });
        }
        if version != PROXIMITY1_VERSION {
            return Err(ParseError::InvalidVersion(version));
        }

        Ok(Self::mut_from_bytes(&mut bytes[..total]).unwrap())
    }

    /// Returns a reference to the header.
    pub fn header(&self) -> &Proximity1Header {
        &self.header
    }

    /// Returns the data field contents.
    pub fn data_field(&self) -> &[u8] {
        &self.data_field
    }

    /// Returns a mutable reference to the data field.
    pub fn data_field_mut(&mut self) -> &mut [u8] {
        &mut self.data_field
    }

    /// Total frame length in bytes (header + data).
    pub fn frame_len(&self) -> usize {
        Self::HEADER_SIZE + self.data_field.len()
    }

    /// Constructs a new Proximity-1 Version-3 Transfer Frame.
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        scid: u16,
        qos: QoS,
        pdu_type: PduType,
        dfc_id: DfcId,
        pcid: bool,
        port_id: u8,
        src_dest: SrcDest,
        fsn: u8,
        data_field_len: usize,
    ) -> Result<&mut Self, BuildError> {
        if scid > 0x3FF {
            return Err(BuildError::InvalidScid(scid));
        }
        if port_id > 0x07 {
            return Err(BuildError::InvalidPortId(port_id));
        }
        if data_field_len > MAX_DATA_FIELD_LEN {
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
        let frame = Self::mut_from_bytes(frame_buf).unwrap();

        frame.header.set_version(PROXIMITY1_VERSION);
        frame.header.set_qos(qos);
        frame.header.set_pdu_type(pdu_type);
        frame.header.set_dfc_id(dfc_id);
        frame.header.set_scid(scid);
        frame.header.set_pcid(pcid);
        frame.header.set_port_id(port_id);
        frame.header.set_src_dest(src_dest);
        frame.header.set_frame_len(total_len);
        frame.header.set_fsn(fsn);

        Ok(frame)
    }
}

// ── Header field accessors ──────────────────────────────────────

impl Proximity1Header {
    /// Returns the 2-bit Transfer Frame Version Number.
    pub fn version(&self) -> u8 {
        get_bits_u16(self.version_qos_pdu_dfc_scid, VERSION_MASK)
            as u8
    }
    /// Sets the 2-bit Transfer Frame Version Number.
    pub fn set_version(&mut self, v: u8) {
        set_bits_u16(
            &mut self.version_qos_pdu_dfc_scid,
            VERSION_MASK,
            v as u16,
        );
    }

    /// Returns the Quality of Service indicator.
    pub fn qos(&self) -> QoS {
        if get_bits_u16(self.version_qos_pdu_dfc_scid, QOS_MASK) == 1
        {
            QoS::Expedited
        } else {
            QoS::SequenceControlled
        }
    }
    /// Sets the Quality of Service indicator.
    pub fn set_qos(&mut self, qos: QoS) {
        set_bits_u16(
            &mut self.version_qos_pdu_dfc_scid,
            QOS_MASK,
            qos as u16,
        );
    }

    /// Returns the PDU Type ID.
    pub fn pdu_type(&self) -> PduType {
        if get_bits_u16(self.version_qos_pdu_dfc_scid, PDU_TYPE_MASK)
            == 1
        {
            PduType::Supervisory
        } else {
            PduType::UserData
        }
    }
    /// Sets the PDU Type ID.
    pub fn set_pdu_type(&mut self, t: PduType) {
        set_bits_u16(
            &mut self.version_qos_pdu_dfc_scid,
            PDU_TYPE_MASK,
            t as u16,
        );
    }

    /// Returns the 2-bit Data Field Construction ID.
    pub fn dfc_id(&self) -> DfcId {
        let v = get_bits_u16(
            self.version_qos_pdu_dfc_scid,
            DFC_ID_MASK,
        );
        match v {
            0b00 => DfcId::Packets,
            0b01 => DfcId::Segments,
            0b10 => DfcId::Reserved,
            _ => DfcId::UserDefined,
        }
    }
    /// Sets the 2-bit Data Field Construction ID.
    pub fn set_dfc_id(&mut self, dfc: DfcId) {
        set_bits_u16(
            &mut self.version_qos_pdu_dfc_scid,
            DFC_ID_MASK,
            dfc as u16,
        );
    }

    /// Returns the 10-bit Spacecraft Identifier.
    pub fn scid(&self) -> u16 {
        get_bits_u16(self.version_qos_pdu_dfc_scid, SCID_MASK)
    }
    /// Sets the 10-bit Spacecraft Identifier.
    pub fn set_scid(&mut self, scid: u16) {
        set_bits_u16(
            &mut self.version_qos_pdu_dfc_scid,
            SCID_MASK,
            scid,
        );
    }

    /// Returns the Physical Channel Identifier.
    pub fn pcid(&self) -> bool {
        get_bits_u16(self.pcid_port_srcdst_len, PCID_MASK) != 0
    }
    /// Sets the Physical Channel Identifier.
    pub fn set_pcid(&mut self, pcid: bool) {
        set_bits_u16(
            &mut self.pcid_port_srcdst_len,
            PCID_MASK,
            u16::from(pcid),
        );
    }

    /// Returns the 3-bit Port Identifier.
    pub fn port_id(&self) -> u8 {
        get_bits_u16(self.pcid_port_srcdst_len, PORT_ID_MASK) as u8
    }
    /// Sets the 3-bit Port Identifier.
    pub fn set_port_id(&mut self, id: u8) {
        set_bits_u16(
            &mut self.pcid_port_srcdst_len,
            PORT_ID_MASK,
            id as u16,
        );
    }

    /// Returns the Source-or-Destination Identifier.
    pub fn src_dest(&self) -> SrcDest {
        if get_bits_u16(self.pcid_port_srcdst_len, SRC_DEST_MASK) == 1
        {
            SrcDest::Destination
        } else {
            SrcDest::Source
        }
    }
    /// Sets the Source-or-Destination Identifier.
    pub fn set_src_dest(&mut self, sd: SrcDest) {
        set_bits_u16(
            &mut self.pcid_port_srcdst_len,
            SRC_DEST_MASK,
            sd as u16,
        );
    }

    /// Returns the total frame length in bytes.
    ///
    /// The header stores `C = total_octets - 1`.
    pub fn frame_len(&self) -> usize {
        get_bits_u16(self.pcid_port_srcdst_len, FRAME_LEN_MASK)
            as usize
            + 1
    }
    /// Sets the frame length from total byte count.
    pub fn set_frame_len(&mut self, len: usize) {
        set_bits_u16(
            &mut self.pcid_port_srcdst_len,
            FRAME_LEN_MASK,
            (len - 1) as u16,
        );
    }

    /// Returns the 8-bit Frame Sequence Number.
    pub fn fsn(&self) -> u8 {
        self.fsn
    }
    /// Sets the 8-bit Frame Sequence Number.
    pub fn set_fsn(&mut self, fsn: u8) {
        self.fsn = fsn;
    }
}

// ── Proximity Link Control Word (PLCW) ──────────────────────────
//
// The PLCW is a 16-bit fixed-length SPDU (Type F1) carried in the
// data field of a P-frame. It reports the receiver's state back to
// the sender for COP-P sequence control.
//
// Layout (Figure 3-5, CCSDS 211.0-B-6):
//
//   Bit 0:      SPDU Format ID       (1 = fixed-length)
//   Bit 1:      SPDU Type Identifier (0 = PLCW)
//   Bit 2:      Retransmit Flag
//   Bit 3:      PCID
//   Bit 4:      Reserved Spare       (always 0)
//   Bits 5-7:   Expedited Frame Counter (mod-8)
//   Bits 8-15:  Report Value V(R)

/// Bitmasks for the 16-bit PLCW.
#[rustfmt::skip]
pub mod plcw_bitmask {
    /// SPDU Format ID (bit 0) — always 1 for fixed-length.
    pub const FORMAT_ID_MASK: u16     = 0b_1000_0000_0000_0000;
    /// SPDU Type Identifier (bit 1) — 0 = PLCW.
    pub const TYPE_ID_MASK: u16       = 0b_0100_0000_0000_0000;
    /// Retransmit Flag (bit 2).
    pub const RETRANSMIT_MASK: u16    = 0b_0010_0000_0000_0000;
    /// PCID (bit 3).
    pub const PLCW_PCID_MASK: u16     = 0b_0001_0000_0000_0000;
    /// Reserved Spare (bit 4) — always 0.
    pub const _RESERVED_MASK: u16     = 0b_0000_1000_0000_0000;
    /// Expedited Frame Counter (bits 5-7), mod-8.
    pub const EXP_COUNTER_MASK: u16   = 0b_0000_0111_0000_0000;
    /// Report Value V(R) (bits 8-15).
    pub const REPORT_VALUE_MASK: u16  = 0b_0000_0000_1111_1111;
}

/// A Proximity Link Control Word (PLCW).
///
/// This is a 16-bit fixed-length SPDU that reports the receiver's
/// state (V(R), retransmit flag, expedited frame counter) back to
/// the sender via a P-frame.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Plcw(u16);

impl Plcw {
    /// Creates a new PLCW with the given fields.
    pub fn new(
        retransmit: bool,
        pcid: bool,
        expedited_counter: u8,
        report_value: u8,
    ) -> Self {
        use plcw_bitmask::*;
        let mut val = 0u16;
        // Format ID = 1 (fixed-length SPDU)
        val |= FORMAT_ID_MASK;
        // Type ID = 0 (PLCW) — already 0
        if retransmit {
            val |= RETRANSMIT_MASK;
        }
        if pcid {
            val |= PLCW_PCID_MASK;
        }
        val |= ((expedited_counter & 0x07) as u16)
            << EXP_COUNTER_MASK.trailing_zeros();
        val |= report_value as u16;
        Self(val)
    }

    /// Parses a PLCW from a 2-byte big-endian slice.
    pub fn from_bytes(bytes: &[u8; 2]) -> Self {
        Self(u16::from_be_bytes(*bytes))
    }

    /// Encodes the PLCW as 2 big-endian bytes.
    pub fn to_bytes(self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    /// Returns the raw 16-bit value.
    pub fn raw(self) -> u16 {
        self.0
    }

    /// Returns true if the Retransmit Flag is set.
    ///
    /// When set, the sender should retransmit the expected frame.
    pub fn retransmit(&self) -> bool {
        self.0 & plcw_bitmask::RETRANSMIT_MASK != 0
    }

    /// Returns the PCID field.
    pub fn pcid(&self) -> bool {
        self.0 & plcw_bitmask::PLCW_PCID_MASK != 0
    }

    /// Returns the 3-bit Expedited Frame Counter (mod-8).
    pub fn expedited_counter(&self) -> u8 {
        ((self.0 & plcw_bitmask::EXP_COUNTER_MASK)
            >> plcw_bitmask::EXP_COUNTER_MASK.trailing_zeros())
            as u8
    }

    /// Returns the 8-bit Report Value V(R).
    ///
    /// This is the next expected sequence-controlled FSN.
    pub fn report_value(&self) -> u8 {
        (self.0 & plcw_bitmask::REPORT_VALUE_MASK) as u8
    }
}

impl core::fmt::Display for Plcw {
    fn fmt(
        &self,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(
            f,
            "PLCW[rt={} pcid={} exp={} vr={}]",
            self.retransmit() as u8,
            self.pcid() as u8,
            self.expedited_counter(),
            self.report_value(),
        )
    }
}

// ── Segment Header ──────────────────────────────────────────────
//
// When DFC ID = 01 (Segments), the data field starts with an 8-bit
// segment header (§3.2.3.3):
//
//   Bits 0-1: Sequence Flags
//   Bits 2-7: Pseudo Packet ID

/// Sequence flags for segmented data units.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum SequenceFlag {
    /// First segment of a packet (binary '01').
    First = 0b01,
    /// Continuation segment (binary '00').
    Continuation = 0b00,
    /// Last segment of a packet (binary '10').
    Last = 0b10,
    /// No segmentation — entire packet (binary '11').
    Unsegmented = 0b11,
}

/// An 8-bit segment header prepended to the data when DFC ID = 01.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct SegmentHeader(u8);

impl SegmentHeader {
    /// Creates a new segment header.
    pub fn new(flags: SequenceFlag, pseudo_packet_id: u8) -> Self {
        Self((flags as u8) << 6 | (pseudo_packet_id & 0x3F))
    }

    /// Parses a segment header from a byte.
    pub fn from_byte(b: u8) -> Self {
        Self(b)
    }

    /// Returns the raw byte value.
    pub fn to_byte(self) -> u8 {
        self.0
    }

    /// Returns the sequence flags.
    pub fn flags(&self) -> SequenceFlag {
        match self.0 >> 6 {
            0b01 => SequenceFlag::First,
            0b00 => SequenceFlag::Continuation,
            0b10 => SequenceFlag::Last,
            _ => SequenceFlag::Unsegmented,
        }
    }

    /// Returns the 6-bit pseudo packet identifier.
    pub fn pseudo_packet_id(&self) -> u8 {
        self.0 & 0x3F
    }
}

// ── Display ─────────────────────────────────────────────────────

impl core::fmt::Display for Proximity1Header {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Prox1[v={} qos={:?} pdu={:?} dfc={:?} scid={} \
             pcid={} port={} sd={:?} len={} fsn={}]",
            self.version(),
            self.qos(),
            self.pdu_type(),
            self.dfc_id(),
            self.scid(),
            self.pcid() as u8,
            self.port_id(),
            self.src_dest(),
            self.frame_len(),
            self.fsn(),
        )
    }
}

impl core::fmt::Display for Proximity1TransferFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{} data=[{}B]",
            self.header,
            self.data_field.len(),
        )
    }
}

// ── FrameWrite / FrameRead implementations ──

use super::super::{FrameRead, FrameWrite, PushError};

/// Configuration for building Proximity-1 transfer frames.
#[derive(Debug, Clone)]
pub struct Prox1FrameWriterConfig {
    /// Spacecraft ID (10-bit).
    pub scid: u16,
    /// Quality of Service indicator.
    pub qos: QoS,
    /// PDU type (user data or supervisory).
    pub pdu_type: PduType,
    /// Data field construction identifier.
    pub dfc_id: DfcId,
    /// Physical channel identifier.
    pub pcid: bool,
    /// Port identifier (3-bit).
    pub port_id: u8,
    /// Source-or-destination identifier.
    pub src_dest: SrcDest,
    /// Maximum data field length in bytes.
    pub max_data_field_len: usize,
}

/// Accumulates packets into Proximity-1 transfer frames.
///
/// Owns its frame buffer internally (sized by `BUF`). Packets
/// are pushed directly into the buffer at the correct offset.
/// [`finish()`](FrameWrite::finish) stamps the header and
/// returns a borrow of the completed frame.
pub struct Prox1FrameWriter<const BUF: usize> {
    config: Prox1FrameWriterConfig,
    fsn: u8,
    data_len: usize,
    buf: [u8; BUF],
}

impl<const BUF: usize> Prox1FrameWriter<BUF> {
    /// Creates a new Proximity-1 frame writer.
    pub fn new(config: Prox1FrameWriterConfig) -> Self {
        Self {
            config,
            fsn: 0,
            data_len: 0,
            buf: [0u8; BUF],
        }
    }
}

impl<const BUF: usize> Prox1FrameWriter<BUF> {
    fn remaining(&self) -> usize {
        self.config.max_data_field_len.saturating_sub(self.data_len)
    }
}

impl<const BUF: usize> FrameWrite for Prox1FrameWriter<BUF> {
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
            Proximity1TransferFrame::HEADER_SIZE + self.data_len;
        self.buf[off..off + data.len()].copy_from_slice(data);
        self.data_len += data.len();
        Ok(())
    }

    fn finish(&mut self) -> Result<&[u8], BuildError> {
        let total =
            Proximity1TransferFrame::HEADER_SIZE + self.data_len;
        let fsn = self.fsn;
        self.fsn = self.fsn.wrapping_add(1);

        Proximity1TransferFrame::builder()
            .buffer(&mut self.buf[..total])
            .scid(self.config.scid)
            .qos(self.config.qos)
            .pdu_type(self.config.pdu_type)
            .dfc_id(self.config.dfc_id)
            .pcid(self.config.pcid)
            .port_id(self.config.port_id)
            .src_dest(self.config.src_dest)
            .fsn(fsn)
            .data_field_len(self.data_len)
            .build()?;

        self.data_len = 0;
        Ok(&self.buf[..total])
    }
}

/// Extracts packets from a received Proximity-1 transfer frame.
///
/// Owns its frame buffer internally (sized by `BUF`). The
/// coding layer writes into
/// [`buffer_mut()`](FrameRead::buffer_mut),
/// [`feed()`](FrameRead::feed) validates the header, and
/// [`next()`](FrameRead::next) returns zero-copy sub-slices.
pub struct Prox1FrameReader<const BUF: usize> {
    buf: [u8; BUF],
    data_start: usize,
    data_end: usize,
}

impl<const BUF: usize> Prox1FrameReader<BUF> {
    /// Creates a new Proximity-1 frame reader.
    pub fn new() -> Self {
        Self {
            buf: [0u8; BUF],
            data_start: 0,
            data_end: 0,
        }
    }
}

impl<const BUF: usize> FrameRead for Prox1FrameReader<BUF> {
    type Error = ParseError;

    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn feed(&mut self, len: usize) -> Result<(), ParseError> {
        let parsed =
            Proximity1TransferFrame::parse(&self.buf[..len])?;
        let data = parsed.data_field();
        self.data_start = Proximity1TransferFrame::HEADER_SIZE;
        self.data_end = self.data_start + data.len();
        Ok(())
    }

    fn data_field(&self) -> &[u8] {
        &self.buf[self.data_start..self.data_end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse_u_frame() {
        let mut buf = [0u8; 256];
        let payload = [0xDE, 0xAD, 0xBE, 0xEF];

        let frame = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(42)
            .qos(QoS::SequenceControlled)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::Packets)
            .pcid(false)
            .port_id(3)
            .src_dest(SrcDest::Source)
            .fsn(0x7F)
            .data_field_len(payload.len())
            .build()
            .unwrap();

        frame.data_field_mut().copy_from_slice(&payload);

        assert_eq!(frame.header().version(), PROXIMITY1_VERSION);
        assert_eq!(frame.header().qos(), QoS::SequenceControlled);
        assert_eq!(frame.header().pdu_type(), PduType::UserData);
        assert_eq!(frame.header().dfc_id(), DfcId::Packets);
        assert_eq!(frame.header().scid(), 42);
        assert!(!frame.header().pcid());
        assert_eq!(frame.header().port_id(), 3);
        assert_eq!(frame.header().src_dest(), SrcDest::Source);
        assert_eq!(frame.header().frame_len(), 5 + payload.len());
        assert_eq!(frame.header().fsn(), 0x7F);
        assert_eq!(frame.data_field(), &payload);
    }

    #[test]
    fn build_and_parse_p_frame() {
        let mut buf = [0u8; 64];

        let frame = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(1023)
            .qos(QoS::Expedited)
            .pdu_type(PduType::Supervisory)
            .dfc_id(DfcId::Packets) // must be 00 for P-frames
            .pcid(true)
            .port_id(0) // must be 0 for P-frames
            .src_dest(SrcDest::Destination)
            .fsn(255)
            .data_field_len(8)
            .build()
            .unwrap();

        assert_eq!(frame.header().scid(), 1023);
        assert_eq!(frame.header().qos(), QoS::Expedited);
        assert_eq!(frame.header().pdu_type(), PduType::Supervisory);
        assert!(frame.header().pcid());
        assert_eq!(frame.header().src_dest(), SrcDest::Destination);
        assert_eq!(frame.header().fsn(), 255);
    }

    #[test]
    fn parse_roundtrip() {
        let mut buf = [0u8; 128];

        let frame = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(500)
            .qos(QoS::Expedited)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::UserDefined)
            .pcid(false)
            .port_id(7)
            .src_dest(SrcDest::Source)
            .fsn(42)
            .data_field_len(10)
            .build()
            .unwrap();

        let total = frame.frame_len();

        let parsed =
            Proximity1TransferFrame::parse(&buf[..total]).unwrap();
        assert_eq!(parsed.header().scid(), 500);
        assert_eq!(parsed.header().qos(), QoS::Expedited);
        assert_eq!(parsed.header().dfc_id(), DfcId::UserDefined);
        assert_eq!(parsed.header().port_id(), 7);
        assert_eq!(parsed.header().fsn(), 42);
        assert_eq!(parsed.data_field().len(), 10);
    }

    #[test]
    fn invalid_scid_rejected() {
        let mut buf = [0u8; 64];
        let err = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(1024) // exceeds 10-bit max
            .qos(QoS::SequenceControlled)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::Packets)
            .pcid(false)
            .port_id(0)
            .src_dest(SrcDest::Source)
            .fsn(0)
            .data_field_len(1)
            .build();
        assert!(matches!(err, Err(BuildError::InvalidScid(1024))));
    }

    #[test]
    fn invalid_port_id_rejected() {
        let mut buf = [0u8; 64];
        let err = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .qos(QoS::SequenceControlled)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::Packets)
            .pcid(false)
            .port_id(8) // exceeds 3-bit max
            .src_dest(SrcDest::Source)
            .fsn(0)
            .data_field_len(1)
            .build();
        assert!(matches!(err, Err(BuildError::InvalidPortId(8))));
    }

    #[test]
    fn data_too_long_rejected() {
        let mut buf = [0u8; 64];
        let err = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .qos(QoS::SequenceControlled)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::Packets)
            .pcid(false)
            .port_id(0)
            .src_dest(SrcDest::Source)
            .fsn(0)
            .data_field_len(2044) // exceeds 2043
            .build();
        assert!(matches!(err, Err(BuildError::DataTooLong(2044))));
    }

    #[test]
    fn buffer_too_small_rejected() {
        let mut buf = [0u8; 4]; // less than header
        let err = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .qos(QoS::SequenceControlled)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::Packets)
            .pcid(false)
            .port_id(0)
            .src_dest(SrcDest::Source)
            .fsn(0)
            .data_field_len(1)
            .build();
        assert!(matches!(
            err,
            Err(BuildError::BufferTooSmall {
                required: 6,
                provided: 4,
            })
        ));
    }

    #[test]
    fn parse_too_short() {
        let buf = [0u8; 3];
        let err = Proximity1TransferFrame::parse(&buf);
        assert!(matches!(
            err,
            Err(ParseError::TooShortForHeader { actual: 3 })
        ));
    }

    #[test]
    fn parse_invalid_version() {
        let mut buf = [0u8; 16];
        // Manually set version to 0b00 (wrong) and length
        buf[0] = 0x00;
        buf[1] = 0x00;
        buf[2] = 0x00;
        buf[3] = 0x0F; // frame length C=15 → 16 bytes total
        let err = Proximity1TransferFrame::parse(&buf);
        assert!(matches!(
            err,
            Err(ParseError::InvalidVersion(0))
        ));
    }

    #[test]
    fn max_frame_size() {
        let mut buf = [0u8; MAX_FRAME_LEN];
        let frame = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .qos(QoS::Expedited)
            .pdu_type(PduType::UserData)
            .dfc_id(DfcId::UserDefined)
            .pcid(false)
            .port_id(0)
            .src_dest(SrcDest::Source)
            .fsn(0)
            .data_field_len(MAX_DATA_FIELD_LEN)
            .build()
            .unwrap();

        assert_eq!(frame.frame_len(), MAX_FRAME_LEN);
        assert_eq!(frame.data_field().len(), MAX_DATA_FIELD_LEN);
    }

    #[test]
    fn all_dfc_id_variants() {
        for (dfc, expected_raw) in [
            (DfcId::Packets, 0b00u8),
            (DfcId::Segments, 0b01),
            (DfcId::Reserved, 0b10),
            (DfcId::UserDefined, 0b11),
        ] {
            let mut buf = [0u8; 16];
            let frame = Proximity1TransferFrame::builder()
                .buffer(&mut buf)
                .scid(0)
                .qos(QoS::SequenceControlled)
                .pdu_type(PduType::UserData)
                .dfc_id(dfc)
                .pcid(false)
                .port_id(0)
                .src_dest(SrcDest::Source)
                .fsn(0)
                .data_field_len(1)
                .build()
                .unwrap();

            assert_eq!(frame.header().dfc_id(), dfc);
            let raw = get_bits_u16(
                frame.header().version_qos_pdu_dfc_scid,
                DFC_ID_MASK,
            );
            assert_eq!(raw as u8, expected_raw);
        }
    }

    #[test]
    fn display_format() {
        let mut buf = [0u8; 16];
        let frame = Proximity1TransferFrame::builder()
            .buffer(&mut buf)
            .scid(100)
            .qos(QoS::Expedited)
            .pdu_type(PduType::Supervisory)
            .dfc_id(DfcId::Packets)
            .pcid(true)
            .port_id(5)
            .src_dest(SrcDest::Destination)
            .fsn(77)
            .data_field_len(4)
            .build()
            .unwrap();

        let mut out = [0u8; 128];
        let n = crate::fmt!(&mut out, "{}", frame).unwrap();
        let s = core::str::from_utf8(&out[..n]).unwrap();
        assert!(s.contains("scid=100"));
        assert!(s.contains("fsn=77"));
    }

    // ── PLCW tests ──────────────────────────────────────────────

    #[test]
    fn plcw_roundtrip() {
        let plcw = Plcw::new(true, false, 5, 0xAB);
        assert!(plcw.retransmit());
        assert!(!plcw.pcid());
        assert_eq!(plcw.expedited_counter(), 5);
        assert_eq!(plcw.report_value(), 0xAB);

        let bytes = plcw.to_bytes();
        let parsed = Plcw::from_bytes(&bytes);
        assert_eq!(parsed, plcw);
    }

    #[test]
    fn plcw_format_id_always_set() {
        let plcw = Plcw::new(false, false, 0, 0);
        // Bit 0 (MSB) should always be 1
        assert!(plcw.raw() & plcw_bitmask::FORMAT_ID_MASK != 0);
        // Type ID should be 0 for PLCW
        assert!(plcw.raw() & plcw_bitmask::TYPE_ID_MASK == 0);
    }

    #[test]
    fn plcw_all_fields() {
        let plcw = Plcw::new(false, true, 7, 255);
        assert!(!plcw.retransmit());
        assert!(plcw.pcid());
        assert_eq!(plcw.expedited_counter(), 7);
        assert_eq!(plcw.report_value(), 255);
    }

    #[test]
    fn plcw_display() {
        let plcw = Plcw::new(true, false, 3, 42);
        let mut out = [0u8; 64];
        let n = crate::fmt!(&mut out, "{}", plcw).unwrap();
        let s = core::str::from_utf8(&out[..n]).unwrap();
        assert!(s.contains("rt=1"));
        assert!(s.contains("vr=42"));
    }

    // ── Segment Header tests ────────────────────────────────────

    #[test]
    fn segment_header_roundtrip() {
        let hdr = SegmentHeader::new(SequenceFlag::First, 42);
        assert_eq!(hdr.flags(), SequenceFlag::First);
        assert_eq!(hdr.pseudo_packet_id(), 42);

        let b = hdr.to_byte();
        let parsed = SegmentHeader::from_byte(b);
        assert_eq!(parsed, hdr);
    }

    #[test]
    fn segment_header_all_flags() {
        for (flag, expected_bits) in [
            (SequenceFlag::First, 0b01),
            (SequenceFlag::Continuation, 0b00),
            (SequenceFlag::Last, 0b10),
            (SequenceFlag::Unsegmented, 0b11),
        ] {
            let hdr = SegmentHeader::new(flag, 0);
            assert_eq!(hdr.flags(), flag);
            assert_eq!(hdr.to_byte() >> 6, expected_bits);
        }
    }

    #[test]
    fn segment_header_pseudo_id_masked() {
        // Only 6 bits should be kept
        let hdr = SegmentHeader::new(SequenceFlag::Unsegmented, 0xFF);
        assert_eq!(hdr.pseudo_packet_id(), 0x3F);
    }
}
