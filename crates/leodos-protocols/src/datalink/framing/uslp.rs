//! Unified Space Data Link Protocol (USLP)
//!
//! Spec: <https://ccsds.org/Pubs/732x1b3e1.pdf>
//!
//! USLP (CCSDS 732.1-B-3) defines a unified transfer frame format
//! that replaces the separate TM, TC, and AOS frame protocols.
//! It supports both fixed-length and variable-length frames with
//! a variable-size primary header (4-14 bytes).
//!
//! # Frame Layout
//!
//! ```text
//! +-------------------------------------+---------+
//! | Field                               | Size    |
//! +-------------------------------------+---------+
//! | Primary Header (fixed part)         | 7 bytes |
//! |   TFVN + SCID + Src/Dst + VCID      |         |
//! |   + MAP ID + EOFPH Flag             | 4 bytes |
//! |   Frame Length                      | 2 bytes |
//! |   Flags + VCF Count Length          | 1 byte  |
//! | Primary Header (VCF Count)          | 0-7 B   |
//! +-------------------------------------+---------+
//! | Insert Zone (optional)              | Varies  |
//! +-------------------------------------+---------+
//! | Transfer Frame Data Field           |         |
//! |   TFDF Header (rules + UPID)        | 1 byte  |
//! |   First Header / Last Octet Ptr     | 0 or 2  |
//! |   Transfer Frame Data Zone          | Varies  |
//! +-------------------------------------+---------+
//! | Operational Control Field (opt)     | 4 bytes |
//! +-------------------------------------+---------+
//! | Frame Error Control Field (opt)     | 2 bytes |
//! +-------------------------------------+---------+
//! ```

use bon::bon;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian::{U16, U32};

use crate::utils::{get_bits_u8, get_bits_u32, set_bits_u8, set_bits_u32};

/// USLP Transfer Frame Version Number (`0b1100` = 12).
pub const USLP_TFVN: u8 = 0b1100;

/// VCID reserved for Only Idle Data (OID) Transfer Frames.
pub const OID_VCID: u8 = 63;

/// Bitmasks for USLP Transfer Frame header fields.
#[rustfmt::skip]
pub mod bitmask {
    // --- Primary Header ID word (U32, bytes 0-3) ---

    /// 4-bit Transfer Frame Version Number (CCSDS bits 0-3).
    pub const TFVN_MASK: u32 =          0xF000_0000;
    /// 16-bit Spacecraft Identifier (CCSDS bits 4-19).
    pub const SCID_MASK: u32 =          0x0FFF_F000;
    /// 1-bit Source-or-Destination Identifier (CCSDS bit 20).
    pub const SRC_DEST_MASK: u32 =      0x0000_0800;
    /// 6-bit Virtual Channel Identifier (CCSDS bits 21-26).
    pub const VCID_MASK: u32 =          0x0000_07E0;
    /// 4-bit Multiplexer Access Point ID (CCSDS bits 27-30).
    pub const MAP_ID_MASK: u32 =        0x0000_001E;
    /// 1-bit End of Frame Primary Header Flag (CCSDS bit 31).
    pub const EOFPH_MASK: u32 =         0x0000_0001;

    // --- Flags byte (byte 6) ---

    /// 1-bit Bypass/Sequence Control Flag.
    pub const BYPASS_FLAG_MASK: u8 =    0b_1000_0000;
    /// 1-bit Protocol Control Command Flag.
    pub const PCC_FLAG_MASK: u8 =       0b_0100_0000;
    /// 2-bit Reserve Spares (shall be `00`).
    pub const SPARE_MASK: u8 =          0b_0011_0000;
    /// 1-bit Operational Control Field Flag.
    pub const OCF_FLAG_MASK: u8 =       0b_0000_1000;
    /// 3-bit VC Frame Count Length.
    pub const VCF_COUNT_LEN_MASK: u8 =  0b_0000_0111;

    // --- TFDF Header (first byte of Transfer Frame Data Field) ---

    /// 3-bit TFDZ Construction Rules.
    pub const TFDZ_RULES_MASK: u8 =     0b_1110_0000;
    /// 5-bit USLP Protocol Identifier.
    pub const UPID_MASK: u8 =           0b_0001_1111;
}

use bitmask::*;

/// TFDZ Construction Rules (CCSDS 732.1-B-3, Table 4-3).
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum TfdzRule {
    /// Packets spanning multiple frames (fixed-length TFDZ).
    PacketsSpanning = 0,
    /// Start of MAPA_SDU or VCA_SDU (fixed-length TFDZ).
    SduStart = 1,
    /// Continuing portion of MAPA_SDU or VCA_SDU (fixed).
    SduContinue = 2,
    /// Octet stream, continuous (variable-length TFDZ).
    OctetStream = 3,
    /// Starting segment of a large SDU (variable-length).
    StartingSegment = 4,
    /// Continuing segment (variable-length).
    ContinuingSegment = 5,
    /// Last segment (variable-length).
    LastSegment = 6,
    /// No segmentation (variable-length TFDZ).
    NoSegmentation = 7,
}

