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
        transaction_id: TransactionId,
        destination_id: EntityId,
        file_size: u64,
        source_file_name: FileId,
        destination_file_name: FileId,
        checksum_type: ChecksumType,
    },
    /// Instructs the `Runner` to serialize and send a File Data PDU.
    SendFileData {
        transaction_id: TransactionId,
        destination_id: EntityId,
        offset: u64,
        data: &'a [u8],
    },
    /// Instructs the `Runner` to serialize and send an EOF PDU.
    SendEof {
        transaction_id: TransactionId,
        destination_id: EntityId,
        condition_code: ConditionCode,
        file_size: u64,
        checksum: u32,
    },
    /// Instructs the `Runner` to serialize and send a Finished PDU.
    SendFinished {
        transaction_id: TransactionId,
        destination_id: EntityId,
        condition_code: ConditionCode,
    },
    SendAck {
        transaction_id: TransactionId,
        destination_id: EntityId,
        acked_directive_code: AckedDirectiveCode,
        condition_code: ConditionCode,
        transaction_status: TransactionStatus,
    },
    SendPrompt {
        transaction_id: TransactionId,
        destination_id: EntityId,
        prompt_type: PromptType,
    },
    /// Instructs the `Runner` to read a segment of data from the filestore.
    ReadDataSegment {
        transaction_id: TransactionId,
        start_offset: u64,
        end_offset: u64,
    },
    /// Instructs the `Runner` to read multiple segments of data from the filestore.
    ReadDataSegmentBatch {
        transaction_id: TransactionId,
        segments: NakSegmentsIterator<'a>,
    },
    /// Instructs the `Runner` to start a timer for a specific transaction.
    StartTimer {
        transaction_id: TransactionId,
        timer_type: TimerType,
        seconds: u16,
    },
    /// Instructs the `Runner` to stop a timer for a specific transaction.
    StopTimer {
        transaction_id: TransactionId,
        /// The type of timer to stop. If `None`, stops all timers for the transaction.
        timer_type: Option<TimerType>,
    },
    /// Instructs the `Runner` that a transaction is completed and can be cleaned up.
    TerminateTransaction {
        transaction_id: TransactionId,
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to calculate a checksum for a transaction.
    CalculateChecksum {
        transaction_id: TransactionId,
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
