use crate::transport::cfdp::class2::machine::TimerType;
use crate::transport::cfdp::class2::machine::TransactionId;
use crate::transport::cfdp::pdu::tlv::filestore_response::FilestoreResponse;
use crate::transport::cfdp::pdu::Pdu;
use heapless::Vec;

/// Represents all possible inputs that can drive the `ReceiverMachine`.
#[derive(Debug)]
pub enum Event<'a> {
    /// A PDU has been received from a remote entity for a transaction this machine is handling.
    PduReceived {
        /// The transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The received PDU to be processed.
        pdu: &'a Pdu,
    },
    /// A timer, previously requested via an `Action`, has expired.
    TimerExpired {
        /// The transaction whose timer expired.
        transaction_id: TransactionId,
        /// The kind of timer that expired.
        timer_type: TimerType,
    },
    /// A chunk of file data, requested to be written via a `WriteFileData` action, has been successfully written.
    FileDataWritten {
        /// The transaction this write confirmation belongs to.
        transaction_id: TransactionId,
        /// The byte offset within the file where the data was written.
        offset: u64,
        /// The number of bytes that were written.
        len: usize,
    },
    /// The checksum verification, requested via an `Action`, has completed.
    ChecksumVerified {
        /// The transaction whose checksum was verified.
        transaction_id: TransactionId,
        /// Whether the computed checksum matched the expected value.
        is_valid: bool,
    },
    /// A user request to suspend a transaction.
    SuspendRequest {
        /// The transaction to suspend.
        transaction_id: TransactionId,
    },
    /// A user request to resume a suspended transaction.
    ResumeRequest {
        /// The transaction to resume.
        transaction_id: TransactionId,
    },
    /// The filestore requests have been executed and responses are ready.
    FilestoreResponsesReceived {
        /// The transaction these responses belong to.
        transaction_id: TransactionId,
        /// The results of the executed filestore requests.
        responses: Vec<FilestoreResponse, 4, u8>,
    },
}
