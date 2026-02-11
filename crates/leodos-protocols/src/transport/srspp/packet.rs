//! SRSPP packet definitions using zerocopy.
//!
//! SRSPP adds a minimal header to Space Packets to distinguish DATA from ACK
//! packets and carry acknowledgment information.

use crate::network::isl::address::Address;
use crate::network::isl::address::RawAddress;
use crate::network::spp::Apid;
use crate::network::spp::PacketType;
use crate::network::spp::PacketVersion;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SecondaryHeaderFlag;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use bon::bon;
use core::mem::size_of;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian;

/// SRSP packet type identifier.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum SrsppType {
    /// Data packet carrying application payload
    Data = 0x00,
    /// Acknowledgment packet
    Ack = 0x01,
}

impl SrsppType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Data),
            0x01 => Some(Self::Ack),
            _ => None,
        }
    }
}

/// The SRSP header (3 bytes).
///
/// Contains source address for stream identification and packet type.
/// The sequence information comes from the Space Packet primary header.
///
/// For ACK packets, additional fields carry acknowledgment information
/// in the AckPayload that follows.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct SrsppHeader {
    /// Source address of the sender.
    pub source_address: RawAddress,
    /// Packet type: DATA (0x00) or ACK (0x01)
    pub packet_type: u8,
}

impl SrsppHeader {
    pub const SIZE: usize = size_of::<Self>();

    pub fn srspp_type(&self) -> Option<SrsppType> {
        SrsppType::from_u8(self.packet_type)
    }

    pub fn source_address(&self) -> Address {
        self.source_address.parse()
    }
}

/// ACK-specific payload following the SRSP header.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct AckPayload {
    /// Highest in-order sequence number received (cumulative ACK)
    cumulative_ack: network_endian::U16,
    /// Bitmap of received out-of-order packets relative to cumulative_ack.
    /// Bit 0 = cumulative_ack + 1, Bit 1 = cumulative_ack + 2, etc.
    selective_ack_bitmap: network_endian::U16,
}

impl AckPayload {
    pub const SIZE: usize = size_of::<Self>();

    pub fn new(cumulative_ack: u16, bitmap: u16) -> Self {
        Self {
            cumulative_ack: network_endian::U16::new(cumulative_ack),
            selective_ack_bitmap: network_endian::U16::new(bitmap),
        }
    }

    pub fn cumulative_ack(&self) -> SequenceCount {
        SequenceCount::from(self.cumulative_ack.get())
    }

    pub fn selective_ack_bitmap(&self) -> u16 {
        self.selective_ack_bitmap.get()
    }
}

/// A zero-copy view over an SRSP DATA packet.
///
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// | Space Packet Primary Header        | 6 bytes |
/// |   - Sequence Count (for reliability)         |
/// |   - Sequence Flags (for segmentation)        |
/// +------------------------------------+---------+
/// | SRSP Header                        | 3 bytes |
/// |   - Source Address (2 bytes)                 |
/// |   - Type: DATA (0x00)                        |
/// +------------------------------------+---------+
/// | Payload                            | N bytes |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct SrsppDataPacket {
    pub primary: PrimaryHeader,
    pub srspp_header: SrsppHeader,
    pub payload: [u8],
}

impl SrsppDataPacket {
    pub const HEADER_SIZE: usize = size_of::<PrimaryHeader>() + SrsppHeader::SIZE;

    /// Maximum payload size for a given MTU
    pub const fn max_payload_size(mtu: usize) -> usize {
        mtu.saturating_sub(Self::HEADER_SIZE)
    }
}

/// A zero-copy view over an SRSP ACK packet.
///
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// | Space Packet Primary Header        | 6 bytes |
/// +------------------------------------+---------+
/// | SRSP Header                        | 3 bytes |
/// |   - Source Address (2 bytes)                 |
/// |   - Type: ACK (0x01)                         |
/// +------------------------------------+---------+
/// | ACK Payload                        | 4 bytes |
/// |   - Cumulative ACK (u16)                     |
/// |   - Selective ACK Bitmap (u16)               |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct SrsppAckPacket {
    pub primary: PrimaryHeader,
    pub srspp_header: SrsppHeader,
    pub ack_payload: AckPayload,
}

impl SrsppAckPacket {
    pub const SIZE: usize = size_of::<Self>();
}

/// Errors that can occur when building SRSP packets.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SrsppPacketError {
    #[error("buffer too small: required {required} bytes, provided {provided} bytes")]
    BufferTooSmall { required: usize, provided: usize },
    #[error("invalid SRSP packet type")]
    InvalidPacketType,
    #[error("payload too large: maximum {max} bytes, provided {provided} bytes")]
    PayloadTooLarge { max: usize, provided: usize },
}

#[bon]
impl SrsppDataPacket {
    /// Build a DATA packet in the provided buffer.
    ///
    /// Returns a mutable reference to the packet, allowing payload to be written.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        apid: Apid,
        sequence_count: SequenceCount,
        sequence_flag: SequenceFlag,
        payload_len: usize,
    ) -> Result<&'a mut SrsppDataPacket, SrsppPacketError> {
        let required_len = SrsppDataPacket::HEADER_SIZE + payload_len;
        let provided_len = buffer.len();

        let (packet, _) = SrsppDataPacket::mut_from_prefix_with_elems(buffer, payload_len)
            .map_err(|_| SrsppPacketError::BufferTooSmall {
                required: required_len,
                provided: provided_len,
            })?;

        // Set primary header fields
        packet.primary.set_version(PacketVersion::VERSION_1);
        packet.primary.set_packet_type(PacketType::Telemetry);
        packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Absent);
        packet.primary.set_apid(apid);
        packet.primary.set_sequence_count(sequence_count);
        packet.primary.set_sequence_flag(sequence_flag);
        packet
            .primary
            .set_data_field_len((SrsppHeader::SIZE + payload_len) as u16);

        // Set SRSP header
        packet.srspp_header.source_address = RawAddress::from(source_address);
        packet.srspp_header.packet_type = SrsppType::Data as u8;

        Ok(packet)
    }
}

