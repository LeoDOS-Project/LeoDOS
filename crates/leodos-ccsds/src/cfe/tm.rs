//! CFE-specific telemetry packet definitions and builder.

use crate::spp::Apid;
use crate::spp::PacketType;
use crate::spp::PacketVersion;
use crate::spp::PrimaryHeader;
use crate::spp::SecondaryHeaderFlag;
use crate::spp::SequenceCount;
use crate::spp::SequenceFlag;
use crate::spp::SpacePacket;
use crate::spp::SpacePacketData;

use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

/// The CFE telemetry secondary header (6-byte time + 4-byte padding).
#[repr(C)]
#[derive(IntoBytes, FromBytes, Unaligned, KnownLayout, Immutable, Default, Copy, Clone, Debug)]
pub struct TelemetrySecondaryHeader {
    /// 6-byte CCSDS Day Segmented (CDS) time format.
    pub time: [u8; 6],
    /// Padding to ensure the payload that follows is 64-bit aligned.
    pub spare: [u8; 4],
}

/// A zero-copy view over a complete CFE telemetry packet (headers + payload).
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct Telemetry<P: SpacePacketData> {
    pub primary: PrimaryHeader,
    pub secondary: TelemetrySecondaryHeader,
    pub payload: P,
}

/// An error that can occur when building a CFE telemetry packet.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TelemetryError {
    BufferTooSmall { required: usize, provided: usize },
}

impl<P: SpacePacketData> Deref for Telemetry<P> {
    type Target = SpacePacket;

    fn deref(&self) -> &Self::Target {
        SpacePacket::ref_from_bytes(self.as_bytes())
            .expect("Telemetry should always be a valid SpacePacket")
    }
}

impl<P: SpacePacketData> DerefMut for Telemetry<P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SpacePacket::mut_from_bytes(self.as_mut_bytes())
            .expect("Telemetry should always be a valid SpacePacket")
    }
}

impl<P: SpacePacketData> Telemetry<P> {
    /// Creates a new telemetry packet view over the provided buffer.
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        time: [u8; 6],
        payload: &P,
    ) -> Result<&'a mut Telemetry<P>, TelemetryError> {
        let total_size = size_of::<Telemetry<P>>();
        if buffer.len() < total_size {
            return Err(TelemetryError::BufferTooSmall {
                required: total_size,
                provided: buffer.len(),
            });
        }

        let tlm_packet = Telemetry::<P>::mut_from_bytes(&mut buffer[..total_size])
            .expect("should not fail due to size check");

        tlm_packet.primary.set_version(PacketVersion::VERSION_1);
        tlm_packet.primary.set_packet_type(PacketType::Telemetry);
        tlm_packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Present);
        tlm_packet.primary.set_apid(apid);
        tlm_packet
            .primary
            .set_sequence_flag(SequenceFlag::Unsegmented);
        tlm_packet.primary.set_sequence_count(sequence_count);

        tlm_packet
            .primary
            .set_data_field_len((size_of::<TelemetrySecondaryHeader>() + size_of::<P>()) as u16);

        tlm_packet.secondary.time = time;

        tlm_packet
            .payload
            .as_mut_bytes()
            .copy_from_slice(payload.as_bytes());

        Ok(tlm_packet)
    }
}
