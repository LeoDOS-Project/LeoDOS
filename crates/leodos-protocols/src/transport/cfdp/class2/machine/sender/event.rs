use crate::transport::cfdp::class2::machine::PromptType;
use crate::transport::cfdp::class2::machine::TimerType;
use crate::transport::cfdp::class2::machine::TransactionId;
use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::file_directive::metadata::ChecksumType;

/// Represents all possible inputs that can drive the `SenderMachine`.
#[derive(Debug)]
pub enum Event<'a> {
    /// A user request to send a file to a remote entity.
    PutRequest {
        source_file_name: FileId,
        destination_file_name: FileId,
        destination_id: EntityId,
        file_size: u64,
        checksum_type: ChecksumType,
    },
    PromptRequest {
        transaction_id: TransactionId,
        prompt_type: PromptType,
    },
    SuspendRequest {
        transaction_id: TransactionId,
    },
    ResumeRequest {
        transaction_id: TransactionId,
    },
    /// A PDU has been received from a remote entity for a transaction this machine is handling.
    PduReceived {
        transaction_id: TransactionId,
        pdu: &'a Pdu,
    },
    /// A timer, previously requested via an `Action`, has expired.
    TimerExpired {
        transaction_id: TransactionId,
        timer_type: TimerType,
    },
    /// A chunk of file data, requested via a `RequestFileData` action, is ready to be sent.
    DataSegmentReady {
        transaction_id: TransactionId,
        data: &'a [u8],
        offset: u64,
    },
    ChecksumReady {
        transaction_id: TransactionId,
        checksum: u32,
    },
}
