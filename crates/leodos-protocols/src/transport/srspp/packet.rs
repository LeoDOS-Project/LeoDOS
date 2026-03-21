use crate::network::cfe::tc::TelecommandSecondaryHeader;
use crate::network::isl::address::Address;
use crate::network::isl::address::RawAddress;
use crate::network::isl::routing::packet::IslRoutingTelecommandHeader;
use crate::network::spp::Apid;
use crate::network::spp::PacketType;
use crate::network::spp::PacketVersion;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SecondaryHeaderFlag;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use crate::utils::checksum_u8;
use crate::utils::get_bits_u8;
use crate::utils::set_bits_u8;
use crate::utils::validate_checksum_u8;
use bon::bon;
use core::mem::size_of;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian;

/// SRSPP packet type discriminator.
/// Current SRSPP protocol version.
pub const SRSPP_VERSION: u8 = 0;

/// SRSPP packet type discriminator (2 bits).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum SrsppType {
    /// Data packet carrying application payload.
    Data = 0,
    /// Acknowledgment packet.
    Ack = 1,
    /// End-of-stream signal (no payload).
    Eos = 2,
}

impl TryFrom<u8> for SrsppType {
    type Error = SrsppPacketError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Data),
            1 => Ok(Self::Ack),
            2 => Ok(Self::Eos),
            _ => Err(SrsppPacketError::InvalidPacketType { value }),
        }
    }
}

/// Wire-format SRSPP header following the ISL routing header.
///
/// Byte layout (3 bytes):
///   - source_address: 2 bytes (RawAddress)
///   - version_type:   1 byte  (version[7:6] | type[5:4] | spare[3:0])
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub(crate) struct SrsppHeader {
    /// Source address of the sender.
    source_address: RawAddress,
    /// Packed version (2 bits), type (2 bits), spare (4 bits).
    version_type: u8,
}

#[rustfmt::skip]
mod bitmask {
    /// Bitmask for the 2-bit protocol version field.
    pub const VERSION_MASK: u8 = 0b_1100_0000;
    /// Bitmask for the 2-bit packet type field.
    pub const TYPE_MASK: u8 =    0b_0011_0000;
}

impl SrsppHeader {
    /// Parse the packet type field into an `SrsppType`.
    pub(crate) fn srspp_type(&self) -> Result<SrsppType, SrsppPacketError> {
        SrsppType::try_from(get_bits_u8(self.version_type, bitmask::TYPE_MASK))
    }

    /// Returns the protocol version (2 bits).
    #[allow(unused)]
    pub(crate) fn version(&self) -> u8 {
        get_bits_u8(self.version_type, bitmask::VERSION_MASK)
    }

    /// Returns the parsed source address.
    pub(crate) fn source_address(&self) -> Address {
        self.source_address.parse()
    }

    /// Sets the source address field.
    pub(crate) fn set_source_address(&mut self, address: Address) {
        self.source_address = RawAddress::from(address);
    }

    /// Sets the version and packet type fields.
    pub(crate) fn set_srspp_type(&mut self, srspp_type: SrsppType) {
        set_bits_u8(&mut self.version_type, bitmask::VERSION_MASK, SRSPP_VERSION);
        set_bits_u8(&mut self.version_type, bitmask::TYPE_MASK, srspp_type as u8);
    }
}

/// Payload of an SRSPP acknowledgment packet.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct AckPayload {
    /// Sequence number up to which all packets are acknowledged.
    cumulative_ack: network_endian::U16,
    /// Bitmap of selectively acknowledged packets beyond the cumulative ack.
    selective_ack_bitmap: network_endian::U16,
}

impl AckPayload {
    /// Creates a new ACK payload with the given cumulative ack and bitmap.
    pub(crate) fn new(cumulative_ack: u16, bitmap: u16) -> Self {
        Self {
            cumulative_ack: network_endian::U16::new(cumulative_ack),
            selective_ack_bitmap: network_endian::U16::new(bitmap),
        }
    }

    /// Returns the cumulative acknowledgment sequence number.
    ///
    /// Note: Used by CFS and Tokio
    #[allow(dead_code)]
    pub(crate) fn cumulative_ack(&self) -> SequenceCount {
        SequenceCount::from(self.cumulative_ack.get())
    }

