// In leodos-spacepacket/src/cfe/tm.rs

//! CFE-specific telemetry packet definitions and builder.

use crate::{
    Apid, PacketSequenceCount, PacketType, PrimaryHeader, SecondaryHeaderFlag, SequenceFlag,
    SpacePacketData, builder::Vacant, cfe::tm_builder::TelemetryBuilder,
};
use core::mem::size_of;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout, Unaligned};

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

impl Telemetry<()> {
    /// Creates a new telemetry packet builder.
    pub fn builder() -> TelemetryBuilder<Vacant, Vacant, Vacant> {
        TelemetryBuilder::new()
    }
}

impl<P: SpacePacketData> Telemetry<P> {
    /// Creates a new telemetry packet view over the provided buffer.
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: PacketSequenceCount,
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

        tlm_packet
            .primary
            .set_version(crate::PacketVersion::VERSION_1);
        tlm_packet.primary.set_packet_type(PacketType::Telemetry);
        tlm_packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Present);
        tlm_packet.primary.set_apid(apid);
        tlm_packet
            .primary
            .set_sequence_flag(SequenceFlag::Unsegmented);
        tlm_packet.primary.set_sequence_count(sequence_count);

        let data_len = (size_of::<TelemetrySecondaryHeader>() + size_of::<P>()) as u16;
        tlm_packet.primary.set_data_field_len(data_len);

        tlm_packet.secondary.time = time;

        tlm_packet
            .payload
            .as_mut_bytes()
            .copy_from_slice(payload.as_bytes());

        Ok(tlm_packet)
    }
}
