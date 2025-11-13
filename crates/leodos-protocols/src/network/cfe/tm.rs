//! CFE-specific telemetry packet definitions and builder.

use crate::network::spp::Apid;
use crate::network::spp::PacketType;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SecondaryHeaderFlag;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use crate::network::spp::SpacePacket;

use bon::bon;
use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U64;

/// A zero-copy view over a complete CFE telemetry packet (headers + payload).
/// This is the primary struct you will use to represent telemetry.
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// + -- Primary Header (6 bytes) ------ | ------- |
/// |                                    |         |
/// | - Packet Type is always Telemetry  |         |
/// | - Sec. Hdr. Flag is always Present |         |
/// |                                    |         |
/// + -- cFE Secondary Header (2 bytes)  | ------- |
/// |                                    |         |
/// | Time                               | 6 bytes |
/// | Spare                              | 4 bytes |
/// |                                    |         |
/// + -- User Data Field (Variable) ---- | ------- |
/// |                                    |         |
/// | Payload                            | 1-65534 |
/// |                                    | bytes   |
/// +------------------------------------+---------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct Telemetry {
    pub primary: PrimaryHeader,
    pub secondary: TelemetrySecondaryHeader,
    pub payload: [u8],
}

/// The CFE telemetry secondary header (6-byte time + 4-byte padding).
#[repr(C)]
#[derive(IntoBytes, FromBytes, Unaligned, KnownLayout, Immutable, Default, Copy, Clone, Debug)]
pub struct TelemetrySecondaryHeader {
    /// 6-byte CCSDS Day Segmented (CDS) time format with 2 spare bytes.
    pub time: U64,
    /// Padding to ensure the payload that follows is 64-bit aligned.
    pub spare: [u8; 2],
}

pub mod bitmask {
    /// Bitmask for the time field in the telemetry secondary header.
    pub const TIME_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
}

use bitmask::*;

/// An error that can occur when building a CFE telemetry packet.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TelemetryError {
    BufferTooSmall { required: usize, provided: usize },
    InvalidTimeValue,
    SpacePacketError(crate::network::spp::BuildError),
    MissingSecondaryHeader,
    PayloadMismatch,
    TypeMismatch,
}

impl Deref for Telemetry {
    type Target = SpacePacket;

    fn deref(&self) -> &Self::Target {
        SpacePacket::ref_from_bytes(self.as_bytes())
            .expect("Telemetry should always be a valid SpacePacket")
    }
}

impl DerefMut for Telemetry {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SpacePacket::mut_from_bytes(self.as_mut_bytes())
            .expect("Telemetry should always be a valid SpacePacket")
    }
}

impl<'a> TryFrom<&'a SpacePacket> for &'a Telemetry {
    type Error = TelemetryError;

    fn try_from(sp: &'a SpacePacket) -> Result<Self, Self::Error> {
        if sp.secondary_header_flag() != SecondaryHeaderFlag::Present {
            return Err(TelemetryError::MissingSecondaryHeader);
        }

        let bytes = sp.as_bytes();

        match sp.packet_type() {
            PacketType::Telecommand => Err(TelemetryError::TypeMismatch),
            PacketType::Telemetry => {
                Telemetry::ref_from_bytes(bytes).map_err(|_| TelemetryError::PayloadMismatch)
            }
        }
    }
}

#[bon]
impl Telemetry {
    /// Creates a new telemetry packet view over the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        time: u64,
        payload_len: usize,
    ) -> Result<&'a mut Telemetry, TelemetryError> {
        let sp = SpacePacket::builder()
            .buffer(buffer)
            .apid(apid)
            .sequence_count(sequence_count)
            .packet_type(PacketType::Telemetry)
            .secondary_header(SecondaryHeaderFlag::Present)
            .sequence_flag(SequenceFlag::Unsegmented)
            .data_len(size_of::<TelemetrySecondaryHeader>() + payload_len)
            .build()
            .map_err(TelemetryError::SpacePacketError)?;

        let buffer = sp.as_mut_bytes();
        let provided_len = buffer.len();
        let required_len = payload_len;

        let tm = Telemetry::mut_from_bytes_with_elems(buffer, required_len).map_err(|_| {
            TelemetryError::BufferTooSmall {
                required: required_len,
                provided: provided_len,
            }
        })?;

        tm.set_time(time)?;

        Ok(tm)
    }

    pub fn time(&self) -> u64 {
        self.secondary.time.get() & TIME_MASK
    }
    pub fn set_time(&mut self, time: u64) -> Result<(), TelemetryError> {
        if time & !TIME_MASK != 0 {
            return Err(TelemetryError::InvalidTimeValue);
        }
        self.secondary.time.set(time);
        Ok(())
    }

    pub fn payload(&self) -> &[u8] {
        self.payload.as_bytes()
    }
    pub fn payload_mut(&mut self) -> &mut [u8] {
        self.payload.as_mut_bytes()
    }

    pub fn parse<'a>(bytes: &'a [u8]) -> Result<&'a Telemetry, TelemetryError> {
        let sp = SpacePacket::ref_from_bytes(bytes).map_err(|_| TelemetryError::PayloadMismatch)?;
        <&Telemetry>::try_from(sp)
    }
}
