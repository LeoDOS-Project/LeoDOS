//! CFE-specific packet definitions, views, and builders.
//!
//! This module builds upon the generic CCSDS `SpacePacket` to provide
//! types that match the exact memory layout of cFE Command and Telemetry messages.

use crate::network::spp;
use crate::network::spp::Apid;
use crate::network::spp::PacketType;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SecondaryHeaderFlag;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use crate::network::spp::SpacePacket;
use crate::utils::checksum_u8;
use crate::utils::validate_checksum_u8;
use bon::bon;
use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// A zero-copy view over a complete CFE command packet (headers + payload).
/// This is the primary struct you will use to represent a command.
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// + -- Primary Header (6 bytes) ------ | ------- |
/// |                                    |         |
/// | - Packet Type is always Telecommand|         |
/// | - Sec. Hdr. Flag is always Present |         |
/// |                                    |         |
/// + -- cFE Secondary Header (2 bytes)  | ------- |
/// |                                    |         |
/// | Function Code                      | 1 byte  |
/// | Checksum                           | 1 byte  |
/// |                                    |         |
/// + -- User Data Field (Variable) ---- | ------- |
/// |                                    |         |
/// | Payload                            | 1-65534 |
/// |                                    | bytes   |
/// +------------------------------------+---------+
/// ```
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, Immutable, KnownLayout)]
pub struct Telecommand {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub payload: [u8],
}
/// The CFE command secondary header (2 bytes).
#[repr(C)]
#[derive(IntoBytes, FromBytes, Unaligned, Immutable, KnownLayout, Default, Copy, Clone, Debug)]
pub struct TelecommandSecondaryHeader {
    function_code: u8,
    checksum: u8,
}

impl TelecommandSecondaryHeader {
    pub fn function_code(&self) -> u8 {
        self.function_code
    }

    pub fn set_function_code(&mut self, function_code: u8) {
        self.function_code = function_code;
    }

    pub fn checksum(&self) -> u8 {
        self.checksum
    }

    pub fn set_checksum(&mut self, checksum: u8) {
        self.checksum = checksum;
    }
}

/// An error that can occur when building a CFE packet.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TelecommandError {
    BufferTooSmall {
        required_len: usize,
        provided_len: usize,
    },
    SpacePacketBuildError(spp::BuildError),
    SpacePacketParseError(spp::ParseError),
    MissingSecondaryHeader,
    PayloadMismatch,
    TypeMismatch,
}

impl Deref for Telecommand {
    type Target = SpacePacket;

    fn deref(&self) -> &Self::Target {
        SpacePacket::ref_from_bytes(self.as_bytes())
            .expect("Telecommand should always be a valid SpacePacket")
    }
}

impl DerefMut for Telecommand {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SpacePacket::mut_from_bytes(self.as_mut_bytes())
            .expect("Telecommand should always be a valid SpacePacket")
    }
}

impl<'a> TryFrom<&'a SpacePacket> for &'a Telecommand {
    type Error = TelecommandError;

    fn try_from(sp: &'a SpacePacket) -> Result<Self, Self::Error> {
        if sp.secondary_header_flag() != SecondaryHeaderFlag::Present {
            return Err(TelecommandError::MissingSecondaryHeader);
        }

        let bytes = sp.as_bytes();

        match sp.packet_type() {
            PacketType::Telecommand => {
                Telecommand::ref_from_bytes(bytes).map_err(|_| TelecommandError::PayloadMismatch)
            }
            PacketType::Telemetry => Err(TelecommandError::TypeMismatch),
        }
    }
}

#[bon]
impl Telecommand {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        function_code: u8,
        payload_len: usize,
    ) -> Result<&'a mut Telecommand, TelecommandError> {
        let sp = SpacePacket::builder()
            .buffer(buffer)
            .apid(apid)
            .packet_type(PacketType::Telecommand)
            .sequence_count(sequence_count)
            .secondary_header(SecondaryHeaderFlag::Present)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(size_of::<TelecommandSecondaryHeader>() + payload_len)
            .build()
            .map_err(TelecommandError::SpacePacketBuildError)?;

        let buffer = sp.as_mut_bytes();
        let provided_len = buffer.len();
        let required_len = payload_len;
        let tc = Telecommand::mut_from_bytes_with_elems(buffer, required_len).map_err(|_| {
            TelecommandError::BufferTooSmall {
                required_len,
                provided_len,
            }
        })?;

        tc.set_function_code(function_code);
        tc.set_cfe_checksum();

        Ok(tc)
    }

    pub const fn size_minimum() -> usize {
        size_of::<PrimaryHeader>() + size_of::<TelecommandSecondaryHeader>()
    }

    pub fn function_code(&self) -> u8 {
        self.secondary.function_code()
    }
    pub fn set_function_code(&mut self, function_code: u8) {
        self.secondary.set_function_code(function_code);
    }

    /// Calculates and sets the 8-bit cFE checksum for this command packet.
    ///
    /// The algorithm is a byte-wise XOR sum of the entire packet,
    /// with the checksum field itself treated as zero during calculation.
    pub fn set_cfe_checksum(&mut self) {
        // Temporarily set the checksum byte to 0 for calculation.
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    /// Validates the 8-bit cFE checksum.
    ///
    /// Returns `true` if the checksum is valid, `false` otherwise.
    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.payload
    }

    pub fn parse<'a>(bytes: &'a [u8]) -> Result<&'a Telecommand, TelecommandError> {
        let sp = SpacePacket::parse(bytes).map_err(TelecommandError::SpacePacketParseError)?;
        <&'a Telecommand>::try_from(sp)
    }

    pub fn as_spacepacket(&self) -> &SpacePacket {
        &**self
    }
}