#[bon]
impl SrsppAckPacket {
    /// Build an ACK packet in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        apid: Apid,
        sequence_count: SequenceCount,
        cumulative_ack: SequenceCount,
        selective_bitmap: u16,
    ) -> Result<&'a mut SrsppAckPacket, SrsppPacketError> {
        let provided_len = buffer.len();
        let (packet, _) = SrsppAckPacket::mut_from_prefix(buffer).map_err(|_| {
            SrsppPacketError::BufferTooSmall {
                required: SrsppAckPacket::SIZE,
                provided: provided_len,
            }
        })?;

        // Set primary header fields
        packet.primary.set_version(PacketVersion::VERSION_1);
        packet.primary.set_packet_type(PacketType::Telemetry);
        packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Absent);
        packet.primary.set_apid(apid);
        packet.primary.set_sequence_count(sequence_count);
        packet.primary.set_sequence_flag(SequenceFlag::Unsegmented);
        packet
            .primary
            .set_data_field_len((SrsppHeader::SIZE + AckPayload::SIZE) as u16);

        // Set SRSP header
        packet.srspp_header.source_address = RawAddress::from(source_address);
        packet.srspp_header.packet_type = SrsppType::Ack as u8;

        // Set ACK payload
        packet.ack_payload = AckPayload::new(cumulative_ack.value(), selective_bitmap);

        Ok(packet)
    }
}

/// Parse an SRSP packet type from a byte slice.
pub fn parse_srspp_type(bytes: &[u8]) -> Result<SrsppType, SrsppPacketError> {
    if bytes.len() < size_of::<PrimaryHeader>() + SrsppHeader::SIZE {
        return Err(SrsppPacketError::BufferTooSmall {
            required: size_of::<PrimaryHeader>() + SrsppHeader::SIZE,
            provided: bytes.len(),
        });
    }

    let type_offset = size_of::<PrimaryHeader>() + size_of::<RawAddress>();
    let srspp_type_byte = bytes[type_offset];
    SrsppType::from_u8(srspp_type_byte).ok_or(SrsppPacketError::InvalidPacketType)
}

/// Parse an SRSP DATA packet from a byte slice.
pub fn parse_data_packet(bytes: &[u8]) -> Result<&SrsppDataPacket, SrsppPacketError> {
    SrsppDataPacket::ref_from_bytes(bytes).map_err(|_| SrsppPacketError::BufferTooSmall {
        required: SrsppDataPacket::HEADER_SIZE,
        provided: bytes.len(),
    })
}

/// Parse an SRSP ACK packet from a byte slice.
pub fn parse_ack_packet(bytes: &[u8]) -> Result<&SrsppAckPacket, SrsppPacketError> {
    if bytes.len() < SrsppAckPacket::SIZE {
        return Err(SrsppPacketError::BufferTooSmall {
            required: SrsppAckPacket::SIZE,
            provided: bytes.len(),
        });
    }

    SrsppAckPacket::ref_from_bytes(&bytes[..SrsppAckPacket::SIZE])
        .map_err(|_| SrsppPacketError::InvalidPacketType)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address() -> Address {
        Address::satellite(1, 5)
    }

    #[test]
    fn test_build_data_packet() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        let packet = SrsppDataPacket::builder()
            .buffer(&mut buffer)
            .source_address(test_address())
            .apid(apid)
            .sequence_count(SequenceCount::from(5))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(10)
            .build()
            .unwrap();

        assert_eq!(packet.primary.apid(), apid);
        assert_eq!(packet.primary.sequence_count().value(), 5);
        assert_eq!(packet.srspp_header.packet_type, SrsppType::Data as u8);
        assert_eq!(packet.srspp_header.source_address(), test_address());
        assert_eq!(packet.payload.len(), 10);
    }

    #[test]
    fn test_build_ack_packet() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        let packet = SrsppAckPacket::builder()
            .buffer(&mut buffer)
            .source_address(test_address())
            .apid(apid)
            .sequence_count(SequenceCount::from(1))
            .cumulative_ack(SequenceCount::from(10))
            .selective_bitmap(0b1010)
            .build()
            .unwrap();

        assert_eq!(packet.primary.apid(), apid);
        assert_eq!(packet.srspp_header.packet_type, SrsppType::Ack as u8);
        assert_eq!(packet.srspp_header.source_address(), test_address());
        assert_eq!(packet.ack_payload.cumulative_ack().value(), 10);
        assert_eq!(packet.ack_payload.selective_ack_bitmap(), 0b1010);
    }

    #[test]
    fn test_parse_srspp_type() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        // Build a DATA packet
        SrsppDataPacket::builder()
            .buffer(&mut buffer)
            .source_address(test_address())
            .apid(apid)
            .sequence_count(SequenceCount::from(0))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(10)
            .build()
            .unwrap();

        assert_eq!(parse_srspp_type(&buffer).unwrap(), SrsppType::Data);

        // Build an ACK packet
        SrsppAckPacket::builder()
            .buffer(&mut buffer)
            .source_address(test_address())
            .apid(apid)
            .sequence_count(SequenceCount::from(0))
            .cumulative_ack(SequenceCount::from(0))
            .selective_bitmap(0)
            .build()
            .unwrap();

        assert_eq!(parse_srspp_type(&buffer).unwrap(), SrsppType::Ack);
    }
}
