//! Contains the shared data structures for CFDP transactions, used by both
//! the sender and receiver state machines.

use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::metadata::ChecksumType;
use crate::transport::cfdp::pdu::tlv::fault_handler_override::FaultHandlerSet;

/// A unique identifier for a single CFDP transaction.
///
/// It is composed of the source entity's ID and a sequence number that is unique
/// for that source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct TransactionId {
    pub source_id: EntityId,
    pub seq_num: TransactionSeqNum,
}

/// The static configuration for a single transaction that is shared between
/// both sender and receiver.
///
/// This information is typically derived from the initial `PutRequest` (for the sender)
/// or the received `Metadata` PDU (for the receiver).
#[derive(Debug)]
pub struct TransactionConfig {
    /// The unique identifier for this transaction.
    pub transaction_id: TransactionId,
    /// The EntityId of the destination for this transaction.
    pub destination_id: EntityId,
    /// The total size of the file in bytes.
    pub file_size: u64,
    /// Optional fault handler overrides, one for each possible Condition Code.
    /// If an entry is `None`, the default MIB handler is used.
    pub fault_handlers: FaultHandlerSet,
    /// Timeout in seconds to wait for any PDU before declaring the transaction inactive.
    pub inactivity_timeout_secs: u16,
    /// The type of checksum to use for data integrity verification.
    pub checksum_type: ChecksumType,
    /// The name of the file at the source entity.
    pub source_file_id: FileId,
    /// The name the file should have at the destination entity.
    pub destination_file_id: FileId,
}