    /// Returns the selective acknowledgment bitmap.
    ///
    /// Note: Used by CFS and Tokio
    #[allow(dead_code)]
    pub(crate) fn selective_ack_bitmap(&self) -> u16 {
        self.selective_ack_bitmap.get()
    }
}

/// An SRSPP packet of unknown type (data or ack).
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct SrsppPacket {
    /// Space Packet primary header.
    pub primary: PrimaryHeader,
    /// cFE telecommand secondary header.
    pub secondary: TelecommandSecondaryHeader,
    /// ISL routing header for inter-satellite addressing.
    pub(crate) isl_header: IslRoutingTelecommandHeader,
    /// SRSPP protocol header.
    pub(crate) srspp_header: SrsppHeader,
    /// Remaining bytes (payload for data, ack fields for ack).
    pub rest: [u8],
}

impl SrsppPacket {
    /// Parse an SRSPP packet from a raw byte buffer.
    pub fn parse(bytes: &[u8]) -> Result<&Self, SrsppPacketError> {
        Self::ref_from_bytes(bytes).map_err(|_| SrsppPacketError::BufferTooSmall {
            required: SrsppDataPacket::HEADER_SIZE,
            provided: bytes.len(),
        })
    }

    /// Returns the SRSPP packet type.
    pub fn srspp_type(&self) -> Result<SrsppType, SrsppPacketError> {
        self.srspp_header.srspp_type()
    }

    /// Returns the source address from the SRSPP header.
    pub fn source_address(&self) -> Address {
        self.srspp_header.source_address()
    }
}

/// ```text
/// +------------------------------------+---------+
/// | Space Packet Primary Header        | 6 bytes |
/// | cFE Telecommand Secondary Header   | 2 bytes |
/// | ISL Routing Header                 | 2 bytes |
/// | SRSPP Header                       | 3 bytes |
/// | Payload                            | N bytes |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct SrsppDataPacket {
    /// Space Packet primary header.
    pub primary: PrimaryHeader,
    /// cFE telecommand secondary header.
    pub secondary: TelecommandSecondaryHeader,
    /// ISL routing header for inter-satellite addressing.
    pub(crate) isl_header: IslRoutingTelecommandHeader,
    /// SRSPP protocol header.
    pub(crate) srspp_header: SrsppHeader,
    /// Variable-length application payload.
    pub payload: [u8],
}

impl SrsppDataPacket {
    /// Total header size in bytes (SPP + cFE + ISL + SRSPP).
    pub const HEADER_SIZE: usize = size_of::<PrimaryHeader>()
        + size_of::<TelecommandSecondaryHeader>()
        + size_of::<IslRoutingTelecommandHeader>()
        + size_of::<SrsppHeader>();

    /// Parse a data packet reference from a raw byte buffer.
    pub fn parse(bytes: &[u8]) -> Result<&Self, SrsppPacketError> {
        Self::ref_from_bytes(bytes).map_err(|_| SrsppPacketError::BufferTooSmall {
            required: Self::HEADER_SIZE,
            provided: bytes.len(),
        })
    }

    /// Maximum payload bytes that fit within the given MTU.
    pub const fn max_payload_size(mtu: usize) -> usize {
        mtu.saturating_sub(Self::HEADER_SIZE)
    }

    /// Compute and set the cFE checksum over the entire packet.
    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    /// Validate the cFE checksum of this packet.
    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }
}

/// ```text
/// +------------------------------------+---------+
/// | Space Packet Primary Header        | 6 bytes |
/// | cFE Telecommand Secondary Header   | 2 bytes |
/// | ISL Routing Header                 | 2 bytes |
/// | SRSPP Header                       | 3 bytes |
/// | ACK Payload                        | 4 bytes |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct SrsppAckPacket {
    /// Space Packet primary header.
    pub primary: PrimaryHeader,
    /// cFE telecommand secondary header.
    pub secondary: TelecommandSecondaryHeader,
    /// ISL routing header for inter-satellite addressing.
    pub(crate) isl_header: IslRoutingTelecommandHeader,
    /// SRSPP protocol header.
    pub(crate) srspp_header: SrsppHeader,
    /// Acknowledgment payload with cumulative ack and selective bitmap.
    pub(crate) ack_payload: AckPayload,
}

