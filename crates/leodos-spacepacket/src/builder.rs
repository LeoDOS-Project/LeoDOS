//! A typed builder for constructing a `SpacePacket`.

use core::fmt::Debug;

use crate::Apid;
use crate::BuildError;
use crate::PacketSequenceCount;
use crate::PacketType;
use crate::SecondaryHeaderFlag;
use crate::SequenceFlag;
use crate::SpacePacket;
use crate::SpacePacketData;

pub struct Vacant;

/// A typed builder for defining the specification of a `SpacePacket`.
#[derive(Clone)]
pub struct SpacePacketBuilder<A, B, C, D, E, F> {
    pub(crate) apid: A,
    pub(crate) packet_type: B,
    pub(crate) sequence_count: C,
    pub(crate) secondary_header_flag: SecondaryHeaderFlag,
    pub(crate) sequence_flag: SequenceFlag,
    pub(crate) buffer: D,
    pub(crate) data_field_len: E,
    pub(crate) crc_alg: F,
}

/// An error that occurs during the building of a `SpacePacket`.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum BuilderError {
    Spec(BuildError),
    BufferTooSmall { required: usize, provided: usize },
}

impl core::fmt::Display for BuilderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BuilderError::Spec(e) => write!(f, "Specification error: {e}"),
            BuilderError::BufferTooSmall { required, provided } => {
                write!(
                    f,
                    "Buffer too small: required {required}, provided {provided}"
                )
            }
        }
    }
}

impl From<BuildError> for BuilderError {
    fn from(e: BuildError) -> Self {
        BuilderError::Spec(e)
    }
}

impl SpacePacketBuilder<Vacant, Vacant, Vacant, Vacant, Vacant, Vacant> {
    pub(crate) fn new() -> Self {
        Self {
            apid: Vacant,
            packet_type: Vacant,
            sequence_count: Vacant,
            secondary_header_flag: SecondaryHeaderFlag::default(),
            sequence_flag: SequenceFlag::default(),
            buffer: Vacant,
            data_field_len: Vacant,
            crc_alg: Vacant,
        }
    }
}

impl<A, B, C, D, E, F> SpacePacketBuilder<A, B, C, D, E, F> {
    pub fn secondary_header(mut self, flag: SecondaryHeaderFlag) -> Self {
        self.secondary_header_flag = flag;
        self
    }
    pub fn sequence_flag(mut self, flag: SequenceFlag) -> Self {
        self.sequence_flag = flag;
        self
    }
}

impl<B, C, D, E, F> SpacePacketBuilder<Vacant, B, C, D, E, F> {
    pub fn apid(self, apid: Apid) -> SpacePacketBuilder<Apid, B, C, D, E, F> {
        SpacePacketBuilder {
            apid,
            packet_type: self.packet_type,
            sequence_count: self.sequence_count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            data_field_len: self.data_field_len,
            buffer: self.buffer,
            crc_alg: self.crc_alg,
        }
    }
}

impl<A, C, D, E, F> SpacePacketBuilder<A, Vacant, C, D, E, F> {
    pub fn packet_type(
        self,
        packet_type: PacketType,
    ) -> SpacePacketBuilder<A, PacketType, C, D, E, F> {
        SpacePacketBuilder {
            apid: self.apid,
            packet_type,
            sequence_count: self.sequence_count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            data_field_len: self.data_field_len,
            buffer: self.buffer,
            crc_alg: self.crc_alg,
        }
    }
}
impl<A, B, D, E, F> SpacePacketBuilder<A, B, Vacant, D, E, F> {
    pub fn sequence_count(
        self,
        count: PacketSequenceCount,
    ) -> SpacePacketBuilder<A, B, PacketSequenceCount, D, E, F> {
        SpacePacketBuilder {
            apid: self.apid,
            packet_type: self.packet_type,
            sequence_count: count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            data_field_len: self.data_field_len,
            buffer: self.buffer,
            crc_alg: self.crc_alg,
        }
    }
}

impl<A, B, C, E, F> SpacePacketBuilder<A, B, C, Vacant, E, F> {
    pub fn buffer<'a>(
        self,
        buffer: &'a mut [u8],
    ) -> SpacePacketBuilder<A, B, C, &'a mut [u8], E, F> {
        SpacePacketBuilder {
            apid: self.apid,
            packet_type: self.packet_type,
            sequence_count: self.sequence_count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            data_field_len: self.data_field_len,
            buffer,
            crc_alg: self.crc_alg,
        }
    }
}

impl<A, B, C, D, F> SpacePacketBuilder<A, B, C, D, Vacant, F> {
    /// Specifies the length of an uninitialized data field.
    pub fn data_len(self, data_field_len: u16) -> SpacePacketBuilder<A, B, C, D, u16, F> {
        SpacePacketBuilder {
            apid: self.apid,
            packet_type: self.packet_type,
            sequence_count: self.sequence_count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            buffer: self.buffer,
            data_field_len,
            crc_alg: self.crc_alg,
        }
    }

    /// Specifies the length of an uninitialized data field.
    pub fn data_type<T: SpacePacketData>(self) -> SpacePacketBuilder<A, B, C, D, u16, F> {
        SpacePacketBuilder {
            apid: self.apid,
            packet_type: self.packet_type,
            sequence_count: self.sequence_count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            buffer: self.buffer,
            data_field_len: core::mem::size_of::<T>() as u16,
            crc_alg: self.crc_alg,
        }
    }
}

impl<'a> SpacePacketBuilder<Apid, PacketType, PacketSequenceCount, &'a mut [u8], u16, Vacant> {
    /// Consumes the writer and builds the final `SpacePacket` view.
    pub fn build(self) -> Result<&'a mut SpacePacket, BuilderError> {
        let sp = SpacePacket::new(
            self.buffer,
            self.apid,
            self.packet_type,
            self.sequence_count,
            self.secondary_header_flag,
            self.sequence_flag,
            self.data_field_len,
        )?;
        Ok(sp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerocopy::IntoBytes;
    use zerocopy::byteorder::network_endian::U32;

    #[test]
    fn new_builder_happy_path() {
        let mut buffer = [0u8; 100];
        let payload = U32::new(0xDEADBEEF);

        let packet = SpacePacket::builder()
            .apid(Apid::IDLE)
            .packet_type(PacketType::Telemetry)
            .sequence_count(PacketSequenceCount::new())
            .buffer(&mut buffer)
            .data_type::<U32>()
            .build()
            .unwrap();

        packet.set_data_field(&payload).unwrap();

        assert_eq!(packet.data_field(), payload.as_bytes());
        assert_eq!(packet.data_field_len(), size_of::<U32>());
        assert_eq!(packet.apid(), Apid::IDLE);
    }
}