impl TfdzRule {
    /// Parses from a 3-bit raw value.
    pub fn from_bits(bits: u8) -> Self {
        match bits & 0x07 {
            0 => Self::PacketsSpanning,
            1 => Self::SduStart,
            2 => Self::SduContinue,
            3 => Self::OctetStream,
            4 => Self::StartingSegment,
            5 => Self::ContinuingSegment,
            6 => Self::LastSegment,
            _ => Self::NoSegmentation,
        }
    }

    /// Whether this rule uses the optional 16-bit pointer.
    pub fn has_pointer(self) -> bool {
        matches!(
            self,
            Self::PacketsSpanning | Self::SduStart | Self::SduContinue
        )
    }
}

/// Well-known USLP Protocol Identifier (UPID) values.
///
/// These are registered in the SANA UPID registry.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
pub enum Upid {
    /// CCSDS Space Packets.
    SpacePackets = 0,
    /// CCSDS Encapsulation Packets.
    EncapsulationPackets = 1,
    /// COP-1 Control Commands.
    Cop1Commands = 2,
    /// COP-P / Proximity-1 SPDUs.
    CopPCommands = 3,
    /// SDLS Protocol data.
    Sdls = 4,
    /// Idle data in a fixed-length TFDZ.
    IdleData = 30,
    /// Only Idle Data (OID Transfer Frames).
    OnlyIdleData = 31,
}

/// An error during USLP frame construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuildError {
    /// VCID exceeds 6-bit range (0-63).
    InvalidVcid(u8),
    /// MAP ID exceeds 4-bit range (0-15).
    InvalidMapId(u8),
    /// VCF Count Length exceeds 3-bit range (0-7).
    InvalidVcfCountLength(u8),
    /// Buffer too small for the frame.
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required_len: usize,
        /// Actual buffer size provided.
        provided_len: usize,
    },
    /// Frame exceeds the 16-bit Frame Length limit (65536 bytes).
    FrameTooLarge {
        /// Maximum allowed length.
        max_len: usize,
        /// Actual buffer size provided.
        provided_len: usize,
    },
}

/// An error during USLP frame parsing.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParseError {
    /// Buffer too short for the 7-byte fixed header.
    TooShortForHeader,
    /// Frame Length field does not match the buffer size.
    LengthMismatch {
        /// Expected from the Frame Length field.
        expected: usize,
        /// Actual buffer size.
        actual: usize,
    },
    /// TFVN is not `0b1100` (Version-4).
    InvalidVersion(u8),
    /// Truncated header (EOFPH=1) is not supported.
    TruncatedHeader,
}

/// The 7-byte fixed portion of a non-truncated USLP Primary Header.
///
/// Covers bytes 0-6: identification fields, frame length, and
/// control flags. The variable-length VCF Count (0-7 bytes)
/// follows immediately after this in the frame body.
///
/// ```text
/// +--------+----------+------+------+------+------+
/// | TFVN   | SCID     | S/D  | VCID | MAP  | EOFPH|
/// | 4 bits | 16 bits  | 1b   | 6b   | 4b   | 1b   |
/// +--------+----------+------+------+------+------+
/// | Frame Length             | Flags byte         |
/// | 16 bits                  | Byp PCC Sp OCF VCL |
/// +--------+----------+------+------+------+------+
/// ```
#[repr(C, packed)]
#[derive(
    FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable,
    Debug, Copy, Clone,
)]
pub struct UslpPrimaryHeaderFixed {
    /// Bytes 0-3: TFVN(4) + SCID(16) + Src/Dst(1) + VCID(6)
    /// + MAP_ID(4) + EOFPH(1).
    id: U32,
    /// Bytes 4-5: Frame Length (total octets - 1).
    frame_length: U16,
    /// Byte 6: Bypass(1) + PCC(1) + Spare(2) + OCF(1) +
    /// VCF Count Length(3).
    flags: u8,
}

/// A zero-copy view over a USLP Transfer Frame.
///
/// The 7-byte fixed header is followed by a variable-length body
/// containing the VCF Count, optional Insert Zone, Transfer Frame
/// Data Field (TFDF Header + TFDZ), optional OCF, and optional
/// FECF.
#[repr(C, packed)]
#[derive(IntoBytes, FromBytes, Unaligned, KnownLayout, Immutable)]
pub struct UslpTransferFrame {
    header: UslpPrimaryHeaderFixed,
    body: [u8],
}

