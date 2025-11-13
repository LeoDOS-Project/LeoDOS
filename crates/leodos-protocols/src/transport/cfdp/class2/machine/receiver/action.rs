use heapless::Vec;

use crate::transport::cfdp::class2::machine::TimerType;
use crate::transport::cfdp::class2::machine::TransactionId;
use crate::transport::cfdp::class2::machine::MAX_ACTIONS_PER_EVENT;
use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::file_directive::ack::TransactionStatus;
use crate::transport::cfdp::pdu::file_directive::metadata::ChecksumType;
use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::tlv::filestore_request::FilestoreRequest;
use crate::transport::cfdp::pdu::tlv::filestore_response::FilestoreResponse;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::CfdpError;

/// Represents all possible outputs from the `ReceiverMachine`.
///
/// These are instructions for the `Runner` to execute.
#[derive(Debug, PartialEq, Eq)]
pub enum Action<'a> {
    /// Instructs the `Runner` to serialize and send an ACK PDU.
    SendAck {
        transaction_id: TransactionId,
        destination_id: EntityId,
        directive_code: DirectiveCode,
        condition_code: ConditionCode,
        transaction_status: TransactionStatus,
    },
    /// Instructs the `Runner` to serialize and send a Finished PDU.
    SendFinished {
        filestore_responses: Vec<FilestoreResponse, 4, u8>,
        transaction_id: TransactionId,
        destination_id: EntityId,
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to serialize and send a NAK PDU.
    SendNak {
        transaction_id: TransactionId,
        destination_id: EntityId,
        start_of_scope: u64,
        end_of_scope: u64,
        // segment_requests: Vec<(U64, U64), 32>,
    },
    ///
    SendKeepAlive {
        transaction_id: TransactionId,
        destination_id: EntityId,
        progress: u64,
    },
    /// Instructs the `Runner` to write a chunk of data to the filestore.
    WriteFileData {
        transaction_id: TransactionId,
        data: &'a [u8],
        offset: u64,
    },
    /// Instructs the `Runner` to start a timer for a specific transaction.
    StartTimer {
        transaction_id: TransactionId,
        seconds: u16,
        timer_type: TimerType,
    },
    /// Instructs the `Runner` to stop a timer for a specific transaction.
    StopTimer {
        transaction_id: TransactionId,
        /// The type of timer to stop. If `None`, stops all timers for the transaction.
        timer_type: Option<TimerType>,
    },
    /// Instructs the `Runner` to notify the user that a transaction has finished.
    TerminateTransaction {
        transaction_id: TransactionId,
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to notify the user that a file has been successfully received.
    NotifyFileReceived {
        /// The unique ID of the completed transaction.
        transaction_id: TransactionId,
        /// The total size of the received file in bytes.
        file_size: u64,
        /// The final name of the received file.
        file_name: FileId,
    },
    /// Instructs the `Runner` to verify the checksum of the received file.
    VerifyChecksum {
        transaction_id: TransactionId,
        expected_checksum: u32,
        checksum_type: ChecksumType,
    },
    /// Instructs the `Runner` to notify the user that a fault has occurred.
    NotifyFault {
        transaction_id: TransactionId,
        condition_code: ConditionCode,
    },
    NotifySuspended {
        transaction_id: TransactionId,
    },
    NotifyResumed {
        transaction_id: TransactionId,
        progress: u64,
    },
    /// Instructs the Runner to execute filestore requests and report back with the results.
    ExecuteFilestoreRequests {
        // The raw bytes of all FilestoreRequest TLVs received in the metadata.
        requests: Vec<FilestoreRequest, 4, u8>,
        transaction_id: TransactionId,
    },
}

pub struct Actions<'a> {
    actions: Vec<Action<'a>, MAX_ACTIONS_PER_EVENT, u8>,
}

impl<'a> Actions<'a> {
    pub fn new() -> Self {
        Actions {
            actions: Vec::new(),
        }
    }

    /// Adds an action to the collection.
    pub fn push(&mut self, action: Action<'a>) -> Result<(), CfdpError> {
        self.actions
            .push(action)
            .map_err(|_| CfdpError::ActionBufferFull)
    }

    /// Returns an iterator over the actions.
    pub fn iter(&self) -> impl Iterator<Item = &Action<'a>> {
        self.actions.iter()
    }

    pub fn clear(&mut self) {
        self.actions.clear();
    }
}

impl<'a> Iterator for Actions<'a> {
    type Item = Action<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.actions.pop()
    }
}
