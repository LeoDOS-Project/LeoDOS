//! CCSDS Space Packet Protocol with CRC-16 Support
//!
//! Spec: https://ccsds.org/Pubs/232x0b4e1c1.pdf

use crate::network::spp::Apid;
use crate::network::spp::BuildError;
use crate::network::spp::PacketType;
use crate::network::spp::PacketVersion;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SequenceCount;
use crate::network::spp::SpacePacket;
use crate::network::spp::SpacePacketData;

use core::fmt::Display;
use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;
use zerocopy::FromBytes;
use zerocopy::IntoBytes;
use zerocopy::byteorder::network_endian::U16;

/// A wrapper around a `SpacePacket` that automatically manages
/// a trailing CRC-16 checksum.
pub struct CrcSpacePacket<'a, 'b> {
    packet: &'a mut SpacePacket,
    crc_bytes: &'a mut [u8],
    crc_alg: &'b crc::Crc<u16>,
}

/// An error that can occur during CRC-aware Space Packet construction.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum BuilderError {
    /// The underlying Space Packet build failed.
    Spec(BuildError),
    /// The buffer is too small to hold the packet and its CRC.
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual buffer size provided.
        provided: usize,
    },
}

impl Display for BuilderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuilderError::Spec(e) => write!(f, "Specification error: {e}"),
            BuilderError::BufferTooSmall { required, provided } => {
                write!(
                    f,
                    "Buffer too small for CRC packet: required {required}, provided {provided}"
                )
            }
        }
    }
}

/// An error related to CRC operations.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum CrcError {
    /// An error occurred during the underlying packet build.
    Build(BuilderError),
    /// The buffer was too small to hold the packet and its CRC.
    BufferTooSmall {
        /// Minimum number of bytes needed.
        required: usize,
        /// Actual buffer size provided.
        provided: usize,
    },
    /// The calculated CRC did not match the one in the buffer.
    ValidationFailed {
        /// CRC value stored in the packet.
        expected: u16,
        /// CRC value recomputed from the packet contents.
        calculated: u16,
    },
    /// An error occurred parsing the underlying data field.
    DataField(crate::network::spp::DataFieldError),
}

impl core::fmt::Display for CrcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CrcError::Build(e) => write!(f, "Build error: {e}"),
            CrcError::BufferTooSmall { required, provided } => {
                write!(
                    f,
                    "Buffer too small for CRC packet: required {required}, provided {provided}"
                )
            }
            CrcError::ValidationFailed {
                expected,
                calculated,
            } => {
                write!(
                    f,
                    "CRC validation failed: expected {expected:#06X}, calculated {calculated:#06X}"
                )
            }
            CrcError::DataField(e) => write!(f, "Data field error: {e}"),
        }
    }
}

impl From<BuilderError> for CrcError {
    fn from(e: BuilderError) -> Self {
        CrcError::Build(e)
    }
}
impl From<crate::network::spp::DataFieldError> for CrcError {
    fn from(e: crate::network::spp::DataFieldError) -> Self {
        CrcError::DataField(e)
    }
}

impl<'a, 'b> CrcSpacePacket<'a, 'b> {
    /// Creates a new CRC-protected Space Packet in the provided buffer.
    pub fn new(
        buffer: &'a mut [u8],
        apid: Apid,
        packet_type: PacketType,
        sequence_count: SequenceCount,
        secondary_header_flag: crate::network::spp::SecondaryHeaderFlag,
        sequence_flag: crate::network::spp::SequenceFlag,
        data_field_len: u16,
        crc_alg: &'b crc::Crc<u16>,
    ) -> Result<CrcSpacePacket<'a, 'b>, CrcError> {
        let required_len = size_of::<PrimaryHeader>() + data_field_len as usize;
        if required_len + 2 > buffer.len() {
            return Err(CrcError::BufferTooSmall {
                required: required_len + 2,
                provided: buffer.len(),
            });
        }

        let (packet_buf, crc_buf) = buffer[..required_len + 2].split_at_mut(required_len);
        let packet = SpacePacket::mut_from_bytes(packet_buf).unwrap();

        // Build the header
        packet.set_version(PacketVersion::VERSION_1);
        packet.set_packet_type(packet_type);
        packet.set_apid(apid);
        packet.set_sequence_count(sequence_count);
        packet.set_data_field_len(data_field_len);
        packet.set_secondary_header_flag(secondary_header_flag);
        packet.set_sequence_flag(sequence_flag);

        // Create the wrapper
        let mut crc_packet = CrcSpacePacket {
            packet,
            crc_bytes: crc_buf,
            crc_alg: crc_alg,
        };

        // Set the initial CRC
        crc_packet.update_crc();

        Ok(crc_packet)
    }
    /// Writes data to the packet's data field and automatically updates the CRC.
    ///
    /// This is the safe, CRC-aware way to set the packet's payload.
    pub fn set_data<T: SpacePacketData>(
        &mut self,
        data: &T,
    ) -> Result<(), crate::network::spp::DataFieldError> {
        self.packet.set_data_field(data)?;
        self.update_crc();
        Ok(())
    }

    /// Validates the CRC and returns a typed, zero-copy view of the data field.
    pub fn data_as<T: SpacePacketData>(&self) -> Result<&T, CrcError> {
        self.validate()?;
        Ok(self.packet.data_as::<T>()?)
    }

    /// Validates the CRC and returns an immutable slice of the data field.
    pub fn data(&self) -> Result<&[u8], CrcError> {
        self.validate()?;
        Ok(self.packet.data_field())
    }

    /// Validates the current CRC against the packet's contents.
    pub fn validate(&self) -> Result<(), CrcError> {
        let expected = U16::read_from_bytes(self.crc_bytes).unwrap().get();
        let calculated = self.crc_alg.checksum(self.packet.as_bytes());
        if expected == calculated {
            Ok(())
        } else {
            Err(CrcError::ValidationFailed {
                expected,
                calculated,
            })
        }
    }

    /// Forces a recalculation and update of the CRC value.
    /// This is called automatically by `set_data_field`.
    pub fn update_crc(&mut self) {
        let calculated = self.crc_alg.checksum(self.packet.as_bytes());
        U16::new(calculated)
            .write_to_prefix(self.crc_bytes)
            .unwrap();
    }
}

// Allow direct access to the underlying SpacePacket header fields (e.g., `crc_packet.apid()`).
impl<'a, 'b> Deref for CrcSpacePacket<'a, 'b> {
    type Target = SpacePacket;
    fn deref(&self) -> &Self::Target {
        self.packet
    }
}
impl<'a, 'b> DerefMut for CrcSpacePacket<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.packet
    }
}
