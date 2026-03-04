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
        /// The transaction this ACK belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send the ACK to.
        destination_id: EntityId,
        /// The directive being acknowledged.
        directive_code: DirectiveCode,
        /// The condition under which this ACK is sent.
        condition_code: ConditionCode,
        /// The current status of the transaction.
        transaction_status: TransactionStatus,
    },
    /// Instructs the `Runner` to serialize and send a Finished PDU.
    SendFinished {
        /// Responses from executed filestore requests.
        filestore_responses: Vec<FilestoreResponse, 4, u8>,
        /// The transaction this Finished PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send the Finished PDU to.
        destination_id: EntityId,
        /// The condition under which the transaction finished.
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to serialize and send a NAK PDU.
    SendNak {
        /// The transaction this NAK belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send the NAK to.
        destination_id: EntityId,
        /// The beginning of the file scope covered by this NAK.
        start_of_scope: u64,
        /// The end of the file scope covered by this NAK.
        end_of_scope: u64,
        // segment_requests: Vec<(U64, U64), 32>,
    },
    /// Instructs the `Runner` to serialize and send a Keep Alive PDU.
    SendKeepAlive {
        /// The transaction this Keep Alive belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send the Keep Alive to.
        destination_id: EntityId,
        /// The number of file data bytes received so far.
        progress: u64,
    },
    /// Instructs the `Runner` to write a chunk of data to the filestore.
    WriteFileData {
        /// The transaction this file data belongs to.
        transaction_id: TransactionId,
        /// The raw file data bytes to write.
        data: &'a [u8],
        /// The byte offset within the file where the data should be written.
        offset: u64,
    },
    /// Instructs the `Runner` to start a timer for a specific transaction.
    StartTimer {
        /// The transaction this timer is associated with.
        transaction_id: TransactionId,
        /// The duration of the timer in seconds.
        seconds: u16,
        /// The kind of timer to start (e.g. ACK, NAK, inactivity).
        timer_type: TimerType,
    },
    /// Instructs the `Runner` to stop a timer for a specific transaction.
    StopTimer {
        /// The transaction whose timer should be stopped.
        transaction_id: TransactionId,
        /// The type of timer to stop. If `None`, stops all timers for the transaction.
        timer_type: Option<TimerType>,
    },
    /// Instructs the `Runner` to notify the user that a transaction has finished.
    TerminateTransaction {
        /// The transaction being terminated.
        transaction_id: TransactionId,
        /// The condition that caused the termination.
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
        /// The transaction whose file checksum should be verified.
        transaction_id: TransactionId,
        /// The checksum value declared by the sender in the EOF PDU.
        expected_checksum: u32,
        /// The algorithm used to compute the checksum.
        checksum_type: ChecksumType,
    },
    /// Instructs the `Runner` to notify the user that a fault has occurred.
    NotifyFault {
        /// The transaction in which the fault occurred.
        transaction_id: TransactionId,
        /// The condition code describing the fault.
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to notify the user that a transaction was suspended.
    NotifySuspended {
        /// The transaction that was suspended.
        transaction_id: TransactionId,
    },
    /// Instructs the `Runner` to notify the user that a transaction was resumed.
    NotifyResumed {
        /// The transaction that was resumed.
        transaction_id: TransactionId,
        /// The number of file data bytes received at the time of resumption.
        progress: u64,
    },
    /// Instructs the Runner to execute filestore requests and report back with the results.
    ExecuteFilestoreRequests {
        /// The filestore requests received in the Metadata PDU.
        requests: Vec<FilestoreRequest, 4, u8>,
        /// The transaction these requests belong to.
        transaction_id: TransactionId,
    },
}

/// A bounded collection of actions produced by processing a single event.
pub struct Actions<'a> {
    /// The bounded buffer of actions.
    actions: Vec<Action<'a>, MAX_ACTIONS_PER_EVENT, u8>,
}

impl<'a> Actions<'a> {
    /// Creates an empty `Actions` collection.
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

    /// Removes all actions from the collection.
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
