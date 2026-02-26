//! SpaceCoMP command packet format.
//!
//! Used by the coordinator to assign roles (collector, mapper, reducer)
//! to satellites, and by satellites to report phase completion.
//!
//! Built on SRSPP transport packets, following the same layering as
//! [`SrsppDataPacket`](crate::transport::srspp::packet::SrsppDataPacket).

use bon::bon;
use core::mem::size_of;
use core::ops::Deref;
use core::ops::DerefMut;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;
use zerocopy::Unaligned;
use zerocopy::network_endian::U16;

use crate::network::cfe::tc::TelecommandSecondaryHeader;
use crate::network::isl::address::Address;
use crate::network::isl::address::RawAddress;
use crate::network::isl::routing::packet::IslRoutingTelecommandHeader;
use crate::network::spp::Apid;
use crate::network::spp::PacketType;
use crate::network::spp::PacketVersion;
use crate::network::spp::PrimaryHeader;
use crate::network::spp::SecondaryHeaderFlag;
use crate::network::spp::SequenceCount;
use crate::network::spp::SequenceFlag;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppHeader;
use crate::transport::srspp::packet::SrsppPacketError;
use crate::transport::srspp::packet::SrsppType;
use crate::utils::checksum_u8;
use crate::utils::validate_checksum_u8;

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
///      ├─ AssignCollector ────────►│                │               │
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
    /// Ground or coordinator initiates a new MapReduce job.
    SubmitJob = 0x00,
    /// Assign a satellite the collector role for one data partition.
    AssignCollector = 0x01,
    /// Assign a satellite the mapper role for a set of collectors.
    AssignMapper = 0x02,
    /// Assign a satellite the reducer role for all mappers.
    AssignReducer = 0x03,
    /// A satellite reports that its phase is complete.
    PhaseDone = 0x04,
    /// Reducer sends aggregated results back to the coordinator.
    JobResult = 0x05,
    /// Raw or intermediate data chunk between pipeline stages.
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

/// A zero-copy view over a SpaceCoMP packet in a raw byte buffer.
///
/// This is the most specific view of an on-wire SpaceCoMP message,
/// combining all protocol layers. Derefs to [`SrsppDataPacket`] for
/// access to lower-layer fields.
///
/// ```text
/// +------------------------------------+---------+
/// | Field Name                         | Size    |
/// +------------------------------------+---------+
/// | -- SPP Primary Header ------------ | ------- |
/// | (see SpacePacket)                  | 6 bytes |
/// | -- CFE TC Secondary Header ------- | ------- |
/// | (see Telecommand)                  | 2 bytes |
/// | -- ISL Routing Header ------------ | ------- |
/// | (see IslRoutingTelecommandHeader)  | 4 bytes |
/// | -- SRSPP Header ------------------ | ------- |
/// | (see SrsppHeader)                  | 3 bytes |
/// | -- SpaceCoMP Header -------------- | ------- |
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
pub struct SpaceCompPacket {
    primary: PrimaryHeader,
    secondary: TelecommandSecondaryHeader,
    isl_header: IslRoutingTelecommandHeader,
    srspp_header: SrsppHeader,
    header: SpaceCompHeader,
    payload: [u8],
}

/// The 4-byte SpaceCoMP header present in every SpaceCoMP message,
/// immediately following the CFE telecommand secondary header.
///
/// This struct is a zero-copy view and provides methods to safely
/// access its fields.
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
    /// The [`OpCode`] identifying the message type.
    op_code: u8,
    _reserved: u8,
    /// Network-endian job identifier, unique per MapReduce job.
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
///
/// Tells a satellite to collect its data partition and forward the
/// resulting [`OpCode::DataChunk`] messages to the given mapper.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignCollectorPayload {
    /// ISL address of the mapper that will receive this collector's data.
    mapper_addr: RawAddress,
    /// Zero-based partition index assigned to this collector.
    partition_id: u8,
}