#[bon]
impl UslpTransferFrame {
    /// Size of the fixed portion of the primary header (bytes).
    pub const FIXED_HEADER_SIZE: usize = 7;

    /// Maximum total frame size (16-bit Frame Length field).
    pub const MAX_FRAME_SIZE: usize = 65536;

    /// Parses a byte slice into a USLP Transfer Frame view.
    ///
    /// Validates the TFVN (must be `0b1100`), the EOFPH flag
    /// (truncated frames are not supported), and the Frame Length
    /// field consistency with the buffer size.
    pub fn parse(bytes: &[u8]) -> Result<&Self, ParseError> {
        if bytes.len() < Self::FIXED_HEADER_SIZE {
            return Err(ParseError::TooShortForHeader);
        }

        let frame = UslpTransferFrame::ref_from_bytes(bytes)
            .map_err(|_| ParseError::TooShortForHeader)?;

        let tfvn = frame.header().tfvn();
        if tfvn != USLP_TFVN {
            return Err(ParseError::InvalidVersion(tfvn));
        }

        if frame.header().eofph() {
            return Err(ParseError::TruncatedHeader);
        }

        let expected = frame.header().frame_length() as usize + 1;
        if bytes.len() != expected {
            return Err(ParseError::LengthMismatch {
                expected,
                actual: bytes.len(),
            });
        }

        Ok(frame)
    }

    /// Returns a reference to the fixed primary header.
    pub fn header(&self) -> &UslpPrimaryHeaderFixed {
        &self.header
    }

    /// Returns a mutable reference to the fixed primary header.
    pub fn header_mut(&mut self) -> &mut UslpPrimaryHeaderFixed {
        &mut self.header
    }

    /// Returns the complete body after the 7-byte fixed header.
    ///
    /// Contains VCF Count + Insert Zone + TFDF + OCF + FECF.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Returns a mutable reference to the body.
    pub fn body_mut(&mut self) -> &mut [u8] {
        &mut self.body
    }

    /// Total size of the primary header including VCF Count.
    pub fn primary_header_size(&self) -> usize {
        Self::FIXED_HEADER_SIZE
            + self.header().vcf_count_length() as usize
    }

    /// Returns the VCF Count value (big-endian, 0-56 bits).
    pub fn vcf_count(&self) -> u64 {
        let len = self.header().vcf_count_length() as usize;
        let mut val = 0u64;
        for &b in &self.body[..len] {
            val = (val << 8) | b as u64;
        }
        val
    }

    /// Sets the VCF Count value (big-endian).
    pub fn set_vcf_count(&mut self, count: u64) {
        let len = self.header().vcf_count_length() as usize;
        for i in 0..len {
            let shift = (len - 1 - i) * 8;
            self.body[i] = (count >> shift) as u8;
        }
    }

    /// Returns the Insert Zone slice.
    ///
    /// `insert_zone_len` is a managed parameter configured for the
    /// Physical Channel (0 if no Insert Zone).
    pub fn insert_zone(&self, insert_zone_len: usize) -> &[u8] {
        let start = self.header().vcf_count_length() as usize;
        &self.body[start..start + insert_zone_len]
    }

    /// Returns a mutable reference to the Insert Zone.
    pub fn insert_zone_mut(
        &mut self,
        insert_zone_len: usize,
    ) -> &mut [u8] {
        let start = self.header().vcf_count_length() as usize;
        &mut self.body[start..start + insert_zone_len]
    }

    /// Returns the Transfer Frame Data Field (TFDF Header + TFDZ).
    ///
    /// `insert_zone_len` is the managed Insert Zone size.
    /// `fecf_present` indicates whether a 2-byte FECF is appended.
    pub fn data_field(
        &self,
        insert_zone_len: usize,
        fecf_present: bool,
    ) -> &[u8] {
        let start = self.header().vcf_count_length() as usize
            + insert_zone_len;
        let ocf_len = if self.header().ocf_flag() { 4 } else { 0 };
        let fecf_len = if fecf_present { 2 } else { 0 };
        let end = self.body.len() - ocf_len - fecf_len;
        &self.body[start..end]
    }

    /// Returns a mutable reference to the data field.
    pub fn data_field_mut(
        &mut self,
        insert_zone_len: usize,
        fecf_present: bool,
    ) -> &mut [u8] {
        let start = self.header().vcf_count_length() as usize
            + insert_zone_len;
        let ocf_len = if self.header().ocf_flag() { 4 } else { 0 };
        let fecf_len = if fecf_present { 2 } else { 0 };
        let end = self.body.len() - ocf_len - fecf_len;
        &mut self.body[start..end]
    }

