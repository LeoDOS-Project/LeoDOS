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
use crate::utils::validate_checksum_u8;
use bon::bon;
use core::mem::size_of;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::byteorder::network_endian;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum SrsppType {
    Data = 0x00,
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

#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub(crate) struct SrsppHeader {
    source_address: RawAddress,
    packet_type: u8,
}

impl SrsppHeader {
    pub(crate) fn srspp_type(&self) -> Option<SrsppType> {
        SrsppType::from_u8(self.packet_type)
    }

    pub(crate) fn source_address(&self) -> Address {
        self.source_address.parse()
    }

    pub(crate) fn set_source_address(&mut self, address: Address) {
        self.source_address = RawAddress::from(address);
    }

    pub(crate) fn set_srspp_type(&mut self, srspp_type: SrsppType) {
        self.packet_type = srspp_type as u8;
    }
}

#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct AckPayload {
    cumulative_ack: network_endian::U16,
    selective_ack_bitmap: network_endian::U16,
}

impl AckPayload {
    pub(crate) fn new(cumulative_ack: u16, bitmap: u16) -> Self {
        Self {
            cumulative_ack: network_endian::U16::new(cumulative_ack),
            selective_ack_bitmap: network_endian::U16::new(bitmap),
        }
    }

    pub(crate) fn cumulative_ack(&self) -> SequenceCount {
        SequenceCount::from(self.cumulative_ack.get())
    }

    pub(crate) fn selective_ack_bitmap(&self) -> u16 {
        self.selective_ack_bitmap.get()
    }
}

/// ```text
/// +------------------------------------+---------+
/// | Space Packet Primary Header        | 6 bytes |
/// | cFE Telecommand Secondary Header   | 2 bytes |
/// | ISL Routing Header                 | 4 bytes |
/// | SRSPP Header                       | 3 bytes |
/// | Payload                            | N bytes |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable)]
pub struct SrsppDataPacket {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub(crate) isl_header: IslRoutingTelecommandHeader,
    pub(crate) srspp_header: SrsppHeader,
    pub payload: [u8],
}

impl SrsppDataPacket {
    pub const HEADER_SIZE: usize = size_of::<PrimaryHeader>()
        + size_of::<TelecommandSecondaryHeader>()
        + size_of::<IslRoutingTelecommandHeader>()
        + size_of::<SrsppHeader>();

    pub const fn max_payload_size(mtu: usize) -> usize {
        mtu.saturating_sub(Self::HEADER_SIZE)
    }

    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }
}

/// ```text
/// +------------------------------------+---------+
/// | Space Packet Primary Header        | 6 bytes |
/// | cFE Telecommand Secondary Header   | 2 bytes |
/// | ISL Routing Header                 | 4 bytes |
/// | SRSPP Header                       | 3 bytes |
/// | ACK Payload                        | 4 bytes |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, KnownLayout, Unaligned, Immutable, Copy, Clone, Debug)]
pub struct SrsppAckPacket {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub(crate) isl_header: IslRoutingTelecommandHeader,
    pub(crate) srspp_header: SrsppHeader,
    pub(crate) ack_payload: AckPayload,
}

impl SrsppAckPacket {
    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }
}

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
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        target: Address,
        apid: Apid,
        function_code: u8,
        message_id: u8,
        action_code: u8,
        sequence_count: SequenceCount,
        sequence_flag: SequenceFlag,
        payload_len: usize,
    ) -> Result<&'a mut SrsppDataPacket, SrsppPacketError> {
        let required_len = Self::HEADER_SIZE + payload_len;
        let provided_len = buffer.len();

        let (packet, _) =
            SrsppDataPacket::mut_from_prefix_with_elems(buffer, payload_len).map_err(|_| {
                SrsppPacketError::BufferTooSmall {
                    required: required_len,
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
        packet.primary.set_sequence_flag(sequence_flag);

        let data_field_len = size_of::<TelecommandSecondaryHeader>()
            + size_of::<IslRoutingTelecommandHeader>()
            + size_of::<SrsppHeader>()
            + payload_len;
        packet.primary.set_data_field_len(data_field_len as u16);

        packet.secondary.set_function_code(function_code);
        packet.secondary.set_checksum(0);

        packet.isl_header.set_target(target);
        packet.isl_header.set_message_id(message_id);
        packet.isl_header.set_action_code(action_code);

        packet.srspp_header.set_source_address(source_address);
        packet.srspp_header.set_srspp_type(SrsppType::Data);

        packet.set_cfe_checksum();

        Ok(packet)
    }
}

#[bon]
impl SrsppAckPacket {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        target: Address,
        apid: Apid,
        function_code: u8,
        message_id: u8,
        action_code: u8,
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
        packet.isl_header.set_message_id(message_id);
        packet.isl_header.set_action_code(action_code);

        packet.srspp_header.set_source_address(source_address);
        packet.srspp_header.set_srspp_type(SrsppType::Ack);

        packet.ack_payload = AckPayload::new(cumulative_ack.value(), selective_bitmap);

        packet.set_cfe_checksum();

        Ok(packet)
    }
}