impl SrsppAckPacket {
    /// Parse an ACK packet reference from a raw byte buffer.
    pub fn parse(bytes: &[u8]) -> Result<&Self, SrsppPacketError> {
        if bytes.len() < size_of::<Self>() {
            return Err(SrsppPacketError::BufferTooSmall {
                required: size_of::<Self>(),
                provided: bytes.len(),
            });
        }
        Self::ref_from_bytes(&bytes[..size_of::<Self>()]).map_err(|_| {
            SrsppPacketError::BufferTooSmall {
                required: size_of::<Self>(),
                provided: bytes.len(),
            }
        })
    }

    /// Compute and set the cFE checksum over the entire packet.
    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    /// Validate the cFE checksum of this packet.
    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }
}

/// Errors that can occur when constructing or parsing SRSPP packets.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SrsppPacketError {
    /// Buffer is too small for the packet.
    #[error("buffer too small: required {required} bytes, provided {provided} bytes")]
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual number of bytes provided.
        provided: usize,
    },
    /// Packet type byte is not a valid SRSPP type.
    #[error("invalid SRSPP packet type: {value:#04x}")]
    InvalidPacketType {
        /// The unrecognised packet-type byte.
        value: u8,
    },
    /// Payload exceeds the maximum allowed size.
    #[error("payload too large: maximum {max} bytes, provided {provided} bytes")]
    PayloadTooLarge {
        /// Maximum allowed payload size in bytes.
        max: usize,
        /// Actual payload size in bytes.
        provided: usize,
    },
}

#[bon]
impl SrsppDataPacket {
    /// Build a new SRSPP data packet in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        target: Address,
        apid: Apid,
        function_code: u8,
        sequence_count: SequenceCount,
        sequence_flag: SequenceFlag,
        payload_len: usize,
    ) -> Result<&'a mut SrsppDataPacket, SrsppPacketError> {
        let required_len = Self::HEADER_SIZE + payload_len;
        let provided_len = buffer.len();

        let (packet, _) = SrsppDataPacket::mut_from_prefix_with_elems(buffer, payload_len)
            .map_err(|_| SrsppPacketError::BufferTooSmall {
                required: required_len,
                provided: provided_len,
            })?;

        packet.primary.set_version(PacketVersion::VERSION_1);
        packet.primary.set_packet_type(PacketType::Telecommand);
        packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Present);
        packet.primary.set_apid(apid);
        packet.primary.set_sequence_count(sequence_count);
        packet.primary.set_sequence_flag(sequence_flag);

        let data_field_len = size_of::<TelecommandSecondaryHeader>()
            + size_of::<IslRoutingTelecommandHeader>()
            + size_of::<SrsppHeader>()
            + payload_len;
        packet.primary.set_data_field_len(data_field_len as u16);

        packet.secondary.set_function_code(function_code);
        packet.secondary.set_checksum(0);

        packet.isl_header.set_target(target);

        packet.srspp_header.set_source_address(source_address);
        packet.srspp_header.set_srspp_type(SrsppType::Data);

        packet.set_cfe_checksum();

        Ok(packet)
    }
}

