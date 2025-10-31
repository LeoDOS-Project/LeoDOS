use crate::builder::SpacePacketBuilder;
use crate::crc::CrcError;
use crate::crc::CrcSpacePacket;
use crate::Apid;
use crate::PacketSequenceCount;
use crate::PacketType;

impl<'a, A, B, C, F> SpacePacketBuilder<A, B, C, &'a mut [u8], u16, F> {
    /// Prepares the builder to construct a packet with a managed CRC.
    pub fn crc<'b>(
        self,
        crc_alg: &'b crc::Crc<u16>,
    ) -> SpacePacketBuilder<A, B, C, &'a mut [u8], u16, &'b crc::Crc<u16>> {
        SpacePacketBuilder {
            apid: self.apid,
            packet_type: self.packet_type,
            sequence_count: self.sequence_count,
            secondary_header_flag: self.secondary_header_flag,
            sequence_flag: self.sequence_flag,
            buffer: self.buffer,
            data_field_len: self.data_field_len,
            crc_alg,
        }
    }
}

impl<'a, 'b>
    SpacePacketBuilder<Apid, PacketType, PacketSequenceCount, &'a mut [u8], u16, &'b crc::Crc<u16>>
{
    pub fn build(self) -> Result<CrcSpacePacket<'a, 'b>, CrcError> {
        CrcSpacePacket::new(
            self.buffer,
            self.apid,
            self.packet_type,
            self.sequence_count,
            self.secondary_header_flag,
            self.sequence_flag,
            self.data_field_len,
            self.crc_alg,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Apid, PacketSequenceCount, PacketType, SpacePacket};
    use crc::{CRC_16_IBM_3740, Crc};
    use zerocopy::byteorder::network_endian::U32;

    const CRC_ALG: Crc<u16> = Crc::<u16>::new(&CRC_16_IBM_3740);

    #[test]
    fn set_and_get_data_with_crc() {
        let mut buffer = [0u8; 100];
        let payload_1 = U32::new(0xDEADBEEF);
        let payload_2 = U32::new(0xCAFEBABE);

        // 1. Build the CRC-managed packet structure.
        let mut crc_packet = SpacePacket::builder()
            .apid(Apid::IDLE)
            .packet_type(PacketType::Telemetry)
            .sequence_count(PacketSequenceCount::new())
            .buffer(&mut buffer)
            .data_len(size_of::<U32>() as u16)
            .crc(&CRC_ALG)
            .build()
            .unwrap();

        // At this point, the CRC is valid for the zeroed data field.
        assert!(crc_packet.validate().is_ok());

        // 2. Set the data. This automatically updates the CRC.
        crc_packet.set_data(&payload_1).unwrap();

        // The CRC should now be valid for the new data.
        assert!(crc_packet.validate().is_ok());
        let data = crc_packet.data_as::<U32>().unwrap();
        assert_eq!(data.get(), 0xDEADBEEF);

        // 3. Manually corrupt the data field (bypassing the safe API).
        crc_packet.data_field_mut()[0] = 0x00;

        // Validation should now fail.
        assert!(crc_packet.validate().is_err());
        assert!(matches!(
            crc_packet.data_as::<U32>(),
            Err(CrcError::ValidationFailed { .. })
        ));

        // 4. Set new data, which "fixes" the CRC.
        crc_packet.set_data(&payload_2).unwrap();
        assert!(crc_packet.validate().is_ok());
        let data = crc_packet.data_as::<U32>().unwrap();
        assert_eq!(data.get(), 0xCAFEBABE);
    }
}
