//! SpaceCoMP message format.
//!
//! Used by the coordinator to assign roles (collector, mapper, reducer)
//! to satellites, and by satellites to report phase completion.

use bon::bon;
use core::mem::size_of;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;

use crate::network::isl::address::{Address, RawAddress};

/// Operation codes for the SpaceCoMP MapReduce protocol.
///
/// A job proceeds in phases: the coordinator assigns roles to
/// satellites, collectors send data chunks to mappers, mappers
/// forward intermediate results to the reducer, and the reducer
/// sends the final result back to the coordinator.
///
/// ```text
///  Coordinator                Collector         Mapper         Reducer
///      │                          │                │               │
///      ├─ AssignCollector ───────►│                │               │
///      ├─ AssignMapper ───────────┼───────────────►│               │
///      ├─ AssignReducer ──────────┼───────────────►┼──────────────►│
///      │                          │                │               │
///      │                          ├─ DataChunk ───►│               │
///      │                          │                ├─ DataChunk ──►│
///      │◄─────────────────────────┼── JobResult ───┼───────────────┤
/// ```
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

/// Application-level SpaceCoMP message: header + variable payload.
///
/// This is a zero-copy view over the data delivered by the transport
/// layer (SRSPP). Lower-layer headers (SPP, SRSPP, ISL) have already
/// been stripped; only the SpaceCoMP header and role-specific payload
/// remain.
///
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// | OpCode                             | 8 bits  |
/// | Reserved                           | 8 bits  |
/// | Job ID                             | 16 bits |
/// | -- Payload (Variable) ------------ | ------- |
/// | Role-specific payload              | 0-N     |
/// |                                    | bytes   |
/// +------------------------------------+---------+
/// ```
#[repr(C, packed)]
#[derive(FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable)]
pub struct SpaceCompMessage {
    header: SpaceCompHeader,
    payload: [u8],
}

impl SpaceCompMessage {
    pub const HEADER_SIZE: usize = size_of::<SpaceCompHeader>();

    pub fn parse(bytes: &[u8]) -> Option<&Self> {
        Self::ref_from_bytes(bytes).ok()
    }

    pub fn op_code(&self) -> Result<OpCode, ()> {
        self.header.op_code()
    }

    pub fn job_id(&self) -> u16 {
        self.header.job_id()
    }

    pub fn header(&self) -> &SpaceCompHeader {
        &self.header
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn as_bytes(&self) -> &[u8] {
        zerocopy::IntoBytes::as_bytes(self)
    }
}

#[bon]
impl SpaceCompMessage {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        op_code: OpCode,
        job_id: u16,
        payload: &[u8],
    ) -> Option<&'a SpaceCompMessage> {
        let (msg, _) = Self::mut_from_prefix_with_elems(buffer, payload.len()).ok()?;
        msg.header = SpaceCompHeader::new(op_code, job_id);
        msg.payload.copy_from_slice(payload);
        Some(msg)
    }
}

/// The 4-byte SpaceCoMP header present in every SpaceCoMP message.
///
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// | OpCode                             | 8 bits  |
/// | Reserved                           | 8 bits  |
/// | Job ID                             | 16 bits |
/// +------------------------------------+---------+
/// ```
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct SpaceCompHeader {
    op_code: u8,
    _reserved: u8,
    job_id: U16,
}

impl SpaceCompHeader {
    pub fn new(op_code: OpCode, job_id: u16) -> Self {
        Self {
            op_code: op_code as u8,
            _reserved: 0,
            job_id: U16::new(job_id),
        }
    }

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

/// Payload for [`OpCode::AssignCollector`].
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignCollectorPayload {
    mapper_addr: RawAddress,
    partition_id: u8,
}

#[bon]
impl AssignCollectorPayload {
    #[builder]
    pub fn new(mapper_addr: Address, partition_id: u8) -> Self {
        Self { mapper_addr: RawAddress::from(mapper_addr), partition_id }
    }

    pub fn mapper_addr(&self) -> Address { self.mapper_addr.parse() }
    pub fn partition_id(&self) -> u8 { self.partition_id }
}

/// Payload for [`OpCode::AssignMapper`].
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignMapperPayload {
    reducer_addr: RawAddress,
    collector_count: u8,
}

#[bon]
impl AssignMapperPayload {
    #[builder]
    pub fn new(reducer_addr: Address, collector_count: u8) -> Self {
        Self { reducer_addr: RawAddress::from(reducer_addr), collector_count }
    }

    pub fn reducer_addr(&self) -> Address { self.reducer_addr.parse() }
    pub fn collector_count(&self) -> u8 { self.collector_count }
}

/// Payload for [`OpCode::AssignReducer`].
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignReducerPayload {
    los_addr: RawAddress,
    mapper_count: u8,
}

#[bon]
impl AssignReducerPayload {
    #[builder]
    pub fn new(los_addr: Address, mapper_count: u8) -> Self {
        Self { los_addr: RawAddress::from(los_addr), mapper_count }
    }

    pub fn los_addr(&self) -> Address { self.los_addr.parse() }
    pub fn mapper_count(&self) -> u8 { self.mapper_count }
}

/// The role a satellite was assigned in a MapReduce job.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Role {
    Collector = 1,
    Mapper = 2,
    Reducer = 3,
}

impl TryFrom<u8> for Role {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Collector),
            2 => Ok(Self::Mapper),
            3 => Ok(Self::Reducer),
            _ => Err(()),
        }
    }
}

/// Payload for [`OpCode::PhaseDone`].
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct PhaseDonePayload {
    role: u8,
}

#[bon]
impl PhaseDonePayload {
    #[builder]
    pub fn new(role: Role) -> Self {
        Self { role: role as u8 }
    }

    pub fn role(&self) -> Result<Role, ()> {
        self.role.try_into()
    }

    pub fn set_role(&mut self, role: Role) {
        self.role = role as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerocopy::IntoBytes;

    #[test]
    fn test_message_roundtrip() {
        let payload_len = size_of::<AssignCollectorPayload>();
        let total = SpaceCompMessage::HEADER_SIZE + payload_len;
        let mut buf = [0u8; 32];

        let hdr = SpaceCompHeader::new(OpCode::AssignCollector, 0x1234);
        buf[..SpaceCompMessage::HEADER_SIZE].copy_from_slice(hdr.as_bytes());

        let assign = AssignCollectorPayload::builder()
            .mapper_addr(crate::network::isl::address::Address::satellite(1, 5))
            .partition_id(3)
            .build();
        buf[SpaceCompMessage::HEADER_SIZE..total].copy_from_slice(assign.as_bytes());

        let msg = SpaceCompMessage::parse(&buf[..total]).unwrap();
        assert_eq!(msg.op_code(), Ok(OpCode::AssignCollector));
        assert_eq!(msg.job_id(), 0x1234);

        let parsed_payload =
            AssignCollectorPayload::read_from_bytes(&msg.payload()[..payload_len]).unwrap();
        assert_eq!(parsed_payload.partition_id(), 3);
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
