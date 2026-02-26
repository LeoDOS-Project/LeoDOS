//! CCSDS SPP (Space Packet Protocol) definitions and utilities.
//! * Specification: https://ccsds.org/Pubs/133x0b2e2.pdf

use bon::bon;
use core::fmt::Debug;
use core::fmt::Display;
use core::fmt::LowerHex;
use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;
use zerocopy::ByteEq;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian;

pub mod encapsulation;
pub mod handler;
pub mod segmentation;

pub use zerocopy;

/// A zero-copy view over a CCSDS Space Packet in a raw byte buffer.
///
/// This struct provides a low-level, safe view over a byte slice that is
/// known to contain a valid Space Packet. It can be created via `SpacePacket::parse()`
/// or through the ergonomic `SpacePacket::builder()`.
///
/// The `data_field` is an unsized `[u8]` slice, allowing this struct to represent
/// packets of any valid length without needing different types.
///
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// + -- Primary Header (6 bytes) ------ | ------- |
/// |                                    |         |
/// | Packet Version Number              | 3 bits  |
/// | Packet Identification Field        | 13 bits |
/// |   - Packet Type                    | 1 bit   |
/// |   - Secondary Header Flag          | 1 bit   |
/// |   - APID (Application Process ID)  | 11 bits |
/// | Packet Sequence Control            | 16 bits |
/// |   - Sequence Flags                 | 2 bits  |
/// |   - Sequence Count                 | 14 bits |
/// | Packet Data Length                 | 16 bits |
/// |                                    |         |
/// | -- Packet Data Field (Variable) -- | ------- |
/// |                                    |         |
/// | Secondary Header (if present)      |         |
/// | User Data Field                    | 1-65536 |
/// |                                    | bytes   |
/// +------------------------------------+---------+
/// ```
#[repr(C)]
#[derive(ByteEq, FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned)]
pub struct SpacePacket {
    pub primary_header: PrimaryHeader,
    pub data_field: [u8],
}

/// A trait alias for the required bounds on a zero-copy packet data payload.
///
/// This is a convenience trait that bundles the necessary traits from the `zerocopy`
/// crate. Any struct that will be used as a typed data field should be able to
/// be soundly cast to and from a byte slice.
pub trait SpacePacketData: FromBytes + IntoBytes + KnownLayout + Unaligned + Immutable {}
impl<T> SpacePacketData for T where T: FromBytes + IntoBytes + KnownLayout + Unaligned + Immutable {}

impl Debug for SpacePacket {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpacePacket")
            .field("header", &self.primary_header)
            .field("data_field", &&self.data_field)
            .finish()
    }
}

/// The 6-byte primary header of a CCSDS Space Packet.
///
/// This struct is a zero-copy view over the first 6 bytes of a packet and provides
/// methods to safely access the bit-packed fields.
#[repr(C)]
#[derive(
    Copy, Clone, Debug, Hash, ByteEq, FromBytes, IntoBytes, KnownLayout, Immutable, Unaligned,
)]
pub struct PrimaryHeader {
    /// Contains the 3-bit Version and 13-bit Packet Identification field (Type, SecHdr, APID)
    packet_version_and_id: network_endian::U16,
    /// Contains the 2-bit Sequence Flags and 14-bit Sequence Count
    packet_sequence_control: network_endian::U16,
    /// Contains the length of the Packet Data Field minus 1
    packet_data_length: network_endian::U16,
}

#[rustfmt::skip]
mod bitmasks {
    // Version and Identification field masks
    pub const PACKET_VERSION_MASK: u16 = 0b_11100000_00000000;
    pub const PACKET_TYPE_MASK: u16 =    0b_00010000_00000000;
    pub const SEC_HDR_MASK: u16 =        0b_00001000_00000000;
    pub const APID_MASK: u16 =           0b_00000111_11111111;

    // Sequence_control field masks
    pub const SEQ_FLAG_MASK: u16 =       0b_11000000_00000000;
    pub const SEQ_COUNT_MASK: u16 =      0b_00111111_11111111;
}
use bitmasks::*;

use crate::utils::get_bits_u16;
use crate::utils::set_bits_u16;

/// The 3-bit packet version number.
///
/// As per the CCSDS standard, this library currently only supports Version 1 (binary `000`).
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct PacketVersion(u8);

