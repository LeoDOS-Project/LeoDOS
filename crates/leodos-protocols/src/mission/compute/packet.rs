//! SpaceCoMP command packet format.
//!
//! Used by the coordinator to assign roles (collector, mapper, reducer)
//! to satellites, and by satellites to report phase completion.
//!
//! Built on CFE Telecommand packets, following the same pattern as
//! ColoniesPacket in `mission/colonies/messages.rs`.

use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;

use crate::network::cfe::tc::Telecommand;
use crate::network::cfe::tc::TelecommandError;
use crate::network::cfe::tc::TelecommandSecondaryHeader;
use crate::network::isl::address::RawAddress;
use crate::network::spp::Apid;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SequenceCount;
use crate::network::spp::SpacePacket;
use crate::utils::checksum_u8;
use crate::utils::validate_checksum_u8;

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OpCode {
    SubmitJob = 0x00,
    AssignCollector = 0x01,
    AssignMapper = 0x02,
    AssignReducer = 0x03,
    PhaseDone = 0x04,
    JobResult = 0x05,
    DataChunk = 0x10,
}

impl TryFrom<u8> for OpCode {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::SubmitJob),
            0x01 => Ok(Self::AssignCollector),
            0x02 => Ok(Self::AssignMapper),
            0x03 => Ok(Self::AssignReducer),
            0x04 => Ok(Self::PhaseDone),
            0x05 => Ok(Self::JobResult),
            0x10 => Ok(Self::DataChunk),
            _ => Err(()),
        }
    }
}

#[repr(C)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct SpaceCompPacket {
    pub primary: PrimaryHeader,
    pub secondary: TelecommandSecondaryHeader,
    pub header: SpaceCompHeader,
    pub payload: [u8],
}

#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct SpaceCompHeader {
    pub op_code: u8,
    pub _reserved: u8,
    pub job_id: U16,
}

impl SpaceCompHeader {
    pub fn op_code(&self) -> Result<OpCode, ()> {
        self.op_code.try_into()
    }

    pub fn set_op_code(&mut self, op_code: OpCode) {
        self.op_code = op_code as u8;
    }

    pub fn job_id(&self) -> u16 {
        self.job_id.get()
    }

    pub fn set_job_id(&mut self, job_id: u16) {
        self.job_id = U16::new(job_id);
    }
}

/// Payload for AssignCollector: tells a satellite which mapper to send data to.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignCollectorPayload {
    pub mapper_addr: RawAddress,
    pub partition_id: u8,
}

/// Payload for AssignMapper: tells a satellite where to send results.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignMapperPayload {
    pub reducer_addr: RawAddress,
    pub collector_count: u8,
}

/// Payload for AssignReducer: tells a satellite where to send final output.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignReducerPayload {
    pub los_addr: RawAddress,
    pub mapper_count: u8,
}

/// Payload for PhaseDone: satellite reports completion.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct PhaseDonePayload {
    pub role: u8,
}

impl Deref for SpaceCompPacket {
    type Target = SpacePacket;
    fn deref(&self) -> &Self::Target {
        SpacePacket::ref_from_bytes(self.as_bytes())
            .expect("SpaceCompPacket layout is a superset of SpacePacket")
    }
}

impl DerefMut for SpaceCompPacket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SpacePacket::mut_from_bytes(self.as_mut_bytes())
            .expect("SpaceCompPacket layout is a superset of SpacePacket")
    }
}

#[derive(Debug)]
pub enum SpaceCompPacketError {
    Telecommand(TelecommandError),
    PayloadTooLarge,
    PayloadTooSmall,
}

impl SpaceCompPacket {
    pub fn new<'a>(
        buffer: &'a mut [u8],
        apid: Apid,
        sequence_count: SequenceCount,
        op_code: OpCode,
        job_id: u16,
        payload_len: usize,
    ) -> Result<&'a mut Self, SpaceCompPacketError> {
        if payload_len > u16::MAX as usize {
            return Err(SpaceCompPacketError::PayloadTooLarge);
        }

        let tc = Telecommand::builder()
            .buffer(buffer)
            .apid(apid)
            .sequence_count(sequence_count)
            .function_code(0)
            .payload_len(size_of::<SpaceCompHeader>() + payload_len)
            .build()
            .map_err(SpaceCompPacketError::Telecommand)?;

        let required_len = size_of::<PrimaryHeader>()
            + size_of::<TelecommandSecondaryHeader>()
            + size_of::<SpaceCompHeader>()
            + payload_len;

        let buffer = tc.as_mut_bytes();
        let provided_len = buffer.len();

        let packet = Self::mut_from_bytes(buffer).map_err(|_| {
            SpaceCompPacketError::Telecommand(TelecommandError::BufferTooSmall {
                required_len,
                provided_len,
            })
        })?;

        packet.header.set_op_code(op_code);
        packet.header.set_job_id(job_id);

        Ok(packet)
    }

    pub fn set_cfe_checksum(&mut self) {
        self.secondary.checksum = 0;
        self.secondary.checksum = checksum_u8(self.as_bytes());
    }

    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    pub fn parse(bytes: &[u8]) -> Result<&Self, SpaceCompPacketError> {
        let tc = Telecommand::parse(bytes).map_err(SpaceCompPacketError::Telecommand)?;
        <&SpaceCompPacket>::try_from(tc)
    }
}

impl<'a> TryFrom<&'a Telecommand> for &'a SpaceCompPacket {
    type Error = SpaceCompPacketError;

    fn try_from(tc: &'a Telecommand) -> Result<Self, Self::Error> {
        if tc.payload().len() < size_of::<SpaceCompHeader>() {
            return Err(SpaceCompPacketError::PayloadTooSmall);
        }
        Ok(SpaceCompPacket::ref_from_bytes(tc.as_bytes()).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let mut buf = [0u8; 64];
        let payload_len = size_of::<AssignCollectorPayload>();

        let pkt = SpaceCompPacket::new(
            &mut buf,
            Apid::new(42).unwrap(),
            SequenceCount::from(1),
            OpCode::AssignCollector,
            0x1234,
            payload_len,
        )
        .unwrap();

        let assign = AssignCollectorPayload {
            mapper_addr: RawAddress::from(crate::network::isl::address::Address::satellite(1, 5)),
            partition_id: 3,
        };
        pkt.payload[..payload_len].copy_from_slice(assign.as_bytes());
        pkt.set_cfe_checksum();

        let parsed = SpaceCompPacket::parse(pkt.as_bytes()).unwrap();
        assert_eq!(parsed.header.op_code(), Ok(OpCode::AssignCollector));
        assert_eq!(parsed.header.job_id(), 0x1234);
        assert!(parsed.validate_cfe_checksum());

        let parsed_payload =
            AssignCollectorPayload::read_from_bytes(&parsed.payload[..payload_len]).unwrap();
        assert_eq!(parsed_payload.partition_id, 3);
    }

    #[test]
    fn test_all_opcodes_roundtrip() {
        for code in [
            OpCode::SubmitJob,
            OpCode::AssignCollector,
            OpCode::AssignMapper,
            OpCode::AssignReducer,
            OpCode::PhaseDone,
            OpCode::JobResult,
            OpCode::DataChunk,
        ] {
            let val = code as u8;
            assert_eq!(OpCode::try_from(val), Ok(code));
        }
    }
}
