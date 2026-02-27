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
    /// Ground station submits a new MapReduce job to the coordinator.
    SubmitJob = 0x00,
    /// Coordinator assigns the collector role to a satellite.
    AssignCollector = 0x01,
    /// Coordinator assigns the mapper role to a satellite.
    AssignMapper = 0x02,
    /// Coordinator assigns the reducer role to a satellite.
    AssignReducer = 0x03,
    /// A satellite reports that its processing phase is complete.
    PhaseDone = 0x04,
    /// The reducer sends the final result back to the coordinator.
    JobResult = 0x05,
    /// A data chunk transferred between processing phases.
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
    /// Size of the fixed SpaceCoMP header in bytes.
    pub const HEADER_SIZE: usize = size_of::<SpaceCompHeader>();

    /// Parses a SpaceCoMP message from a byte slice.
    pub fn parse(bytes: &[u8]) -> Result<&Self, ParseError> {
        Self::ref_from_bytes(bytes).map_err(|_| ParseError::Message)
    }

    /// Returns the operation code from the header.
    pub fn op_code(&self) -> Result<OpCode, ParseError> {
        self.header.op_code().map_err(|_| ParseError::InvalidOpCode)
    }

    /// Returns the job identifier from the header.
    pub fn job_id(&self) -> u16 {
        self.header.job_id()
    }

    /// Returns the variable-length payload following the header.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Returns the entire message (header + payload) as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        zerocopy::IntoBytes::as_bytes(self)
    }
}

#[bon]
impl SpaceCompMessage {
    #[builder]
    /// Constructs a new SpaceCoMP message in the provided buffer.
    pub fn new<'a>(
        buffer: &'a mut [u8],
        op_code: OpCode,
        job_id: u16,
        payload: &[u8],
    ) -> Result<&'a SpaceCompMessage, BuildError> {
        let (msg, _) = Self::mut_from_prefix_with_elems(buffer, payload.len())
            .map_err(|_| BuildError::BufferTooSmall)?;
        msg.header = SpaceCompHeader::new(op_code, job_id);
        msg.payload.copy_from_slice(payload);
        Ok(msg)
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
pub(crate) struct SpaceCompHeader {
    op_code: u8,
    _reserved: u8,
    job_id: U16,
}

impl SpaceCompHeader {
    pub(crate) fn new(op_code: OpCode, job_id: u16) -> Self {
        Self {
            op_code: op_code as u8,
            _reserved: 0,
            job_id: U16::new(job_id),
        }
    }

    pub(crate) fn op_code(&self) -> Result<OpCode, ()> {
        self.op_code.try_into()
    }

    pub(crate) fn set_op_code(&mut self, op_code: OpCode) {
        self.op_code = op_code as u8;
    }

    pub(crate) fn job_id(&self) -> u16 {
        self.job_id.get()
    }

    pub(crate) fn set_job_id(&mut self, job_id: u16) {
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
    /// Creates a new collector assignment payload.
    pub fn new(mapper_addr: Address, partition_id: u8) -> Self {
        Self { mapper_addr: RawAddress::from(mapper_addr), partition_id }
    }

    /// Returns the address of the mapper this collector should send data to.
    pub fn mapper_addr(&self) -> Address { self.mapper_addr.parse() }
    /// Returns the partition index assigned to this collector.
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
    /// Creates a new mapper assignment payload.
    pub fn new(reducer_addr: Address, collector_count: u8) -> Self {
        Self { reducer_addr: RawAddress::from(reducer_addr), collector_count }
    }

    /// Returns the address of the reducer this mapper should forward to.
    pub fn reducer_addr(&self) -> Address { self.reducer_addr.parse() }
    /// Returns the number of collectors feeding data to this mapper.
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
    /// Creates a new reducer assignment payload.
    pub fn new(los_addr: Address, mapper_count: u8) -> Self {
        Self { los_addr: RawAddress::from(los_addr), mapper_count }
    }

    /// Returns the line-of-sight node address for result delivery.
    pub fn los_addr(&self) -> Address { self.los_addr.parse() }
    /// Returns the number of mappers feeding data to this reducer.
    pub fn mapper_count(&self) -> u8 { self.mapper_count }
}

/// Errors that can occur when parsing a SpaceCoMP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ParseError {
    /// The byte slice is not a valid [`SpaceCompMessage`].
    #[error("invalid message")]
    Message,
    /// The opcode byte does not map to a known [`OpCode`].
    #[error("invalid opcode")]
    InvalidOpCode,
    /// The payload is not a valid [`AssignCollectorPayload`].
    #[error("invalid collector assignment")]
    AssignCollector,
    /// The payload is not a valid [`AssignMapperPayload`].
    #[error("invalid mapper assignment")]
    AssignMapper,
    /// The payload is not a valid [`AssignReducerPayload`].
    #[error("invalid reducer assignment")]
    AssignReducer,
}

/// Errors that can occur when parsing a SpaceCoMP message.
impl ParseError {
    /// Returns a static string describing the error.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Message => "invalid message",
            Self::InvalidOpCode => "invalid opcode",
            Self::AssignCollector => "invalid collector assignment",
            Self::AssignMapper => "invalid mapper assignment",
            Self::AssignReducer => "invalid reducer assignment",
        }
    }
}

/// Errors that can occur when constructing a SpaceCoMP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BuildError {
    /// A numeric value exceeded the valid `u8` range.
    #[error("value out of range")]
    OutOfRange,
    /// The provided buffer is too small for the message.
    #[error("buffer too small")]
    BufferTooSmall,
}