impl PacketVersion {
    /// The version number for CCSDS Space Packets defined in 133.0-B-2 (value is 0).
    pub const VERSION_1: Self = Self(0);
    /// Checks if this library supports the packet version.
    pub fn is_supported(&self) -> bool {
        *self == Self::VERSION_1
    }
}

/// The 1-bit packet type identifier.
///
/// Distinguishes between telemetry (data sent from space to ground) and
/// telecommand (commands sent from ground to space) packets.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
#[repr(u8)]
pub enum PacketType {
    /// Identifies a telemetry packet (value `0`).
    Telemetry = 0,
    /// Identifies a telecommand packet (value `1`).
    Telecommand = 1,
}

/// The 1-bit secondary header flag.
///
/// Indicates whether an optional, mission-defined Secondary Header is present
/// at the beginning of the packet's data field.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
#[repr(u8)]
pub enum SecondaryHeaderFlag {
    /// Indicates that no secondary header is present.
    #[default]
    Absent = 0,
    /// Indicates that a secondary header is present.
    Present = 1,
}

/// The 11-bit Application Process Identifier (APID).
///
/// The APID is used to route the packet to a specific application process or
/// component within the satellite's flight software. It acts as a "port number"
/// or "topic" for the packet's data.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
#[repr(transparent)]
pub struct Apid(u16);

impl LowerHex for Apid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:#05x}", self.0)
    }
}

impl Apid {
    /// The maximum valid APID value.
    pub const MAX: u16 = 0b_00000111_11111111;
    /// The reserved APID value for idle packets.
    ///
    /// Idle packets are sent to maintain link synchronization when no real data is available.
    pub const IDLE: Self = Self(Self::MAX);

    /// Creates a new `Apid`, returning an error if the value is out of range.
    pub const fn new(id: u16) -> Result<Self, BuildError> {
        if id > Self::MAX {
            Err(BuildError::InvalidApid { value: id })
        } else {
            Ok(Self(id))
        }
    }

    /// Checks if this is the reserved idle APID.
    pub fn is_idle(&self) -> bool {
        *self == Self::IDLE
    }

    pub fn value(&self) -> u16 {
        self.0
    }

    #[cfg(kani)]
    fn any() -> Self {
        let any_u16: u16 = kani::any();
        kani::assume(any_u16 <= Self::MAX);
        Self(any_u16)
    }
}

/// The 2-bit sequence flag.
///
/// Indicates if a large block of user data has been split (segmented) across
/// multiple Space Packets.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
#[repr(u8)]
pub enum SequenceFlag {
    /// The packet's data field is a continuation of a segmented message.
    Continuation = 0b00,
    /// The packet's data field is the first segment of a message.
    First = 0b01,
    /// The packet's data field is the last segment of a message.
    Last = 0b10,
    /// The packet contains a complete, unsegmented block of data.
    #[default]
    Unsegmented = 0b11,
}

/// The 14-bit packet sequence count.
///
/// A rolling counter for packets with a specific APID. This allows the receiver
/// to detect dropped or out-of-order packets. The count wraps around from 16383 to 0.
#[derive(Copy, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Default)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct SequenceCount(u16);

impl SequenceCount {
    /// The maximum valid sequence count value (16383).
    pub const MAX: u16 = 0b_00111111_11111111;

    /// Creates a new `SequenceCount` initialized to zero.
    pub fn new() -> Self {
        Self(0)
    }

    /// Increments the sequence count, wrapping around on overflow.
    pub fn increment(&mut self) {
        self.0 = (self.0 + 1) & Self::MAX;
    }

    pub fn value(&self) -> u16 {
        self.0
    }

    #[cfg(kani)]
    fn any() -> Self {
        let any_u16: u16 = kani::any();
        kani::assume(any_u16 <= Self::MAX);
        Self(any_u16)
    }
}

/// An error that can occur when parsing a byte slice into a `SpacePacket`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ParseError {
    /// The provided slice is shorter than the 6-byte primary header.
    TooShortForHeader { actual: usize },
    /// The header's length field implies a packet larger than the provided buffer.
    IncompletePacket {
        header_len: usize,
        buffer_len: usize,
    },
    /// The packet's header fields contain semantically invalid values.
    Invalid(ValidationError),
}

