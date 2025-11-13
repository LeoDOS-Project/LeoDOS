//! SRSP packet definitions using zerocopy.
//!
//! SRSP adds a minimal header to Space Packets to distinguish DATA from ACK
//! packets and carry acknowledgment information.

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
pub enum SrspType {
    /// Data packet carrying application payload
    Data = 0x00,
    /// Acknowledgment packet
    Ack = 0x01,
}

impl SrspType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Data),
            0x01 => Some(Self::Ack),
            _ => None,
        }
    }
}

/// The SRSP header (1 byte).
///
/// For DATA packets, only the type field is used. The sequence information
/// comes from the Space Packet primary header.
///
/// For ACK packets, additional fields carry acknowledgment information
/// in the AckPayload that follows.
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct SrspHeader {
    /// Packet type: DATA (0x00) or ACK (0x01)
    pub packet_type: u8,
}

impl SrspHeader {
    pub const SIZE: usize = size_of::<Self>();

    pub fn srsp_type(&self) -> Option<SrspType> {
        SrspType::from_u8(self.packet_type)
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
/// | SRSP Header                        | 1 byte  |
/// |   - Type: DATA (0x00)                        |
/// +------------------------------------+---------+
/// | Payload                            | N bytes |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct SrspDataPacket {
    pub primary: PrimaryHeader,
    pub srsp_header: SrspHeader,
    pub payload: [u8],
}

impl SrspDataPacket {
    pub const HEADER_SIZE: usize = size_of::<PrimaryHeader>() + SrspHeader::SIZE;

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
/// | SRSP Header                        | 1 byte  |
/// |   - Type: ACK (0x01)                         |
/// +------------------------------------+---------+
/// | ACK Payload                        | 4 bytes |
/// |   - Cumulative ACK (u16)                     |
/// |   - Selective ACK Bitmap (u16)               |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct SrspAckPacket {
    pub primary: PrimaryHeader,
    pub srsp_header: SrspHeader,
    pub ack_payload: AckPayload,
}

impl SrspAckPacket {
    pub const SIZE: usize = size_of::<Self>();
}

/// Errors that can occur when building SRSP packets.
#[derive(Debug, Copy, Clone, Eq, PartialEq, thiserror::Error)]
pub enum SrspPacketError {
    #[error("buffer too small: required {required} bytes, provided {provided} bytes")]
    BufferTooSmall { required: usize, provided: usize },
    #[error("invalid SRSP packet type")]
    InvalidPacketType,
    #[error("payload too large: maximum {max} bytes, provided {provided} bytes")]
    PayloadTooLarge { max: usize, provided: usize },
}

#[bon]
impl SrspDataPacket {
    /// Build a DATA packet in the provided buffer.
    ///
    /// Returns a mutable reference to the packet, allowing payload to be written.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        sequence_flag: SequenceFlag,
        payload_len: usize,
    ) -> Result<&'a mut SrspDataPacket, SrspPacketError> {
        let required_len = SrspDataPacket::HEADER_SIZE + payload_len;
        let provided_len = buffer.len();