    /// Returns the TFDZ Construction Rule from the TFDF Header.
    pub fn tfdz_rule(
        &self,
        insert_zone_len: usize,
    ) -> TfdzRule {
        let off = self.header().vcf_count_length() as usize
            + insert_zone_len;
        TfdzRule::from_bits(get_bits_u8(
            self.body[off],
            TFDZ_RULES_MASK,
        ))
    }

    /// Returns the UPID from the TFDF Header.
    pub fn upid(&self, insert_zone_len: usize) -> u8 {
        let off = self.header().vcf_count_length() as usize
            + insert_zone_len;
        get_bits_u8(self.body[off], UPID_MASK)
    }

    /// Returns the First Header / Last Valid Octet Pointer.
    ///
    /// Only present for TFDZ Construction Rules 000, 001, and 010.
    /// Returns `None` if the current rule does not use a pointer.
    pub fn pointer(
        &self,
        insert_zone_len: usize,
    ) -> Option<u16> {
        let rule = self.tfdz_rule(insert_zone_len);
        if !rule.has_pointer() {
            return None;
        }
        let off = self.header().vcf_count_length() as usize
            + insert_zone_len
            + 1;
        let hi = self.body[off] as u16;
        let lo = self.body[off + 1] as u16;
        Some((hi << 8) | lo)
    }

    /// Returns the Transfer Frame Data Zone (payload only).
    ///
    /// This is the TFDF minus the TFDF Header bytes.
    pub fn data_zone(
        &self,
        insert_zone_len: usize,
        fecf_present: bool,
    ) -> &[u8] {
        let rule = self.tfdz_rule(insert_zone_len);
        let tfdf_hdr_len = if rule.has_pointer() { 3 } else { 1 };
        let start = self.header().vcf_count_length() as usize
            + insert_zone_len
            + tfdf_hdr_len;
        let ocf_len = if self.header().ocf_flag() { 4 } else { 0 };
        let fecf_len = if fecf_present { 2 } else { 0 };
        let end = self.body.len() - ocf_len - fecf_len;
        &self.body[start..end]
    }

    /// Returns the OCF (4 bytes) if present.
    pub fn ocf(&self, fecf_present: bool) -> Option<&[u8]> {
        if !self.header().ocf_flag() {
            return None;
        }
        let fecf_len = if fecf_present { 2 } else { 0 };
        let end = self.body.len() - fecf_len;
        Some(&self.body[end - 4..end])
    }

    /// Returns the FECF (2 bytes) if present.
    ///
    /// `fecf_present` is a managed parameter for the Physical
    /// Channel.
    pub fn fecf(&self, fecf_present: bool) -> Option<&[u8]> {
        if !fecf_present {
            return None;
        }
        let len = self.body.len();
        Some(&self.body[len - 2..])
    }

    /// Writes the TFDF Header at the start of the data field.
    ///
    /// Sets the Construction Rule, UPID, and optional 16-bit
    /// pointer in the TFDF Header bytes.
    pub fn set_tfdf_header(
        &mut self,
        insert_zone_len: usize,
        rule: TfdzRule,
        upid: u8,
        pointer: Option<u16>,
    ) {
        let off = self.header().vcf_count_length() as usize
            + insert_zone_len;
        let mut byte0 = 0u8;
        set_bits_u8(&mut byte0, TFDZ_RULES_MASK, rule as u8);
        set_bits_u8(&mut byte0, UPID_MASK, upid);
        self.body[off] = byte0;
        if let Some(ptr) = pointer {
            self.body[off + 1] = (ptr >> 8) as u8;
            self.body[off + 2] = ptr as u8;
        }
    }