#[bon]
impl AssignCollectorPayload {
    #[builder]
    pub fn new(mapper_addr: RawAddress, partition_id: u8) -> Self {
        Self { mapper_addr, partition_id }
    }

    pub fn mapper_addr(&self) -> RawAddress { self.mapper_addr }
    pub fn partition_id(&self) -> u8 { self.partition_id }
}

/// Payload for [`OpCode::AssignMapper`].
///
/// Tells a satellite to accumulate [`OpCode::DataChunk`] messages from
/// `collector_count` collectors, run the map function, and forward
/// intermediate results to the reducer.
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignMapperPayload {
    /// ISL address of the reducer that will receive map output.
    reducer_addr: RawAddress,
    /// Number of collectors that will send data to this mapper.
    collector_count: u8,
}

#[bon]
impl AssignMapperPayload {
    #[builder]
    pub fn new(reducer_addr: RawAddress, collector_count: u8) -> Self {
        Self { reducer_addr, collector_count }
    }

    pub fn reducer_addr(&self) -> RawAddress { self.reducer_addr }
    pub fn collector_count(&self) -> u8 { self.collector_count }
}

/// Payload for [`OpCode::AssignReducer`].
///
/// Tells a satellite to accumulate intermediate results from
/// `mapper_count` mappers, run the reduce function, and send
/// the final [`OpCode::JobResult`] to the coordinator (LOS address).
#[repr(C)]
#[derive(Debug, FromBytes, IntoBytes, Unaligned, KnownLayout, Immutable, Clone, Copy)]
pub struct AssignReducerPayload {
    /// ISL address of the coordinator (line-of-sight to ground).
    los_addr: RawAddress,
    /// Number of mappers that will send results to this reducer.
    mapper_count: u8,
}

#[bon]
impl AssignReducerPayload {
    #[builder]
    pub fn new(los_addr: RawAddress, mapper_count: u8) -> Self {
        Self { los_addr, mapper_count }
    }

    pub fn los_addr(&self) -> RawAddress { self.los_addr }
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
    /// The [`Role`] that completed.
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

impl Deref for SpaceCompPacket {
    type Target = SrsppDataPacket;
    fn deref(&self) -> &Self::Target {
        SrsppDataPacket::ref_from_bytes(self.as_bytes())
            .expect("SpaceCompPacket layout is a superset of SrsppDataPacket")
    }
}

impl DerefMut for SpaceCompPacket {
    fn deref_mut(&mut self) -> &mut Self::Target {
        SrsppDataPacket::mut_from_bytes(self.as_mut_bytes())
            .expect("SpaceCompPacket layout is a superset of SrsppDataPacket")
    }
}

impl SpaceCompPacket {
    pub const HEADER_SIZE: usize = SrsppDataPacket::HEADER_SIZE + size_of::<SpaceCompHeader>();

    pub fn header(&self) -> &SpaceCompHeader {
        &self.header
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut [u8] {
        &mut self.payload
    }

    pub fn set_cfe_checksum(&mut self) {
        self.secondary.set_checksum(0);
        self.secondary.set_checksum(checksum_u8(self.as_bytes()));
    }

    pub fn validate_cfe_checksum(&self) -> bool {
        validate_checksum_u8(self.as_bytes())
    }

    pub fn parse(bytes: &[u8]) -> Result<&Self, SrsppPacketError> {
        Self::ref_from_bytes(bytes).map_err(|_| SrsppPacketError::BufferTooSmall {
            required: Self::HEADER_SIZE,
            provided: bytes.len(),
        })
    }
}

#[bon]
impl SpaceCompPacket {
    #[builder]
    pub fn new<'a>(
        buffer: &'a mut [u8],
        source_address: Address,
        target: Address,
        apid: Apid,
        function_code: u8,
        message_id: u8,
        action_code: u8,
        sequence_count: SequenceCount,
        sequence_flag: SequenceFlag,
        op_code: OpCode,
        job_id: u16,
        payload_len: usize,
    ) -> Result<&'a mut Self, SrsppPacketError> {
        let required_len = Self::HEADER_SIZE + payload_len;
        let provided_len = buffer.len();

