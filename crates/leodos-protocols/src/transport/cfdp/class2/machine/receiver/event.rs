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
        transaction_id: TransactionId,
        pdu: &'a Pdu,
    },
    /// A timer, previously requested via an `Action`, has expired.
    TimerExpired {
        transaction_id: TransactionId,
        timer_type: TimerType,
    },
    /// A chunk of file data, requested to be written via a `WriteFileData` action, has been successfully written.
    FileDataWritten {
        transaction_id: TransactionId,
        offset: u64,
        len: usize,
    },
    ChecksumVerified {
        transaction_id: TransactionId,
        is_valid: bool,
    },
    SuspendRequest {
        transaction_id: TransactionId,
    },
    ResumeRequest {
        transaction_id: TransactionId,
    },
    FilestoreResponsesReceived {
        transaction_id: TransactionId,
        responses: Vec<FilestoreResponse, 4, u8>,
    },
}