    /// Constructs a new USLP Transfer Frame in the provided buffer.
    ///
    /// The frame length is determined by the buffer size. The body
    /// after the primary header is zeroed; use [`Self::body_mut`],
    /// [`Self::set_tfdf_header`], and [`Self::data_field_mut`] to
    /// fill in the Insert Zone, TFDF, OCF, and FECF.
    #[builder]
    pub fn new(
        buffer: &mut [u8],
        scid: u16,
        vcid: u8,
        #[builder(default)] source_or_dest: bool,
        #[builder(default)] map_id: u8,
        #[builder(default)] bypass: bool,
        #[builder(default)] protocol_control_command: bool,
        #[builder(default)] ocf_flag: bool,
        #[builder(default)] vcf_count_length: u8,
        #[builder(default)] vcf_count: u64,
    ) -> Result<&mut Self, BuildError> {
        if vcid > 63 {
            return Err(BuildError::InvalidVcid(vcid));
        }
        if map_id > 15 {
            return Err(BuildError::InvalidMapId(map_id));
        }
        if vcf_count_length > 7 {
            return Err(BuildError::InvalidVcfCountLength(
                vcf_count_length,
            ));
        }

        let provided_len = buffer.len();
        let min_size =
            Self::FIXED_HEADER_SIZE + vcf_count_length as usize;

        if provided_len < min_size {
            return Err(BuildError::BufferTooSmall {
                required_len: min_size,
                provided_len,
            });
        }
        if provided_len > Self::MAX_FRAME_SIZE {
            return Err(BuildError::FrameTooLarge {
                max_len: Self::MAX_FRAME_SIZE,
                provided_len,
            });
        }

        let frame =
            UslpTransferFrame::mut_from_bytes(buffer).map_err(
                |_| BuildError::BufferTooSmall {
                    required_len: min_size,
                    provided_len,
                },
            )?;

        // Zero the header for a clean state.
        frame.header.id = U32::new(0);
        frame.header.frame_length = U16::new(0);
        frame.header.flags = 0;

        frame.header.set_tfvn(USLP_TFVN);
        frame.header.set_scid(scid);
        frame.header.set_source_or_dest(source_or_dest);
        frame.header.set_vcid(vcid);
        frame.header.set_map_id(map_id);
        frame.header.set_eofph(false);
        frame
            .header
            .set_frame_length((provided_len - 1) as u16);
        frame.header.set_bypass(bypass);
        frame
            .header
            .set_protocol_control_command(protocol_control_command);
        frame.header.set_ocf_flag(ocf_flag);
        frame.header.set_vcf_count_length(vcf_count_length);

        // Write VCF Count (big-endian).
        let vcf_len = vcf_count_length as usize;
        for i in 0..vcf_len {
            let shift = (vcf_len - 1 - i) * 8;
            frame.body[i] = (vcf_count >> shift) as u8;
        }

        Ok(frame)
    }
}

// --- UslpPrimaryHeaderFixed accessors ---

impl UslpPrimaryHeaderFixed {
    // === ID field (U32, bytes 0-3) ===

    /// Returns the 4-bit Transfer Frame Version Number.
    pub fn tfvn(&self) -> u8 {
        get_bits_u32(self.id, TFVN_MASK) as u8
    }

    /// Sets the Transfer Frame Version Number.
    pub fn set_tfvn(&mut self, v: u8) {
        set_bits_u32(&mut self.id, TFVN_MASK, v as u32);
    }

    /// Returns the 16-bit Spacecraft Identifier.
    pub fn scid(&self) -> u16 {
        get_bits_u32(self.id, SCID_MASK) as u16
    }

    /// Sets the Spacecraft Identifier.
    pub fn set_scid(&mut self, v: u16) {
        set_bits_u32(&mut self.id, SCID_MASK, v as u32);
    }

    /// Returns the Source-or-Destination Identifier.
    ///
    /// `false` = SCID is the source; `true` = SCID is the dest.
    pub fn source_or_dest(&self) -> bool {
        get_bits_u32(self.id, SRC_DEST_MASK) != 0
    }

    /// Sets the Source-or-Destination Identifier.
    pub fn set_source_or_dest(&mut self, v: bool) {
        set_bits_u32(&mut self.id, SRC_DEST_MASK, v as u32);
    }

    /// Returns the 6-bit Virtual Channel Identifier.
    pub fn vcid(&self) -> u8 {
        get_bits_u32(self.id, VCID_MASK) as u8
    }

    /// Sets the Virtual Channel Identifier.
    pub fn set_vcid(&mut self, v: u8) {
        set_bits_u32(&mut self.id, VCID_MASK, v as u32);
    }

    /// Returns the 4-bit Multiplexer Access Point Identifier.
    pub fn map_id(&self) -> u8 {
        get_bits_u32(self.id, MAP_ID_MASK) as u8
    }

    /// Sets the MAP Identifier.
    pub fn set_map_id(&mut self, v: u8) {
        set_bits_u32(&mut self.id, MAP_ID_MASK, v as u32);
    }

    /// Returns the End of Frame Primary Header Flag.
    ///
    /// `false` = non-truncated (13 fields);
    /// `true` = truncated (6 fields only).
    pub fn eofph(&self) -> bool {
        get_bits_u32(self.id, EOFPH_MASK) != 0
    }

    /// Sets the End of Frame Primary Header Flag.
    pub fn set_eofph(&mut self, v: bool) {
        set_bits_u32(&mut self.id, EOFPH_MASK, v as u32);
    }

    // === Frame Length (U16, bytes 4-5) ===

    /// Returns the Frame Length field value.
    ///
    /// Total frame size in bytes = `frame_length() + 1`.
    pub fn frame_length(&self) -> u16 {
        self.frame_length.get()
    }