        let (packet, _) =
            Self::mut_from_prefix_with_elems(buffer, payload_len).map_err(|_| {
                SrsppPacketError::BufferTooSmall {
                    required: required_len,
                    provided: provided_len,
                }
            })?;

        packet.primary.set_version(PacketVersion::VERSION_1);
        packet.primary.set_packet_type(PacketType::Telecommand);
        packet
            .primary
            .set_secondary_header_flag(SecondaryHeaderFlag::Present);
        packet.primary.set_apid(apid);
        packet.primary.set_sequence_count(sequence_count);
        packet.primary.set_sequence_flag(sequence_flag);

        let data_field_len = size_of::<TelecommandSecondaryHeader>()
            + size_of::<IslRoutingTelecommandHeader>()
            + size_of::<SrsppHeader>()
            + size_of::<SpaceCompHeader>()
            + payload_len;
        packet.primary.set_data_field_len(data_field_len as u16);

        packet.secondary.set_function_code(function_code);
        packet.secondary.set_checksum(0);

        packet.isl_header.set_target(target);
        packet.isl_header.set_message_id(message_id);
        packet.isl_header.set_action_code(action_code);

        packet.srspp_header.set_source_address(source_address);
        packet.srspp_header.set_srspp_type(SrsppType::Data);

        packet.header = SpaceCompHeader::new(op_code, job_id);

        packet.set_cfe_checksum();

        Ok(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Address {
        Address::satellite(0, 1)
    }

    fn target() -> Address {
        Address::satellite(1, 5)
    }

    #[test]
    fn test_roundtrip() {
        let mut buf = [0u8; 128];
        let payload_len = size_of::<AssignCollectorPayload>();

        let pkt = SpaceCompPacket::builder()
            .buffer(&mut buf)
            .source_address(source())
            .target(target())
            .apid(Apid::new(42).unwrap())
            .function_code(0)
            .message_id(0)
            .action_code(0)
            .sequence_count(SequenceCount::from(1))
            .sequence_flag(SequenceFlag::Unsegmented)
            .op_code(OpCode::AssignCollector)
            .job_id(0x1234)
            .payload_len(payload_len)
            .build()
            .unwrap();

        let assign = AssignCollectorPayload::builder()
            .mapper_addr(RawAddress::from(target()))
            .partition_id(3)
            .build();
        pkt.payload_mut()[..payload_len].copy_from_slice(assign.as_bytes());
        pkt.set_cfe_checksum();

        let parsed = SpaceCompPacket::parse(pkt.as_bytes()).unwrap();
        assert_eq!(parsed.header().op_code(), Ok(OpCode::AssignCollector));
        assert_eq!(parsed.header().job_id(), 0x1234);
        assert!(parsed.validate_cfe_checksum());

        let parsed_payload =
            AssignCollectorPayload::read_from_bytes(&parsed.payload()[..payload_len]).unwrap();
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

    #[test]
    fn test_deref_to_srspp() {
        let mut buf = [0u8; 128];

        let pkt = SpaceCompPacket::builder()
            .buffer(&mut buf)
            .source_address(source())
            .target(target())
            .apid(Apid::new(42).unwrap())
            .function_code(0)
            .message_id(0)
            .action_code(0)
            .sequence_count(SequenceCount::from(5))
            .sequence_flag(SequenceFlag::Unsegmented)
            .op_code(OpCode::DataChunk)
            .job_id(1)
            .payload_len(4)
            .build()
            .unwrap();

        let srspp: &SrsppDataPacket = pkt;
        assert_eq!(srspp.primary.apid(), Apid::new(42).unwrap());
        assert_eq!(srspp.srspp_header.source_address(), source());
        assert_eq!(srspp.isl_header.target(), target());
    }
}