/// An error representing a violation of the CCSDS semantic rules.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum ValidationError {
    /// The packet version number is not supported by this library.
    UnsupportedVersion(PacketVersion),
    /// An idle packet (APID 2047) must not have a secondary header.
    IdlePacketWithSecondaryHeader,
}

/// An error that occurs during the construction of a `SpacePacket` header.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum BuildError {
    /// This variant is not used by the builder but is kept for the `SpacePacket::new` constructor.
    BufferTooSmall { required: usize, provided: usize },
    /// The provided data field length exceeds the maximum allowed size.
    PayloadTooLarge { max: usize, provided: usize },
    /// The CCSDS standard forbids packets with an empty data field.
    EmptyDataField,
    /// The provided APID value is outside the valid 11-bit range (0-2047).
    InvalidApid { value: u16 },
}

/// An error that occurs when setting or getting the typed data field.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum DataFieldError {
    /// The size of the provided data does not match the size of the packet's data field.
    SizeMismatch,
    /// The packet's data field could not be safely cast to the target data type,
    /// often due to alignment issues or an invalid discriminant.
    InvalidLayout,
    /// A secondary header was requested but the secondary header flag is absent.
    SecondaryHeaderAbsent,
}

impl Deref for SpacePacket {
    /// Dereferences to the `PrimaryHeader` to allow direct access to header fields.
    type Target = PrimaryHeader;
    fn deref(&self) -> &Self::Target {
        &self.primary_header
    }
}

impl DerefMut for SpacePacket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.primary_header
    }
}

impl crate::utils::Header<PrimaryHeader> for PrimaryHeader {
    fn get(&self) -> &PrimaryHeader {
        self
    }
    fn get_mut(&mut self) -> &mut PrimaryHeader {
        self
    }
}

#[bon]
impl SpacePacket {
    /// Constructs a new `SpacePacket` in the provided buffer.
    #[builder]
    pub fn new<'a, 'b>(
        buffer: &'a mut [u8],
        apid: Apid,
        packet_type: PacketType,
        sequence_count: SequenceCount,
        secondary_header: SecondaryHeaderFlag,
        sequence_flag: SequenceFlag,
        data_len: usize,
    ) -> Result<&'a mut Self, BuildError> {
        if data_len > u16::MAX as usize {
            return Err(BuildError::InvalidApid { value: apid.0 });
        }
        if data_len == 0 {
            return Err(BuildError::EmptyDataField);
        }
        let required_len = data_len;
        let provided_len = buffer.len();
        let (packet, _) = Self::mut_from_prefix_with_elems(buffer, required_len).map_err(|_| {
            BuildError::BufferTooSmall {
                required: required_len,
                provided: provided_len,
            }
        })?;

        packet.set_version(PacketVersion::VERSION_1);
        packet.set_packet_type(packet_type);
        packet.set_apid(apid);
        packet.set_sequence_count(sequence_count);
        packet.set_data_field_len(data_len as u16);
        packet.set_secondary_header_flag(secondary_header);
        packet.set_sequence_flag(sequence_flag);

        Ok(packet)
    }

    /// Parses (zero-copy) a raw byte slice into a `SpacePacket`.
    ///
    /// This function reads the packet header to determine the packet's total length
    /// and returns a view over that exact portion of the provided slice.
    pub fn parse(bytes: &[u8]) -> Result<&Self, ParseError> {
        if bytes.len() < size_of::<PrimaryHeader>() {
            return Err(ParseError::TooShortForHeader {
                actual: bytes.len(),
            });
        }
        let packet = Self::ref_from_bytes(bytes)
            .expect("Should not fail due to prior length check and Unaligned trait");

        packet.primary_header.validate()?;
        let specified_len = packet.primary_header.packet_len();
        if specified_len > bytes.len() {
            return Err(ParseError::IncompletePacket {
                header_len: specified_len,
                buffer_len: bytes.len(),
            });
        }

        let packet = Self::ref_from_bytes(&bytes[..specified_len])
            .expect("Should not fail due to prior length checks");
        Ok(packet)
    }

    /// Copies a user-defined data structure into the packet's data field.
    ///
    /// The size of `T` must exactly match the length of the data field.
    pub fn set_data_field<T: SpacePacketData>(&mut self, data: &T) -> Result<(), DataFieldError> {
        if self.data_field.len() != size_of::<T>() {
            return Err(DataFieldError::SizeMismatch);
        }
        self.data_field_mut().copy_from_slice(data.as_bytes());
        Ok(())
    }

    /// Returns a zero-copy, typed view of the packet's data field.
    ///
    /// This is the primary method for interpreting the packet's payload as a
    /// specific data structure. It will fail if the size of `T` does not match
    /// the data field's length.
    pub fn data_as<T: SpacePacketData>(&self) -> Result<&T, DataFieldError> {
        if self.data_field.len() != size_of::<T>() {
            return Err(DataFieldError::SizeMismatch);
        }
        T::ref_from_bytes(self.data_field()).map_err(|_| DataFieldError::InvalidLayout)
    }

    /// Returns an immutable slice of the packet's data field.
    pub fn data_field(&self) -> &[u8] {
        &self.data_field
    }

    /// Returns a mutable slice of the packet's data field.
    ///
    /// **Warning:** Modifying the data field directly will invalidate any
    /// CRC checksum. If using a `CrcSpacePacket`, prefer the safe `set_data()`
    /// method instead.
    pub fn data_field_mut(&mut self) -> &mut [u8] {
        &mut self.data_field
    }
}