        let (packet, _) = SrspDataPacket::mut_from_prefix_with_elems(buffer, required_len)
            .map_err(|_| SrspPacketError::BufferTooSmall {
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
            .set_data_field_len((SrspHeader::SIZE + payload_len) as u16);

        // Set SRSP header
        packet.srsp_header.packet_type = SrspType::Data as u8;

        Ok(packet)
    }
}

#[bon]
impl SrspAckPacket {
    /// Build an ACK packet in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        cumulative_ack: SequenceCount,
        selective_bitmap: u16,
    ) -> Result<&'a mut SrspAckPacket, SrspPacketError> {
        let provided_len = buffer.len();
        let (packet, _) = SrspAckPacket::mut_from_prefix(buffer).map_err(|_| {
            SrspPacketError::BufferTooSmall {
                required: SrspAckPacket::SIZE,
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
            .set_data_field_len((SrspHeader::SIZE + AckPayload::SIZE) as u16);

        // Set SRSP header
        packet.srsp_header.packet_type = SrspType::Ack as u8;

        // Set ACK payload
        packet.ack_payload = AckPayload::new(cumulative_ack.value(), selective_bitmap);

        Ok(packet)
    }
}

/// Parse an SRSP packet type from a byte slice.
pub fn parse_srsp_type(bytes: &[u8]) -> Result<SrspType, SrspPacketError> {
    if bytes.len() < size_of::<PrimaryHeader>() + SrspHeader::SIZE {
        return Err(SrspPacketError::BufferTooSmall {
            required: size_of::<PrimaryHeader>() + SrspHeader::SIZE,
            provided: bytes.len(),
        });
    }

    let srsp_type_byte = bytes[size_of::<PrimaryHeader>()];
    SrspType::from_u8(srsp_type_byte).ok_or(SrspPacketError::InvalidPacketType)
}

/// Parse an SRSP DATA packet from a byte slice.
pub fn parse_data_packet(bytes: &[u8]) -> Result<&SrspDataPacket, SrspPacketError> {
    SrspDataPacket::ref_from_bytes(bytes).map_err(|_| SrspPacketError::BufferTooSmall {
        required: SrspDataPacket::HEADER_SIZE,
        provided: bytes.len(),
    })
}

/// Parse an SRSP ACK packet from a byte slice.
pub fn parse_ack_packet(bytes: &[u8]) -> Result<&SrspAckPacket, SrspPacketError> {
    if bytes.len() < SrspAckPacket::SIZE {
        return Err(SrspPacketError::BufferTooSmall {
            required: SrspAckPacket::SIZE,
            provided: bytes.len(),
        });
    }

    SrspAckPacket::ref_from_bytes(&bytes[..SrspAckPacket::SIZE])
        .map_err(|_| SrspPacketError::InvalidPacketType)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_data_packet() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        let packet = SrspDataPacket::builder()
            .buffer(&mut buffer)
            .apid(apid)
            .sequence_count(SequenceCount::from(5))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(10)
            .build()
            .unwrap();

        assert_eq!(packet.primary.apid(), apid);
        assert_eq!(packet.primary.sequence_count().value(), 5);
        assert_eq!(packet.srsp_header.packet_type, SrspType::Data as u8);
        assert_eq!(packet.payload.len(), 10);
    }

    #[test]
    fn test_build_ack_packet() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        let packet = SrspAckPacket::builder()
            .buffer(&mut buffer)
            .apid(apid)
            .sequence_count(SequenceCount::from(1))
            .cumulative_ack(SequenceCount::from(10))
            .selective_bitmap(0b1010)
            .build()
            .unwrap();

        assert_eq!(packet.primary.apid(), apid);
        assert_eq!(packet.srsp_header.packet_type, SrspType::Ack as u8);
        assert_eq!(packet.ack_payload.cumulative_ack().value(), 10);
        assert_eq!(packet.ack_payload.selective_ack_bitmap(), 0b1010);
    }

    #[test]
    fn test_parse_srsp_type() {
        let mut buffer = [0u8; 128];
        let apid = Apid::new(0x42).unwrap();

        // Build a DATA packet
        SrspDataPacket::builder()
            .buffer(&mut buffer)
            .apid(apid)
            .sequence_count(SequenceCount::from(0))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(10)
            .build()
            .unwrap();

        assert_eq!(parse_srsp_type(&buffer).unwrap(), SrspType::Data);

        // Build an ACK packet
        SrspAckPacket::builder()
            .buffer(&mut buffer)
            .apid(apid)
            .sequence_count(SequenceCount::from(0))
            .cumulative_ack(SequenceCount::from(0))
            .selective_bitmap(0)
            .build()
            .unwrap();

        assert_eq!(parse_srsp_type(&buffer).unwrap(), SrspType::Ack);
    }
}