    /// Sets the Frame Length field.
    pub fn set_frame_length(&mut self, v: u16) {
        self.frame_length.set(v);
    }

    // === Flags byte (byte 6) ===

    /// Returns the Bypass/Sequence Control Flag.
    ///
    /// `false` = Sequence-Controlled (Type-A);
    /// `true` = Expedited / bypass (Type-B).
    pub fn bypass(&self) -> bool {
        get_bits_u8(self.flags, BYPASS_FLAG_MASK) != 0
    }

    /// Sets the Bypass/Sequence Control Flag.
    pub fn set_bypass(&mut self, v: bool) {
        set_bits_u8(&mut self.flags, BYPASS_FLAG_MASK, v as u8);
    }

    /// Returns the Protocol Control Command Flag.
    ///
    /// `false` = user data; `true` = protocol control commands.
    pub fn protocol_control_command(&self) -> bool {
        get_bits_u8(self.flags, PCC_FLAG_MASK) != 0
    }

    /// Sets the Protocol Control Command Flag.
    pub fn set_protocol_control_command(&mut self, v: bool) {
        set_bits_u8(&mut self.flags, PCC_FLAG_MASK, v as u8);
    }

    /// Returns the OCF Flag.
    ///
    /// `true` = OCF (4 bytes) is present in this frame.
    pub fn ocf_flag(&self) -> bool {
        get_bits_u8(self.flags, OCF_FLAG_MASK) != 0
    }

    /// Sets the OCF Flag.
    pub fn set_ocf_flag(&mut self, v: bool) {
        set_bits_u8(&mut self.flags, OCF_FLAG_MASK, v as u8);
    }

    /// Returns the VCF Count Length (0-7).
    ///
    /// Determines how many bytes are used for the VCF Count
    /// field following this fixed header.
    pub fn vcf_count_length(&self) -> u8 {
        get_bits_u8(self.flags, VCF_COUNT_LEN_MASK)
    }

    /// Sets the VCF Count Length.
    pub fn set_vcf_count_length(&mut self, v: u8) {
        set_bits_u8(&mut self.flags, VCF_COUNT_LEN_MASK, v);
    }
}

impl core::fmt::Debug for UslpTransferFrame {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UslpTransferFrame")
            .field("header", &self.header)
            .field("body_len", &self.body.len())
            .finish()
    }
}

// ── FrameWrite / FrameRead implementations ──

use super::{FrameRead, FrameWrite, PushError};

/// Configuration for building USLP transfer frames.
#[derive(Debug, Clone)]
pub struct UslpFrameWriterConfig {
    /// Spacecraft ID (16-bit).
    pub scid: u16,
    /// Virtual Channel ID (6-bit).
    pub vcid: u8,
    /// Multiplexer Access Point ID (4-bit).
    pub map_id: u8,
    /// Bypass/Sequence Control Flag.
    pub bypass: bool,
    /// Protocol Control Command Flag.
    pub protocol_control_command: bool,
    /// Operational Control Field Flag.
    pub ocf_flag: bool,
    /// VCF Count field length in bytes (0-7).
    pub vcf_count_length: u8,
    /// Insert Zone length in bytes.
    pub insert_zone_len: usize,
    /// TFDZ Construction Rule.
    pub tfdz_rule: TfdzRule,
    /// USLP Protocol Identifier.
    pub upid: u8,
    /// Whether a 2-byte FECF is appended.
    pub fecf_present: bool,
    /// Maximum data zone length in bytes (payload capacity).
    pub max_data_zone_len: usize,
}

/// Accumulates packets into USLP transfer frames.
///
/// Owns its frame buffer internally (sized by `BUF`). Packets
/// are pushed directly into the buffer at the correct offset.
/// [`finish()`](FrameWrite::finish) stamps the header and
/// returns a borrow of the completed frame.
pub struct UslpFrameWriter<const BUF: usize> {
    config: UslpFrameWriterConfig,
    vcf_count: u64,
    data_zone_offset: usize,
    data_len: usize,
    buf: [u8; BUF],
}

impl<const BUF: usize> UslpFrameWriter<BUF> {
    /// Creates a new USLP frame writer.
    pub fn new(config: UslpFrameWriterConfig) -> Self {
        let tfdf_header_len =
            if config.tfdz_rule.has_pointer() { 3 } else { 1 };
        let data_zone_offset =
            UslpTransferFrame::FIXED_HEADER_SIZE
                + config.vcf_count_length as usize
                + config.insert_zone_len
                + tfdf_header_len;
        Self {
            config,
            vcf_count: 0,
            data_zone_offset,
            data_len: 0,
            buf: [0u8; BUF],
        }
    }