impl PrimaryHeader {
    fn validate(&self) -> Result<(), ValidationError> {
        let version = self.version();
        if !version.is_supported() {
            return Err(ValidationError::UnsupportedVersion(version));
        }
        if self.apid().is_idle() && self.secondary_header_flag() == SecondaryHeaderFlag::Present {
            return Err(ValidationError::IdlePacketWithSecondaryHeader);
        }
        Ok(())
    }

    /// Returns the 3-bit packet version number.
    pub fn version(&self) -> PacketVersion {
        PacketVersion(get_bits_u16(self.packet_version_and_id, PACKET_VERSION_MASK) as u8)
    }

    /// Sets the 3-bit packet version number.
    pub fn set_version(&mut self, version: PacketVersion) {
        set_bits_u16(
            &mut self.packet_version_and_id,
            PACKET_VERSION_MASK,
            version.0 as u16,
        );
    }

    /// Returns the `PacketType` (Telemetry or Telecommand).
    pub fn packet_type(&self) -> PacketType {
        if get_bits_u16(self.packet_version_and_id, PACKET_TYPE_MASK) == 0 {
            PacketType::Telemetry
        } else {
            PacketType::Telecommand
        }
    }

    /// Sets the `PacketType` (Telemetry or Telecommand).
    pub fn set_packet_type(&mut self, packet_type: PacketType) {
        set_bits_u16(
            &mut self.packet_version_and_id,
            PACKET_TYPE_MASK,
            packet_type as u16,
        );
    }

    /// Returns the `SecondaryHeader` flag (Present or Absent).
    pub fn secondary_header_flag(&self) -> SecondaryHeaderFlag {
        if get_bits_u16(self.packet_version_and_id, SEC_HDR_MASK) == 0 {
            SecondaryHeaderFlag::Absent
        } else {
            SecondaryHeaderFlag::Present
        }
    }

    /// Sets the `SecondaryHeader` flag (Present or Absent).
    pub fn set_secondary_header_flag(&mut self, flag: SecondaryHeaderFlag) {
        set_bits_u16(&mut self.packet_version_and_id, SEC_HDR_MASK, flag as u16);
    }

    /// Returns the 11-bit Application Process Identifier (`Apid`).
    pub fn apid(&self) -> Apid {
        Apid(self.packet_version_and_id.get() & APID_MASK)
    }

    /// Sets the 11-bit Application Process Identifier (`Apid`).
    pub fn set_apid(&mut self, apid: Apid) {
        let ident = self.packet_version_and_id.get();
        self.packet_version_and_id
            .set((ident & !APID_MASK) | apid.0);
    }

    /// Returns the 2-bit `SequenceFlag`.
    pub fn sequence_flag(&self) -> SequenceFlag {
        match get_bits_u16(self.packet_sequence_control, SEQ_FLAG_MASK) {
            0b00 => SequenceFlag::Continuation,
            0b01 => SequenceFlag::First,
            0b10 => SequenceFlag::Last,
            _ => SequenceFlag::Unsegmented,
        }
    }

    /// Sets the 2-bit `SequenceFlag`.
    pub fn set_sequence_flag(&mut self, flag: SequenceFlag) {
        set_bits_u16(
            &mut self.packet_sequence_control,
            SEQ_FLAG_MASK,
            flag as u16,
        );
    }

