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
        /// Filestore identifier for the file on the sending side.
        source_file_name: FileId,
        /// Filestore identifier for the file on the receiving side.
        destination_file_name: FileId,
        /// The remote entity to send the file to.
        destination_id: EntityId,
        /// Total size of the file being transferred in bytes.
        file_size: u64,
        /// Algorithm used to verify file integrity.
        checksum_type: ChecksumType,
    },
    /// A user request to prompt the receiver for a NAK or Keep Alive response.
    PromptRequest {
        /// Identifies the transaction to send the prompt for.
        transaction_id: TransactionId,
        /// Whether to prompt for a NAK or Keep Alive response.
        prompt_type: PromptType,
    },
    /// A user request to suspend a transaction.
    SuspendRequest {
        /// Identifies the transaction to suspend.
        transaction_id: TransactionId,
    },
    /// A user request to resume a suspended transaction.
    ResumeRequest {
        /// Identifies the transaction to resume.
        transaction_id: TransactionId,
    },
    /// A PDU has been received from a remote entity for a transaction this machine is handling.
    PduReceived {
        /// Identifies the transaction the received PDU belongs to.
        transaction_id: TransactionId,
        /// The received protocol data unit to process.
        pdu: &'a Pdu,
    },
    /// A timer, previously requested via an `Action`, has expired.
    TimerExpired {
        /// Identifies the transaction the expired timer belongs to.
        transaction_id: TransactionId,
        /// The kind of timer that expired.
        timer_type: TimerType,
    },
    /// A chunk of file data, requested via a `RequestFileData` action, is ready to be sent.
    DataSegmentReady {
        /// Identifies the transaction the data belongs to.
        transaction_id: TransactionId,
        /// The file data segment payload.
        data: &'a [u8],
        /// Byte offset within the file where this data begins.
        offset: u64,
    },
    /// The checksum calculation, requested via an `Action`, has completed.
    ChecksumReady {
        /// Identifies the transaction the checksum was computed for.
        transaction_id: TransactionId,
        /// The computed checksum value.
        checksum: u32,
    },
}