#[bon]
impl SrsppAckPacket {
    /// Build a new SRSPP acknowledgment packet in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        target: Address,
        apid: Apid,
        function_code: u8,
        sequence_count: SequenceCount,
        cumulative_ack: SequenceCount,
        selective_bitmap: u16,
    ) -> Result<&'a mut SrsppAckPacket, SrsppPacketError> {
        let provided_len = buffer.len();
        let (packet, _) = SrsppAckPacket::mut_from_prefix(buffer).map_err(|_| {
            SrsppPacketError::BufferTooSmall {
                required: size_of::<SrsppAckPacket>(),
                provided: provided_len,
            }
        })?;

        packet.primary.set_version(PacketVersion::VERSION_1);
        packet.primary.set_packet_type(PacketType::Telecommand);
        packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Present);
        packet.primary.set_apid(apid);
        packet.primary.set_sequence_count(sequence_count);
        packet.primary.set_sequence_flag(SequenceFlag::Unsegmented);

        let data_field_len = size_of::<TelecommandSecondaryHeader>()
            + size_of::<IslRoutingTelecommandHeader>()
            + size_of::<SrsppHeader>()
            + size_of::<AckPayload>();
        packet.primary.set_data_field_len(data_field_len as u16);

        packet.secondary.set_function_code(function_code);
        packet.secondary.set_checksum(0);

        packet.isl_header.set_target(target);

        packet.srspp_header.set_source_address(source_address);
        packet.srspp_header.set_srspp_type(SrsppType::Ack);

        packet.ack_payload = AckPayload::new(cumulative_ack.value(), selective_bitmap);

        packet.set_cfe_checksum();

        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_address() -> Address {
        Address::satellite(1, 5)
    }

    fn target_address() -> Address {
        Address::satellite(2, 3)
    }

    #[test]
    fn test_data_roundtrip() {
        let mut buffer = [0u8; 256];
        let apid = Apid::new(0x42).unwrap();
        let payload_data = b"hello SRSPP";

        let packet = SrsppDataPacket::builder()
            .buffer(&mut buffer)
            .source_address(source_address())
            .target(target_address())
            .apid(apid)
            .function_code(0x10)
            .sequence_count(SequenceCount::from(7))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(payload_data.len())
            .build()
            .unwrap();

        packet.payload.copy_from_slice(payload_data);
        packet.set_cfe_checksum();

        let bytes = packet.as_bytes();

        let header = SrsppPacket::parse(bytes).unwrap();
        assert_eq!(header.srspp_type().unwrap(), SrsppType::Data);

        let parsed = SrsppDataPacket::parse(bytes).unwrap();
        assert_eq!(parsed.primary.apid(), apid);
        assert_eq!(parsed.primary.sequence_count().value(), 7);
        assert_eq!(parsed.primary.packet_type(), PacketType::Telecommand);
        assert_eq!(
            parsed.primary.secondary_header_flag(),
            SecondaryHeaderFlag::Present
        );
        assert_eq!(parsed.isl_header.target(), target_address());
        assert_eq!(parsed.secondary.function_code(), 0x10);
        assert_eq!(parsed.srspp_header.source_address(), source_address());
        assert_eq!(&parsed.payload, payload_data);
        assert!(parsed.validate_cfe_checksum());
    }

    #[test]
    fn test_ack_roundtrip() {
        let mut buffer = [0u8; 64];
        let apid = Apid::new(0x42).unwrap();

        let packet = SrsppAckPacket::builder()
            .buffer(&mut buffer)
            .source_address(source_address())
            .target(target_address())
            .apid(apid)
            .function_code(0x10)
            .sequence_count(SequenceCount::from(3))
            .cumulative_ack(SequenceCount::from(15))
            .selective_bitmap(0b1100)
            .build()
            .unwrap();

        let bytes = packet.as_bytes();

        let header = SrsppPacket::parse(bytes).unwrap();
        assert_eq!(header.srspp_type().unwrap(), SrsppType::Ack);

        let parsed = SrsppAckPacket::parse(bytes).unwrap();
        assert_eq!(parsed.primary.apid(), apid);
        assert_eq!(parsed.isl_header.target(), target_address());
        assert_eq!(parsed.srspp_header.source_address(), source_address());
        assert_eq!(parsed.ack_payload.cumulative_ack().value(), 15);
        assert_eq!(parsed.ack_payload.selective_ack_bitmap(), 0b1100);
        assert!(parsed.validate_cfe_checksum());
    }

    #[test]
    fn test_parse_srspp_type() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        SrsppDataPacket::builder()
            .buffer(&mut buffer)
            .source_address(source_address())
            .target(target_address())
            .apid(apid)
            .function_code(0)
            .sequence_count(SequenceCount::from(0))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(10)
            .build()
            .unwrap();

        assert_eq!(
            SrsppPacket::parse(&buffer).unwrap().srspp_type().unwrap(),
            SrsppType::Data,
        );

        SrsppAckPacket::builder()
            .buffer(&mut buffer)
            .source_address(source_address())
            .target(target_address())
            .apid(apid)
            .function_code(0)
            .sequence_count(SequenceCount::from(0))
            .cumulative_ack(SequenceCount::from(0))
            .selective_bitmap(0)
            .build()
            .unwrap();

        assert_eq!(
            SrsppPacket::parse(&buffer).unwrap().srspp_type().unwrap(),
            SrsppType::Ack,
        );
    }
}