    /// Returns the 14-bit packet sequence count.
    pub fn sequence_count(&self) -> SequenceCount {
        SequenceCount(get_bits_u16(self.packet_sequence_control, SEQ_COUNT_MASK))
    }

    /// Sets the 14-bit `SequenceCount`.
    pub fn set_sequence_count(&mut self, count: SequenceCount) {
        set_bits_u16(&mut self.packet_sequence_control, SEQ_COUNT_MASK, count.0);
    }

    /// Returns the length of the data field in bytes as specified by the header.
    pub fn data_field_len(&self) -> usize {
        self.packet_data_length.get() as usize + 1
    }

    /// Sets the length of the data field in bytes. The value written to the header
    /// will be `len - 1` as per the CCSDS standard.
    pub fn set_data_field_len(&mut self, len: u16) {
        self.packet_data_length.set(len - 1);
    }

    /// Returns the total length of the packet (header + data field) in bytes.
    pub fn packet_len(&self) -> usize {
        self.data_field_len() + size_of::<PrimaryHeader>()
    }

    /// Reconstructs the CFE-style Message ID (`MsgId`) from the primary header fields.
    ///
    /// This is a crucial convenience function for systems that interact with the cFE
    /// Software Bus (SB). The SB uses a single integer `MsgId` for routing, which is
    /// a composite value created from several fields in the CCSDS header.
    ///
    /// A cFE `MsgId` is 16-bit integer with the following structure:
    ///
    /// ```text
    /// +-----------------+------+-----------------------------------------+
    /// | Field           | Size | Description                             |
    /// +-----------------+------+-----------------------------------------+
    /// | APID            | 11   | The 11-bit Application Process ID.      |
    /// | SB Flag         | 1    | 1 indicates a Software Bus message.     |
    /// | Type            | 1    | 0 for Telemetry, 1 for Telecommand.     |
    /// | Reserved        | 3    | Unused, should be zero.                 |
    /// +-----------------+------+-----------------------------------------+
    /// ```
    pub fn cfe_msg_id(&self) -> u16 {
        const CFE_MSG_ID_BASE: u16 = 0b_00001000_00000000;
        let type_bit = (self.packet_type() as u16) << 12;
        self.apid().0 | type_bit | CFE_MSG_ID_BASE
    }
}

impl From<u16> for SequenceCount {
    fn from(value: u16) -> Self {
        Self(value & Self::MAX)
    }
}

impl Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooShortForHeader { actual } => write!(
                f,
                "slice is too short for a primary header (expected at least {} bytes, got {actual})",
                size_of::<PrimaryHeader>()
            ),
            Self::IncompletePacket {
                header_len,
                buffer_len,
            } => write!(
                f,
                "incomplete packet (header specifies {} bytes, but buffer has only {})",
                header_len, buffer_len
            ),
            Self::Invalid(validation_err) => {
                write!(f, "packet validation failed: {}", validation_err)
            }
        }
    }
}
impl Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(f, "unsupported packet version: {:?}", v),
            Self::IdlePacketWithSecondaryHeader => {
                write!(f, "idle packet is invalid with a secondary header")
            }
        }
    }
}
impl Display for BuildError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BufferTooSmall { required, provided } => write!(
                f,
                "buffer is too small (requires {} bytes, but buffer has only {})",
                required, provided
            ),
            Self::EmptyDataField => write!(f, "a packet data field length of zero is forbidden"),
            Self::InvalidApid { value } => write!(
                f,
                "invalid APID value (must be <= {}, but was {})",
                Apid::MAX,
                value
            ),
            Self::PayloadTooLarge { max, provided } => write!(
                f,
                "payload too large (maximum is {} bytes, but was {})",
                max, provided
            ),
        }
    }
}

impl Display for DataFieldError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SizeMismatch => write!(
                f,
                "size of provided data does not match the packet's data field size"
            ),
            Self::InvalidLayout => write!(
                f,
                "packet data field could not be cast to the target data type"
            ),
            Self::SecondaryHeaderAbsent => {
                write!(f, "secondary header is not present in this packet")
            }
        }
    }
}

impl From<ValidationError> for ParseError {
    fn from(err: ValidationError) -> Self {
        Self::Invalid(err)
    }
}