    fn remaining(&self) -> usize {
        self.config
            .max_data_zone_len
            .saturating_sub(self.data_len)
    }
}

impl<const BUF: usize> FrameWrite for UslpFrameWriter<BUF> {
    type Error = BuildError;

    fn is_empty(&self) -> bool {
        self.data_len == 0
    }

    fn push(&mut self, data: &[u8]) -> Result<(), PushError> {
        if data.len() > self.config.max_data_zone_len {
            return Err(PushError::TooLarge);
        }
        if data.len() > self.remaining() {
            return Err(PushError::Full);
        }
        let off = self.data_zone_offset + self.data_len;
        self.buf[off..off + data.len()].copy_from_slice(data);
        self.data_len += data.len();
        Ok(())
    }

    fn finish(&mut self) -> Result<&[u8], BuildError> {
        let ocf_len =
            if self.config.ocf_flag { 4 } else { 0 };
        let fecf_len =
            if self.config.fecf_present { 2 } else { 0 };
        let total = self.data_zone_offset
            + self.data_len
            + ocf_len
            + fecf_len;

        let vcf_count = self.vcf_count;
        self.vcf_count = self.vcf_count.wrapping_add(1);

        // Stamp the primary header and VCF count. The builder
        // zeroes the header bytes but does not touch data beyond
        // the VCF count field, so our payload is safe.
        UslpTransferFrame::builder()
            .buffer(&mut self.buf[..total])
            .scid(self.config.scid)
            .vcid(self.config.vcid)
            .map_id(self.config.map_id)
            .bypass(self.config.bypass)
            .protocol_control_command(
                self.config.protocol_control_command,
            )
            .ocf_flag(self.config.ocf_flag)
            .vcf_count_length(self.config.vcf_count_length)
            .vcf_count(vcf_count)
            .build()?;

        // Stamp the TFDF header (rule + UPID + optional pointer).
        let frame = UslpTransferFrame::mut_from_bytes(
            &mut self.buf[..total],
        )
        .unwrap();
        frame.set_tfdf_header(
            self.config.insert_zone_len,
            self.config.tfdz_rule,
            self.config.upid,
            if self.config.tfdz_rule.has_pointer() {
                Some(0)
            } else {
                None
            },
        );

        self.data_len = 0;
        Ok(&self.buf[..total])
    }
}

/// Extracts packets from a received USLP transfer frame.
///
/// Owns its frame buffer internally (sized by `BUF`). The
/// coding layer writes into
/// [`buffer_mut()`](FrameRead::buffer_mut),
/// [`feed()`](FrameRead::feed) validates the header, and
/// [`next()`](FrameRead::next) returns zero-copy sub-slices.
pub struct UslpFrameReader<const BUF: usize> {
    insert_zone_len: usize,
    fecf_present: bool,
    buf: [u8; BUF],
    data_start: usize,
    data_end: usize,
}

impl<const BUF: usize> UslpFrameReader<BUF> {
    /// Creates a new USLP frame reader.
    pub fn new(
        insert_zone_len: usize,
        fecf_present: bool,
    ) -> Self {
        Self {
            insert_zone_len,
            fecf_present,
            buf: [0u8; BUF],
            data_start: 0,
            data_end: 0,
        }
    }
}

impl<const BUF: usize> FrameRead for UslpFrameReader<BUF> {
    type Error = ParseError;

    fn buffer_mut(&mut self) -> &mut [u8] {
        &mut self.buf
    }

