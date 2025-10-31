// In leodos-spacepacket/src/cfe.rs

//! CFE-specific packet definitions, views, and builders.
//!
//! This module builds upon the generic CCSDS `SpacePacket` to provide
//! types that match the exact memory layout of cFE Command and Telemetry messages.

use crate::{
    Apid, PacketSequenceCount, PacketType, PrimaryHeader, SecondaryHeaderFlag, SequenceFlag,
    SpacePacket, SpacePacketData, builder::Vacant, cfe::tc_builder::TelecommandBuilder,
};
use core::{
    mem::size_of,
    ops::{Deref, DerefMut},
};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

/// The CFE command secondary header (2 bytes).
#[repr(C)]
#[derive(IntoBytes, FromBytes, Unaligned, Immutable, KnownLayout, Default, Copy, Clone, Debug)]
pub struct TelecommandSecondaryHeader {
    pub function_code: u8,
    pub checksum: u8,
}

/// A zero-copy view over a complete CFE command packet (headers + payload).
/// This is the primary struct you will use to represent a command.
#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, Immutable, KnownLayout)]
pub struct Telecommand<P: SpacePacketData> {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub payload: P,
}

/// An error that can occur when building a CFE packet.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TelecommandError {
    BufferTooSmall { required: usize, provided: usize },
}

impl<P: SpacePacketData> Deref for Telecommand<P> {
    type Target = SpacePacket;

    fn deref(&self) -> &Self::Target {
        SpacePacket::ref_from_bytes(self.as_bytes())
            .expect("Telecommand should always be a valid SpacePacket")
    }
}

impl<P: SpacePacketData> DerefMut for Telecommand<P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SpacePacket::mut_from_bytes(self.as_mut_bytes())
            .expect("Telecommand should always be a valid SpacePacket")
    }
}

impl Telecommand<()> {
    pub fn builder() -> TelecommandBuilder<Vacant, Vacant, Vacant, Vacant> {
        TelecommandBuilder::new()
    }
}

impl<P: SpacePacketData> Telecommand<P> {
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: PacketSequenceCount,
        function_code: u8,
        payload: &P,
    ) -> Result<&'a mut Telecommand<P>, TelecommandError> {
        let total_size = size_of::<Telecommand<P>>();
        if buffer.len() < total_size {
            return Err(TelecommandError::BufferTooSmall {
                required: total_size,
                provided: buffer.len(),
            });
        }

        let cmd_packet = Telecommand::<P>::mut_from_bytes(&mut buffer[..total_size])
            .expect("Buffer size checked");

        // Populate Primary Header
        cmd_packet
            .primary
            .set_version(crate::PacketVersion::VERSION_1);
        cmd_packet.primary.set_packet_type(PacketType::Telecommand);
        cmd_packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Present);
        cmd_packet.primary.set_apid(apid);
        cmd_packet
            .primary
            .set_sequence_flag(SequenceFlag::Unsegmented);
        cmd_packet.primary.set_sequence_count(sequence_count);

        let data_len = (size_of::<TelecommandSecondaryHeader>() + size_of::<P>()) as u16;
        cmd_packet.primary.set_data_field_len(data_len);

        // Populate Secondary Header
        cmd_packet.secondary.function_code = function_code;

        // Copy payload
        cmd_packet
            .payload
            .as_mut_bytes()
            .copy_from_slice(payload.as_bytes());

        // Automatically calculate and set the final checksum
        cmd_packet.set_cfe_checksum();

        Ok(cmd_packet)
    }
    /// Calculates and sets the 8-bit cFE checksum for this command packet.
    ///
    /// The algorithm is a byte-wise XOR sum of the entire packet,
    /// with the checksum field itself treated as zero during calculation.
    pub fn set_cfe_checksum(&mut self) {
        // Temporarily set the checksum byte to 0 for calculation.
        self.secondary.checksum = 0;

        let bytes = self.as_bytes();
        let calculated_checksum = bytes.iter().fold(0, |acc, &byte| acc ^ byte);

        self.secondary.checksum = calculated_checksum;
    }

    /// Validates the 8-bit cFE checksum.
    ///
    /// Returns `true` if the checksum is valid, `false` otherwise.
    pub fn validate_cfe_checksum(&self) -> bool {
        let bytes = self.as_bytes();
        let checksum = bytes.iter().fold(0, |acc, &byte| acc ^ byte);
        checksum == 0
    }
}