#[cfg(kani)]
mod kani_harness {
    use super::*;
    use ::kani;

    #[kani::proof]
    fn header_parsing() {
        let mut bytes = [0u8; 1024];

        for i in 0..size_of::<PrimaryHeader>() {
            bytes[i] = kani::any();
        }

        if let Ok(packet) = SpacePacket::parse(&bytes) {
            assert!(packet.packet_len() <= bytes.len());
            assert_eq!(packet.data_field().len(), packet.data_field_len());
            assert!(packet.apid().0 <= Apid::MAX);
        }
    }

    #[kani::proof]
    fn packet_construction() {
        let mut bytes = [kani::any(); 1024];
        let buffer_len = bytes.len();
        let apid = Apid::any();
        let packet_type = kani::any();
        let sequence_count = SequenceCount::any();
        let data_field_len: u16 = kani::any();

        let result = SpacePacket::builder()
            .buffer(&mut bytes)
            .apid(apid)
            .packet_type(packet_type)
            .sequence_count(sequence_count)
            .secondary_header(SecondaryHeaderFlag::Absent)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(data_field_len)
            .build();

        let required_len = size_of::<PrimaryHeader>() + data_field_len as usize;
        let is_valid_request = data_field_len != 0 && required_len <= buffer_len;

        if is_valid_request {
            let packet = result.expect("Should succeed for valid inputs");
            assert_eq!(packet.packet_len(), required_len);
            assert_eq!(packet.data_field_len(), data_field_len as usize);
            assert_eq!(packet.apid(), apid);
            assert_eq!(packet.packet_type(), packet_type);
            assert_eq!(packet.sequence_count(), sequence_count);
        } else {
            assert!(result.is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerocopy::byteorder::network_endian::{F32, U64};

    #[repr(C)]
    #[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Debug, Default)]
    struct TelemetryData {
        timestamp: U64,
        reading: F32,
        status: u8,
    }

    #[test]
    fn build_and_set_data() {
        let telemetry_payload = TelemetryData {
            timestamp: U64::new(1234567890),
            reading: F32::new(3.14159),
            status: 0xAB,
        };

        let apid = Apid::new(42).unwrap();
        let packet_type = PacketType::Telemetry;
        let mut buffer = [0u8; 100];
        let data_len = size_of::<TelemetryData>();

        let packet = SpacePacket::builder()
            .buffer(&mut buffer)
            .apid(apid)
            .packet_type(packet_type)
            .sequence_count(SequenceCount::new())
            .secondary_header(SecondaryHeaderFlag::Absent)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(data_len)
            .build()
            .unwrap();

        packet.set_data_field(&telemetry_payload).unwrap();

        let parsed_packet = SpacePacket::parse(packet.as_bytes()).unwrap();
        let extracted_data = parsed_packet.data_as::<TelemetryData>().unwrap();

        assert_eq!(extracted_data.timestamp.get(), 1234567890);
        assert!((extracted_data.reading.get() - 3.14159).abs() < f32::EPSILON);
        assert_eq!(extracted_data.status, 0xAB);
    }

    #[test]
    fn deserialize_trivial_packet() {
        let bytes = &[
            0b0000_1000u8,
            0b0000_0000u8, // Version, Type, SecHdr, APID
            0b1100_0000u8,
            0b0000_0000u8, // Seq. Flags, Seq. Count
            0b0000_0000u8,
            0b0000_0000u8, // Data Length = 0 (means 1 byte)
            0xDEu8,        // Data Field (1 byte)
        ];
        let packet = SpacePacket::parse(bytes).unwrap();

        assert_eq!(packet.packet_len(), 7);
        assert_eq!(packet.version(), PacketVersion::VERSION_1);
        assert_eq!(packet.packet_type(), PacketType::Telemetry);
        assert_eq!(packet.secondary_header_flag(), SecondaryHeaderFlag::Present);
        assert_eq!(packet.apid(), Apid::new(0).unwrap());
        assert_eq!(packet.sequence_flag(), SequenceFlag::Unsegmented);
        assert_eq!(packet.sequence_count(), SequenceCount::from(0));
        assert_eq!(packet.data_field_len(), 1);
        assert_eq!(packet.data_field(), &[0xDE]);
    }

    #[test]
    fn roundtrip_header_fields() {
        use rand::{RngCore, SeedableRng};
        let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
        let mut buffer = [0u8; 1024];

        for _ in 0..10_000 {
            let packet_type = if rng.next_u32() % 2 == 0 {
                PacketType::Telemetry
            } else {
                PacketType::Telecommand
            };
            let apid = Apid::new((rng.next_u32() & Apid::MAX as u32) as u16).unwrap();
            let sequence_count = SequenceCount::from(rng.next_u32() as u16);
            let data_field_len = 1;

            let packet = SpacePacket::builder()
                .buffer(&mut buffer)
                .apid(apid)
                .packet_type(packet_type)
                .sequence_count(sequence_count)
                .secondary_header(SecondaryHeaderFlag::Absent)
                .sequence_flag(SequenceFlag::Unsegmented)
                .data_len(data_field_len)
                .build()
                .unwrap();

            let parsed = SpacePacket::parse(packet.as_bytes()).unwrap();

            assert_eq!(parsed.packet_type(), packet_type);
            assert_eq!(parsed.apid(), apid);
            assert_eq!(parsed.sequence_count(), sequence_count);
            assert_eq!(parsed.data_field_len(), data_field_len as usize);
        }
    }

    #[test]
    fn error_on_empty_data_field() {
        let mut buffer = [0u8; 7];
        let result = SpacePacket::builder()
            .buffer(&mut buffer)
            .apid(Apid::new(0).unwrap())
            .packet_type(PacketType::Telemetry)
            .sequence_count(SequenceCount::new())
            .secondary_header(SecondaryHeaderFlag::Absent)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(0)
            .build();
        assert_eq!(result, Err(BuildError::EmptyDataField));
    }

    #[test]
    fn error_on_buffer_too_small() {
        let mut buffer = [0u8; 128];
        let data_field_len = 200; // Requires more than 128 bytes
        let result = SpacePacket::builder()
            .buffer(&mut buffer)
            .apid(Apid::new(0).unwrap())
            .packet_type(PacketType::Telemetry)
            .sequence_count(SequenceCount::new())
            .secondary_header(SecondaryHeaderFlag::Absent)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(data_field_len)
            .build();
        assert_eq!(
            result,
            Err(BuildError::BufferTooSmall {
                required: data_field_len as usize + size_of::<PrimaryHeader>(),
                provided: 128
            })
        );
    }

    #[test]
    fn error_on_incomplete_packet_parse() {
        let mut buffer = [0u8; 256];
        let data_field_len = 200;

        // Build a valid packet that is 206 bytes long
        let packet = SpacePacket::builder()
            .buffer(&mut buffer)
            .apid(Apid::new(0).expect("Valid APID"))
            .packet_type(PacketType::Telemetry)
            .sequence_count(SequenceCount::new())
            .secondary_header(SecondaryHeaderFlag::Absent)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(data_field_len)
            .build()
            .expect("Should build successfully");

        // Now try to parse a truncated slice of it
        let truncated_bytes = &packet.as_bytes()[..127];
        let result = SpacePacket::parse(truncated_bytes);

        assert_eq!(
            result,
            Err(ParseError::IncompletePacket {
                header_len: data_field_len as usize + size_of::<PrimaryHeader>(),
                buffer_len: truncated_bytes.len(),
            })
        );
    }

    #[test]
    fn error_on_data_field_size_mismatch() {
        let mut buffer = [0u8; 100];
        // Build a packet expecting a 10-byte data field
        let packet = SpacePacket::builder()
            .buffer(&mut buffer)
            .apid(Apid::new(0).expect("Valid APID"))
            .packet_type(PacketType::Telemetry)
            .sequence_count(SequenceCount::new())
            .secondary_header(SecondaryHeaderFlag::Absent)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(10)
            .build()
            .expect("Should build successfully");

        // Try to set it with a struct that is not 10 bytes
        let wrong_sized_data = TelemetryData::default(); // size_of is not 10
        assert_ne!(size_of::<TelemetryData>(), 10);

        let result = packet.set_data_field(&wrong_sized_data);
        assert_eq!(result, Err(DataFieldError::SizeMismatch));
    }

    #[test]
    fn error_on_invalid_apid() {
        let result = Apid::new(Apid::MAX + 1);
        assert_eq!(
            result,
            Err(BuildError::InvalidApid {
                value: Apid::MAX + 1,
            })
        );
    }
}
