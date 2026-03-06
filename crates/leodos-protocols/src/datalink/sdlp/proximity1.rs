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
}