impl BuildError {
    /// Returns a static string describing the error.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OutOfRange => "value out of range",
            Self::BufferTooSmall => "buffer too small",
        }
    }
}

/// Decoded [`OpCode::AssignCollector`] message.
#[derive(Debug, Clone, Copy)]
pub struct AssignCollectorMessage {
    /// Job identifier.
    pub job_id: u16,
    /// Address of the mapper this collector sends data to.
    pub mapper_addr: Address,
    /// Partition index assigned to this collector.
    pub partition_id: u8,
}

#[bon]
impl AssignCollectorMessage {
    /// Decodes from a raw [`SpaceCompMessage`].
    pub fn parse(msg: &SpaceCompMessage) -> Result<Self, ParseError> {
        let (p, _) = AssignCollectorPayload::read_from_prefix(msg.payload())
            .map_err(|_| ParseError::AssignCollector)?;
        Ok(Self {
            job_id: msg.job_id(),
            mapper_addr: p.mapper_addr(),
            partition_id: p.partition_id(),
        })
    }

    /// Constructs a new collector assignment message in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        job_id: u16,
        mapper_addr: impl Into<Address>,
        partition_id: usize,
    ) -> Result<&'a SpaceCompMessage, BuildError> {
        let partition_id = u8::try_from(partition_id).map_err(|_| BuildError::OutOfRange)?;
        let payload = AssignCollectorPayload::builder()
            .mapper_addr(mapper_addr.into())
            .partition_id(partition_id)
            .build();
        Ok(SpaceCompMessage::builder()
            .buffer(buffer)
            .op_code(OpCode::AssignCollector)
            .job_id(job_id)
            .payload(payload.as_bytes())
            .build()?)
    }
}

/// Decoded [`OpCode::AssignMapper`] message.
#[derive(Debug, Clone, Copy)]
pub struct AssignMapperMessage {
    /// Job identifier.
    pub job_id: u16,
    /// Address of the reducer this mapper forwards to.
    pub reducer_addr: Address,
    /// Number of collectors feeding data to this mapper.
    pub collector_count: u8,
}

#[bon]
impl AssignMapperMessage {
    /// Decodes from a raw [`SpaceCompMessage`].
    pub fn parse(msg: &SpaceCompMessage) -> Result<Self, ParseError> {
        let (p, _) = AssignMapperPayload::read_from_prefix(msg.payload())
            .map_err(|_| ParseError::AssignMapper)?;
        Ok(Self {
            job_id: msg.job_id(),
            reducer_addr: p.reducer_addr(),
            collector_count: p.collector_count(),
        })
    }

    /// Constructs a new mapper assignment message in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        job_id: u16,
        reducer_addr: impl Into<Address>,
        collector_count: usize,
    ) -> Result<&'a SpaceCompMessage, BuildError> {
        let collector_count = u8::try_from(collector_count).map_err(|_| BuildError::OutOfRange)?;
        let payload = AssignMapperPayload::builder()
            .reducer_addr(reducer_addr.into())
            .collector_count(collector_count)
            .build();
        Ok(SpaceCompMessage::builder()
            .buffer(buffer)
            .op_code(OpCode::AssignMapper)
            .job_id(job_id)
            .payload(payload.as_bytes())
            .build()?)
    }
}

/// Decoded [`OpCode::AssignReducer`] message.
#[derive(Debug, Clone, Copy)]
pub struct AssignReducerMessage {
    /// Job identifier.
    pub job_id: u16,
    /// Line-of-sight node address for result delivery.
    pub los_addr: Address,
    /// Number of mappers feeding data to this reducer.
    pub mapper_count: u8,
}

#[bon]
impl AssignReducerMessage {
    /// Decodes from a raw [`SpaceCompMessage`].
    pub fn parse(msg: &SpaceCompMessage) -> Result<Self, ParseError> {
        let (p, _) = AssignReducerPayload::read_from_prefix(msg.payload())
            .map_err(|_| ParseError::AssignReducer)?;
        Ok(Self {
            job_id: msg.job_id(),
            los_addr: p.los_addr(),
            mapper_count: p.mapper_count(),
        })
    }

    /// Constructs a new reducer assignment message in the provided buffer.
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        job_id: u16,
        los_addr: impl Into<Address>,
        mapper_count: usize,
    ) -> Result<&'a SpaceCompMessage, BuildError> {
        let mapper_count = u8::try_from(mapper_count).map_err(|_| BuildError::OutOfRange)?;
        let payload = AssignReducerPayload::builder()
            .los_addr(los_addr.into())
            .mapper_count(mapper_count)
            .build();
        Ok(SpaceCompMessage::builder()
            .buffer(buffer)
            .op_code(OpCode::AssignReducer)
            .job_id(job_id)
            .payload(payload.as_bytes())
            .build()?)
    }
}

/// The role a satellite was assigned in a MapReduce job.
#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Role {
    /// Collects raw sensor data from the satellite's instruments.
    Collector = 1,
    /// Processes and transforms collected data.
    Mapper = 2,
    /// Aggregates mapped results into a final output.
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
    /// Creates a new phase-done payload for the given role.
    pub fn new(role: Role) -> Self {
        Self { role: role as u8 }
    }

    /// Returns the role that completed its phase.
    pub fn role(&self) -> Result<Role, ()> {
        self.role.try_into()
    }

    /// Sets the role that completed its phase.
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