    fn feed(&mut self, len: usize) -> Result<(), ParseError> {
        let frame =
            UslpTransferFrame::parse(&self.buf[..len])?;
        let rule = frame.tfdz_rule(self.insert_zone_len);
        let tfdf_hdr_len =
            if rule.has_pointer() { 3 } else { 1 };
        let header_overhead =
            UslpTransferFrame::FIXED_HEADER_SIZE
                + frame.header().vcf_count_length() as usize
                + self.insert_zone_len
                + tfdf_hdr_len;
        let ocf_len =
            if frame.header().ocf_flag() { 4 } else { 0 };
        let fecf_len =
            if self.fecf_present { 2 } else { 0 };
        self.data_start = header_overhead;
        self.data_end = len - ocf_len - fecf_len;
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
    fn build_and_parse_basic_frame() {
        let mut buf = [0u8; 32];
        let frame = UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(42)
            .vcid(1)
            .build()
            .unwrap();

        assert_eq!(frame.header().tfvn(), USLP_TFVN);
        assert_eq!(frame.header().scid(), 42);
        assert_eq!(frame.header().vcid(), 1);
        assert_eq!(frame.header().map_id(), 0);
        assert!(!frame.header().source_or_dest());
        assert!(!frame.header().eofph());
        assert!(!frame.header().bypass());
        assert!(!frame.header().protocol_control_command());
        assert!(!frame.header().ocf_flag());
        assert_eq!(frame.header().vcf_count_length(), 0);
        assert_eq!(frame.header().frame_length(), 31);
    }

    #[test]
    fn parse_validates_tfvn() {
        let mut buf = [0u8; 16];
        // Build a valid frame then corrupt the TFVN.
        UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .vcid(0)
            .build()
            .unwrap();
        buf[0] = 0x00; // TFVN = 0 instead of 0xC
        let err = UslpTransferFrame::parse(&buf).unwrap_err();
        assert_eq!(err, ParseError::InvalidVersion(0));
    }

    #[test]
    fn parse_validates_length() {
        let mut buf = [0u8; 16];
        UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .vcid(0)
            .build()
            .unwrap();
        // Parse with a truncated slice.
        let err =
            UslpTransferFrame::parse(&buf[..12]).unwrap_err();
        assert!(matches!(err, ParseError::LengthMismatch { .. }));
    }

    #[test]
    fn roundtrip_with_vcf_count() {
        let mut buf = [0u8; 64];
        let frame = UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(1000)
            .vcid(5)
            .map_id(3)
            .bypass(true)
            .vcf_count_length(2)
            .vcf_count(1234)
            .build()
            .unwrap();

        assert_eq!(frame.header().scid(), 1000);
        assert_eq!(frame.header().vcid(), 5);
        assert_eq!(frame.header().map_id(), 3);
        assert!(frame.header().bypass());
        assert_eq!(frame.header().vcf_count_length(), 2);
        assert_eq!(frame.vcf_count(), 1234);
        assert_eq!(frame.primary_header_size(), 9);
    }

    #[test]
    fn roundtrip_parse() {
        let mut buf = [0u8; 32];
        UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(500)
            .vcid(7)
            .source_or_dest(true)
            .ocf_flag(true)
            .build()
            .unwrap();

        let frame = UslpTransferFrame::parse(&buf).unwrap();
        assert_eq!(frame.header().scid(), 500);
        assert_eq!(frame.header().vcid(), 7);
        assert!(frame.header().source_or_dest());
        assert!(frame.header().ocf_flag());
    }

    #[test]
    fn tfdf_header_roundtrip() {
        let mut buf = [0u8; 32];
        let frame = UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .vcid(0)
            .build()
            .unwrap();

        frame.set_tfdf_header(
            0,
            TfdzRule::NoSegmentation,
            Upid::SpacePackets as u8,
            None,
        );

        assert_eq!(frame.tfdz_rule(0), TfdzRule::NoSegmentation);
        assert_eq!(frame.upid(0), 0);
        assert_eq!(frame.pointer(0), None);
    }

    #[test]
    fn tfdf_header_with_pointer() {
        let mut buf = [0u8; 32];
        let frame = UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .vcid(0)
            .build()
            .unwrap();

        frame.set_tfdf_header(
            0,
            TfdzRule::PacketsSpanning,
            Upid::SpacePackets as u8,
            Some(0x0100),
        );

        assert_eq!(
            frame.tfdz_rule(0),
            TfdzRule::PacketsSpanning
        );
        assert_eq!(frame.pointer(0), Some(0x0100));
    }

    #[test]
    fn invalid_vcid_rejected() {
        let mut buf = [0u8; 32];
        let err = UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0)
            .vcid(64)
            .build()
            .unwrap_err();
        assert_eq!(err, BuildError::InvalidVcid(64));
    }

    #[test]
    fn tfdz_rule_from_bits() {
        for i in 0..=7u8 {
            let rule = TfdzRule::from_bits(i);
            assert_eq!(rule as u8, i);
        }
    }

    #[test]
    fn tfdz_rule_has_pointer() {
        assert!(TfdzRule::PacketsSpanning.has_pointer());
        assert!(TfdzRule::SduStart.has_pointer());
        assert!(TfdzRule::SduContinue.has_pointer());
        assert!(!TfdzRule::OctetStream.has_pointer());
        assert!(!TfdzRule::StartingSegment.has_pointer());
        assert!(!TfdzRule::ContinuingSegment.has_pointer());
        assert!(!TfdzRule::LastSegment.has_pointer());
        assert!(!TfdzRule::NoSegmentation.has_pointer());
    }

    #[test]
    fn max_scid() {
        let mut buf = [0u8; 16];
        let frame = UslpTransferFrame::builder()
            .buffer(&mut buf)
            .scid(0xFFFF)
            .vcid(0)
            .build()
            .unwrap();
        assert_eq!(frame.header().scid(), 0xFFFF);
    }
}
