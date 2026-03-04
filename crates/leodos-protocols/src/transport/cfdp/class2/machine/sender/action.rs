use heapless::Vec;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::class2::machine::MAX_ACTIONS_PER_EVENT;
use crate::transport::cfdp::class2::machine::PromptType;
use crate::transport::cfdp::class2::machine::TimerType;
use crate::transport::cfdp::class2::machine::TransactionId;
use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::pdu::file_directive::ack::AckedDirectiveCode;
use crate::transport::cfdp::pdu::file_directive::ack::TransactionStatus;
use crate::transport::cfdp::pdu::file_directive::metadata::ChecksumType;
use crate::transport::cfdp::pdu::file_directive::nak::NakSegmentsIterator;

/// Represents all possible outputs from the `SenderMachine`.
///
/// These are instructions for the `Runner` to execute, such as sending PDUs,
/// managing timers, or interacting with the filestore.
#[derive(Debug)]
pub enum Action<'a> {
    /// Instructs the `Runner` to serialize and send a Metadata PDU.
    SendMetadata {
        /// Identifies the transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send this PDU to.
        destination_id: EntityId,
        /// Total size of the file being transferred in bytes.
        file_size: u64,
        /// Filestore identifier for the file on the sending side.
        source_file_name: FileId,
        /// Filestore identifier for the file on the receiving side.
        destination_file_name: FileId,
        /// Algorithm used to verify file integrity.
        checksum_type: ChecksumType,
    },
    /// Instructs the `Runner` to serialize and send a File Data PDU.
    SendFileData {
        /// Identifies the transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send this PDU to.
        destination_id: EntityId,
        /// Byte offset within the file where this data begins.
        offset: u64,
        /// The file data segment payload.
        data: &'a [u8],
    },
    /// Instructs the `Runner` to serialize and send an EOF PDU.
    SendEof {
        /// Identifies the transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send this PDU to.
        destination_id: EntityId,
        /// Status condition at the time the EOF is sent.
        condition_code: ConditionCode,
        /// Total size of the file in bytes.
        file_size: u64,
        /// Computed checksum of the complete file data.
        checksum: u32,
    },
    /// Instructs the `Runner` to serialize and send a Finished PDU.
    SendFinished {
        /// Identifies the transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send this PDU to.
        destination_id: EntityId,
        /// Final status condition of the transaction.
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to serialize and send an ACK PDU.
    SendAck {
        /// Identifies the transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send this PDU to.
        destination_id: EntityId,
        /// The directive code being acknowledged.
        acked_directive_code: AckedDirectiveCode,
        /// Status condition associated with the acknowledgment.
        condition_code: ConditionCode,
        /// Current status of the transaction being acknowledged.
        transaction_status: TransactionStatus,
    },
    /// Instructs the `Runner` to serialize and send a Prompt PDU.
    SendPrompt {
        /// Identifies the transaction this PDU belongs to.
        transaction_id: TransactionId,
        /// The remote entity to send this PDU to.
        destination_id: EntityId,
        /// Whether to prompt for a NAK or Keep Alive response.
        prompt_type: PromptType,
    },
    /// Instructs the `Runner` to read a segment of data from the filestore.
    ReadDataSegment {
        /// Identifies the transaction this read belongs to.
        transaction_id: TransactionId,
        /// Byte offset where the segment begins.
        start_offset: u64,
        /// Byte offset where the segment ends (exclusive).
        end_offset: u64,
    },
    /// Instructs the `Runner` to read multiple segments of data from the filestore.
    ReadDataSegmentBatch {
        /// Identifies the transaction this batch read belongs to.
        transaction_id: TransactionId,
        /// Iterator over the missing segments requested by the receiver.
        segments: NakSegmentsIterator<'a>,
    },
    /// Instructs the `Runner` to start a timer for a specific transaction.
    StartTimer {
        /// Identifies the transaction this timer belongs to.
        transaction_id: TransactionId,
        /// The kind of timer to start (e.g. Ack, Nak, Inactivity).
        timer_type: TimerType,
        /// Duration of the timer in seconds.
        seconds: u16,
    },
    /// Instructs the `Runner` to stop a timer for a specific transaction.
    StopTimer {
        /// Identifies the transaction this timer belongs to.
        transaction_id: TransactionId,
        /// The type of timer to stop. If `None`, stops all timers for the transaction.
        timer_type: Option<TimerType>,
    },
    /// Instructs the `Runner` that a transaction is completed and can be cleaned up.
    TerminateTransaction {
        /// Identifies the transaction to terminate.
        transaction_id: TransactionId,
        /// Final status condition describing why the transaction ended.
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to calculate a checksum for a transaction.
    CalculateChecksum {
        /// Identifies the transaction to compute the checksum for.
        transaction_id: TransactionId,
        /// Algorithm to use for the checksum calculation.
        checksum_type: ChecksumType,
    },
    /// Instructs the `Runner` to notify the user that a fault has occurred.
    NotifyFault {
        /// Identifies the faulted transaction.
        transaction_id: TransactionId,
        /// The condition that caused the fault.
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to notify the user that a transaction was suspended.
    NotifySuspended {
        /// Identifies the suspended transaction.
        transaction_id: TransactionId,
    },
    /// Instructs the `Runner` to notify the user that a transaction was resumed.
    NotifyResumed {
        /// Identifies the resumed transaction.
        transaction_id: TransactionId,
        /// Number of bytes successfully transferred before suspension.
        progress: u64,
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
