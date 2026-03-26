//! SpaceCoMP message format.
//!
//! Used by the coordinator to assign roles (collector, mapper, reducer)
//! to satellites, and by satellites to report phase completion.

use bon::bon;
use core::mem::size_of;

use zerocopy::network_endian::U16;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;

use leodos_protocols::network::isl::address::Address;
use leodos_protocols::network::isl::address::RawAddress;

/// Payload for [`OpCode::SubmitJob`].
pub type SubmitJobPayload = crate::job::Job;

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

const fn const_max(sizes: &[usize]) -> usize {
    let mut max = 0;
    let mut i = 0;
    while i < sizes.len() {
        if sizes[i] > max {
            max = sizes[i];
        }
        i += 1;
    }
    max
}

impl SpaceCompMessage {
    /// Size of the fixed SpaceCoMP header in bytes.
    pub const HEADER_SIZE: usize = size_of::<SpaceCompHeader>();

    /// Max message size for dispatch (header + largest receivable payload).
    pub const MAX_DISPATCH_SIZE: usize = Self::HEADER_SIZE
        + const_max(&[
            size_of::<SubmitJobPayload>(),
            size_of::<AssignCollectorPayload>(),
            size_of::<AssignMapperPayload>(),
            size_of::<AssignReducerPayload>(),
        ]);

    /// Max message size for assignment commands (header + largest assignment payload).
    pub const MAX_ASSIGN_SIZE: usize = Self::HEADER_SIZE
        + const_max(&[
            size_of::<AssignCollectorPayload>(),
            size_of::<AssignMapperPayload>(),
            size_of::<AssignReducerPayload>(),
        ]);

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

    /// Returns a mutable reference to the payload.
    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.payload
    }

    /// Returns the entire message (header + payload) as a byte slice.
    pub fn as_bytes(&self) -> &[u8] {
        zerocopy::IntoBytes::as_bytes(self)
    }

    /// Parses a fixed-size payload from the message.
    pub fn parse_payload<T: FromBytes + KnownLayout + Immutable>(
        &self,
        err: ParseError,
    ) -> Result<T, ParseError> {
        let (val, _) = T::read_from_prefix(&self.payload).map_err(|_| err)?;
        Ok(val)
    }

    /// Iterates over fixed-size `T` records in the payload.
    pub fn records<'a, T: FromBytes + Immutable + KnownLayout + 'a>(
        &'a self,
    ) -> crate::reader::RecordIter<'a, T> {
        crate::reader::RecordIter::new(&self.payload)
    }

    /// Returns a reference to the message header.
    pub fn header(&self) -> &SpaceCompHeader {
        &self.header
    }
}

#[bon]
impl SpaceCompMessage {
    #[builder]
    /// Constructs a new SpaceCoMP message in the provided buffer.
    ///
    /// Returns a mutable reference so the caller can write directly
    /// into the payload, avoiding an extra copy.
    pub fn new<'a>(
        buffer: &'a mut [u8],
        op_code: OpCode,
        job_id: u16,
        payload_len: usize,
    ) -> Result<&'a mut SpaceCompMessage, BuildError> {
        let (msg, _) = Self::mut_from_prefix_with_elems(buffer, payload_len)
            .map_err(|_| BuildError::BufferTooSmall)?;
        msg.header = SpaceCompHeader::new(op_code, job_id);
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
pub struct SpaceCompHeader {
    op_code: u8,
    _reserved: u8,
    job_id: U16,
}

impl SpaceCompHeader {
    /// Creates a new SpaceCoMP header with the given opcode and job ID.
    pub fn new(op_code: OpCode, job_id: u16) -> Self {
        Self {
            op_code: op_code as u8,
            _reserved: 0,
            job_id: U16::new(job_id),
        }
    }

    /// Returns the operation code from the header.
    pub fn op_code(&self) -> Result<OpCode, ()> {
        self.op_code.try_into()
    }

    /// Sets the operation code in the header.
    pub fn set_op_code(&mut self, op_code: OpCode) {
        self.op_code = op_code as u8;
    }

    /// Returns the job identifier from the header.
    pub fn job_id(&self) -> u16 {
        self.job_id.get()
    }

    /// Sets the job identifier in the header.
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
    /// Creates a new collector assignment payload.
    pub fn new(mapper_addr: impl Into<Address>, partition_id: u8) -> Self {
        Self {
            mapper_addr: RawAddress::from(mapper_addr.into()),
            partition_id,
        }
    }

    /// Returns the address of the mapper this collector should send data to.
    pub fn mapper_addr(&self) -> Address {
        self.mapper_addr.parse()
    }
    /// Returns the partition index assigned to this collector.
    pub fn partition_id(&self) -> u8 {
        self.partition_id
    }
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
    pub fn new(reducer_addr: impl Into<Address>, collector_count: u8) -> Self {
        Self {
            reducer_addr: RawAddress::from(reducer_addr.into()),
            collector_count,
        }
    }

    /// Returns the address of the reducer this mapper should forward to.
    pub fn reducer_addr(&self) -> Address {
        self.reducer_addr.parse()
    }
    /// Returns the number of collectors feeding data to this mapper.
    pub fn collector_count(&self) -> u8 {
        self.collector_count
    }
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
    pub fn new(los_addr: impl Into<Address>, mapper_count: u8) -> Self {
        Self {
            los_addr: RawAddress::from(los_addr.into()),
            mapper_count,
        }
    }

    /// Returns the line-of-sight node address for result delivery.
    pub fn los_addr(&self) -> Address {
        self.los_addr.parse()
    }
    /// Returns the number of mappers feeding data to this reducer.
    pub fn mapper_count(&self) -> u8 {
        self.mapper_count
    }
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
    /// The payload is not a valid [`Job`].
    #[error("invalid submit job")]
    SubmitJob,
    /// The payload is not a valid [`AssignCollectorPayload`].
    #[error("invalid collector assignment")]
    AssignCollector,
    /// The payload is not a valid [`AssignMapperPayload`].
    #[error("invalid mapper assignment")]
    AssignMapper,
    /// The payload is not a valid [`AssignReducerPayload`].
    #[error("invalid reducer assignment")]
    AssignReducer,
    /// The message was valid but not the expected type.
    #[error("unexpected message")]
    UnexpectedMessage,
}

/// Errors that can occur when parsing a SpaceCoMP message.
impl ParseError {
    /// Returns a static string describing the error.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Message => "invalid message",
            Self::InvalidOpCode => "invalid opcode",
            Self::SubmitJob => "invalid submit job",
            Self::AssignCollector => "invalid collector assignment",
            Self::AssignMapper => "invalid mapper assignment",
            Self::AssignReducer => "invalid reducer assignment",
            Self::UnexpectedMessage => "unexpected message",
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
            .mapper_addr(Address::satellite(1, 5))
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