pub fn parse_srspp_type(bytes: &[u8]) -> Result<SrsppType, SrsppPacketError> {
    let min_size = SrsppDataPacket::HEADER_SIZE;
    if bytes.len() < min_size {
        return Err(SrsppPacketError::BufferTooSmall {
            required: min_size,
            provided: bytes.len(),
        });
    }

    let type_offset = size_of::<PrimaryHeader>()
        + size_of::<TelecommandSecondaryHeader>()
        + size_of::<IslRoutingTelecommandHeader>()
        + size_of::<RawAddress>();
    SrsppType::from_u8(bytes[type_offset]).ok_or(SrsppPacketError::InvalidPacketType)
}

pub fn parse_data_packet(bytes: &[u8]) -> Result<&SrsppDataPacket, SrsppPacketError> {
    SrsppDataPacket::ref_from_bytes(bytes).map_err(|_| SrsppPacketError::BufferTooSmall {
        required: SrsppDataPacket::HEADER_SIZE,
        provided: bytes.len(),
    })
}

pub fn parse_ack_packet(bytes: &[u8]) -> Result<&SrsppAckPacket, SrsppPacketError> {
    if bytes.len() < size_of::<SrsppAckPacket>() {
        return Err(SrsppPacketError::BufferTooSmall {
            required: size_of::<SrsppAckPacket>(),
            provided: bytes.len(),
        });
    }

    SrsppAckPacket::ref_from_bytes(&bytes[..size_of::<SrsppAckPacket>()])
        .map_err(|_| SrsppPacketError::InvalidPacketType)
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
            .message_id(0x01)
            .action_code(0x20)
            .sequence_count(SequenceCount::from(7))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(payload_data.len())
            .build()
            .unwrap();

        packet.payload.copy_from_slice(payload_data);
        packet.set_cfe_checksum();

        let bytes = packet.as_bytes();

        assert_eq!(parse_srspp_type(bytes).unwrap(), SrsppType::Data);

        let parsed = parse_data_packet(bytes).unwrap();
        assert_eq!(parsed.primary.apid(), apid);
        assert_eq!(parsed.primary.sequence_count().value(), 7);
        assert_eq!(parsed.primary.packet_type(), PacketType::Telecommand);
        assert_eq!(
            parsed.primary.secondary_header_flag(),
            SecondaryHeaderFlag::Present
        );
        assert_eq!(parsed.isl_header.target(), target_address());
        assert_eq!(parsed.isl_header.message_id(), 0x01);
        assert_eq!(parsed.isl_header.action_code(), 0x20);
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
            .message_id(0x02)
            .action_code(0x21)
            .sequence_count(SequenceCount::from(3))
            .cumulative_ack(SequenceCount::from(15))
            .selective_bitmap(0b1100)
            .build()
            .unwrap();

        let bytes = packet.as_bytes();

        assert_eq!(parse_srspp_type(bytes).unwrap(), SrsppType::Ack);

        let parsed = parse_ack_packet(bytes).unwrap();
        assert_eq!(parsed.primary.apid(), apid);
        assert_eq!(parsed.isl_header.target(), target_address());
        assert_eq!(parsed.isl_header.message_id(), 0x02);
        assert_eq!(parsed.isl_header.action_code(), 0x21);
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
            .message_id(0)
            .action_code(0)
            .sequence_count(SequenceCount::from(0))
            .sequence_flag(SequenceFlag::Unsegmented)
            .payload_len(10)
            .build()
            .unwrap();

        assert_eq!(parse_srspp_type(&buffer).unwrap(), SrsppType::Data);

        SrsppAckPacket::builder()
            .buffer(&mut buffer)
            .source_address(source_address())
            .target(target_address())
            .apid(apid)
            .function_code(0)
            .message_id(0)
            .action_code(0)
            .sequence_count(SequenceCount::from(0))
            .cumulative_ack(SequenceCount::from(0))
            .selective_bitmap(0)
            .build()
            .unwrap();

        assert_eq!(parse_srspp_type(&buffer).unwrap(), SrsppType::Ack);
    }
}
