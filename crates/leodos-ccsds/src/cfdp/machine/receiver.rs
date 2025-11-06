//! The synchronous state machine for the receiving-side of a CFDP transaction.
//!
//! This module contains the [`ReceiverMachine`], which processes [`Event`]s and
//! produces [`Action`]s. It is responsible for handling incoming file data,
//! tracking missing segments, and sending acknowledgments back to the sender.
//! It is designed to be completely independent of the underlying I/O and timing
//! mechanisms, making it portable and easily testable.

use crate::cfdp::machine::tracker::SegmentTracker;
use crate::cfdp::machine::transaction::Transaction;
use crate::cfdp::machine::transaction::TransactionConfig;
use crate::cfdp::machine::transaction::TransactionId;
use crate::cfdp::machine::transaction::TransactionState;
use crate::cfdp::machine::TimerType;
use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::machine::FILE_DATA_CHUNK_SIZE;
use crate::cfdp::machine::MAX_ACTIONS_PER_EVENT;
use crate::cfdp::machine::MAX_CONCURRENT_TRANSACTIONS;
use crate::cfdp::pdu::ConditionCode;
use crate::cfdp::pdu::EntityId;
use crate::cfdp::pdu::FinishedPdu;
use crate::cfdp::pdu::MetadataPdu;
use crate::cfdp::pdu::Pdu;
use heapless::index_map::FnvIndexMap;
use heapless::Vec;
use zerocopy::byteorder::network_endian::U64;

/// Manages the state of all active receiving ('destination') transactions.
///
/// This struct holds a map of all transactions for which this entity is the receiver.
/// It should be driven by a `Runner` which feeds it events and executes the
/// resulting actions.
#[derive(Debug, Default, Clone)]
pub struct ReceiverMachine {
    /// A map of active transactions, keyed by their unique `TransactionId`.
    transactions: FnvIndexMap<TransactionId, Transaction, MAX_CONCURRENT_TRANSACTIONS>,
    my_entity_id: u32,
}

const DIRECTIVE_CODE_FINISHED: u8 = 0x05;
const DIRECTIVE_CODE_EOF: u8 = 0x04;

/// Represents all possible inputs that can drive the `ReceiverMachine`.
#[derive(Debug)]
pub enum Event<'a> {
    /// A PDU has been received from a remote entity for a transaction this machine is handling.
    PduReceived {
        pdu: Pdu<'a>,
        transaction_id: TransactionId,
    },
    /// A timer, previously requested via an `Action`, has expired.
    TimerExpired(TimerType, TransactionId),
    /// A chunk of file data, requested to be written via a `WriteFileData` action, has been successfully written.
    FileDataWritten {
        id: TransactionId,
        offset: u64,
        len: usize,
    },
}

/// Represents all possible outputs from the `ReceiverMachine`.
///
/// These are instructions for the `Runner` to execute.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Instructs the `Runner` to serialize and send an ACK PDU.
    SendAck {
        destination: EntityId,
        transaction_id: TransactionId,
        directive_code: u8,
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to serialize and send a NAK PDU.
    SendNak {
        destination: EntityId,
        transaction_id: TransactionId,
        start_of_scope: u64,
        end_of_scope: u64,
        segment_requests: Vec<(U64, U64), 32>,
    },
    /// Instructs the `Runner` to write a chunk of data to the filestore.
    WriteFileData {
        id: TransactionId,
        data: Vec<u8, FILE_DATA_CHUNK_SIZE>,
        offset: u64,
    },
    /// Instructs the `Runner` to start a timer for a specific transaction.
    StartTimer(TimerType, u64, TransactionId),
    /// Instructs the `Runner` to stop a timer for a specific transaction.
    StopTimer(TimerType, TransactionId),
    /// Instructs the `Runner` to notify the user that a transaction has finished.
    NotifyTransactionFinished(TransactionFinishedParams),
    /// Instructs the `Runner` to notify the user that a file has been successfully received.
    NotifyFileReceived(ReceivedFileParams),
    /// Instructs the `Runner` that a transaction is complete and can be cleaned up.
    TransactionComplete(TransactionId),
}

/// Parameters providing information about a newly received file.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ReceivedFileParams {
    /// The unique ID of the completed transaction.
    pub id: TransactionId,
    /// The final name of the received file.
    pub file_name: Vec<u8, 256>,
    /// The total size of the received file in bytes.
    pub length: u64,
}

impl ReceiverMachine {
    /// Creates a new `ReceiverMachine`.
    pub fn new(my_entity_id: u32) -> Self {
        Self {
            transactions: FnvIndexMap::new(),
            my_entity_id,
        }
    }

    /// Retrieves the destination file name for a given transaction.
    /// Used by the `Runner` to fulfill `WriteFileData` actions.
    pub fn get_transaction_filestore_name(&self, id: &TransactionId) -> Option<&str> {
        self.transactions
            .get(id)
            .and_then(|txn| core::str::from_utf8(&txn.config.destination_file_name).ok())
    }

    /// The primary state machine logic for the receiver.
    ///
    /// It takes an `Event` as input and returns a `Vec` of `Action`s for the
    /// `Runner` to execute. This function is pure and has no side effects.
    pub fn handle<'a>(
        &mut self,
        event: Event<'a>,
    ) -> Result<Vec<Action, MAX_ACTIONS_PER_EVENT>, ()> {
        let mut actions = Vec::new();

        match event {
            Event::PduReceived {
                pdu,
                transaction_id,
            } => match pdu {
                Pdu::Metadata(meta) => self
                    .handle_metadata(transaction_id, meta, &mut actions)
                    .expect("Should have space for action"),
                Pdu::FileData(data) => self
                    .handle_file_data(transaction_id, &data.data, data.offset.get(), &mut actions)
                    .expect("Should have space for action"),
                Pdu::Eof(_eof) => self
                    .handle_eof(transaction_id, &mut actions)
                    .expect("Should have space for action"),
                Pdu::Finished(fin) => self
                    .handle_finished(transaction_id, fin, &mut actions)
                    .expect("Should have space for action"),
                _ => {}
            },
            Event::FileDataWritten { id, offset, len } => {
                if let Some(transaction) = self.transactions.get_mut(&id) {
                    transaction.progress = offset + len as u64;
                    if let Some(tracker) = transaction.tracker.as_mut() {
                        tracker.add_segment(offset, len as u64);
                    }
                }
            }
            Event::TimerExpired(timer_type, id) => match timer_type {
                TimerType::Nak => self
                    .handle_nak_timeout(id, &mut actions)
                    .expect("Should have space for action"),
                TimerType::Inactivity => self
                    .handle_inactivity(id, &mut actions)
                    .expect("Should have space for action"),
                _ => {}
            },
        }
        Ok(actions)
    }

    fn handle_metadata(
        &mut self,
        id: TransactionId,
        meta: &MetadataPdu,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if self.transactions.contains_key(&id) {
            return Ok(());
        }
        let file_size = meta.file_size.get();
        let config = TransactionConfig {
            id,
            dest_entity_id: self.my_entity_id.into(),
            source_file_name: Vec::from_slice(meta.source_file_name().unwrap_or_default()).unwrap(),
            destination_file_name: Vec::from_slice(meta.dest_file_name().unwrap_or_default())
                .unwrap(),
            file_size,
            ack_limit: 5,
            nak_limit: 5,
            nak_timeout_secs: 10,
            inactivity_timeout_secs: 30,
        };
        let transaction = Transaction {
            config,
            state: TransactionState::ReceivingFileData,
            progress: 0,
            tracker: Some(SegmentTracker::new(file_size)),
            ack_retries: 0,
            nak_retries: 0,
        };
        self.transactions
            .insert(id, transaction)
            .map_err(|_| ())
            .expect("Should have space for action");
        let inactivity_timeout = self
            .transactions
            .get(&id)
            .unwrap()
            .config
            .inactivity_timeout_secs;
        actions
            .push(Action::StartTimer(
                TimerType::Inactivity,
                inactivity_timeout,
                id,
            ))
            .expect("Should have space for action");
        Ok(())
    }

    fn handle_file_data(
        &mut self,
        id: TransactionId,
        data: &[u8],
        offset: u64,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if self.transactions.contains_key(&id) {
            actions
                .push(Action::StopTimer(TimerType::Inactivity, id))
                .expect("Should have space for action");
            actions
                .push(Action::WriteFileData {
                    id,
                    data: Vec::from_slice(data).unwrap(),
                    offset,
                })
                .expect("Should have space for action");
            let inactivity_timeout = self
                .transactions
                .get(&id)
                .unwrap()
                .config
                .inactivity_timeout_secs;
            actions
                .push(Action::StartTimer(
                    TimerType::Inactivity,
                    inactivity_timeout,
                    id,
                ))
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn handle_eof(
        &mut self,
        id: TransactionId,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        let mut send_ack = false;
        let mut send_nak = false;

        if let Some(transaction) = self.transactions.get_mut(&id) {
            actions
                .push(Action::StopTimer(TimerType::Inactivity, id))
                .expect("Should have space for action");
            let tracker = transaction.tracker.as_ref().unwrap();

            if tracker.is_complete() {
                send_ack = true;
                actions
                    .push(Action::NotifyFileReceived(ReceivedFileParams {
                        id,
                        file_name: transaction.config.destination_file_name.clone(),
                        length: transaction.config.file_size,
                    }))
                    .expect("Should have space for action");
            } else {
                send_nak = true;
            }
        }

        if send_ack {
            self.send_eof_ack(id, ConditionCode::NoError, actions)
                .expect("Should have space for action");
        }
        if send_nak {
            self.send_nak(id, actions)
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn handle_finished(
        &mut self,
        id: TransactionId,
        fin: &FinishedPdu,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.remove(&id) {
            actions
                .push(Action::SendAck {
                    destination: transaction.config.id.source_entity_id,
                    transaction_id: id,
                    directive_code: DIRECTIVE_CODE_FINISHED,
                    condition_code: fin.condition_code(),
                })
                .expect("Should have space for action");

            actions
                .push(Action::NotifyTransactionFinished(
                    TransactionFinishedParams {
                        id,
                        condition_code: fin.condition_code(),
                    },
                ))
                .expect("Should have space for action");
            actions
                .push(Action::TransactionComplete(id))
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn send_eof_ack(
        &mut self,
        id: TransactionId,
        condition: ConditionCode,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.get_mut(&id) {
            actions
                .push(Action::SendAck {
                    destination: transaction.config.id.source_entity_id,
                    transaction_id: id,
                    directive_code: DIRECTIVE_CODE_EOF,
                    condition_code: condition,
                })
                .expect("Should have space for action");
            transaction.state = TransactionState::SendingEofAck;
        }
        Ok(())
    }

    fn send_nak(
        &mut self,
        id: TransactionId,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.get_mut(&id) {
            let tracker = transaction.tracker.as_ref().unwrap();
            let missing = tracker.get_missing_ranges();

            actions
                .push(Action::SendNak {
                    destination: transaction.config.id.source_entity_id,
                    transaction_id: id,
                    start_of_scope: 0,
                    end_of_scope: transaction.config.file_size,
                    segment_requests: missing,
                })
                .expect("Should have space for action");
            transaction.state = TransactionState::SendingNak;
            let nak_timeout = transaction.config.nak_timeout_secs;
            actions
                .push(Action::StartTimer(
                    TimerType::Nak,
                    nak_timeout,
                    transaction.config.id,
                ))
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn handle_nak_timeout(
        &mut self,
        id: TransactionId,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        let mut should_resend_nak = false;
        let mut should_fault = false;

        if let Some(transaction) = self.transactions.get_mut(&id) {
            if transaction.nak_retries >= transaction.config.nak_limit {
                should_fault = true;
            } else {
                transaction.nak_retries += 1;
                should_resend_nak = true;
            }
        }

        if should_fault {
            self.fault_transaction(id, ConditionCode::NakLimitReached, actions)
                .expect("Should have space for action");
        }
        if should_resend_nak {
            self.send_nak(id, actions)
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn handle_inactivity(
        &mut self,
        id: TransactionId,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        self.fault_transaction(id, ConditionCode::InactivityDetected, actions)
    }

    fn fault_transaction(
        &mut self,
        id: TransactionId,
        condition: ConditionCode,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.remove(&id) {
            actions
                .push(Action::StopTimer(
                    TimerType::Inactivity,
                    transaction.config.id,
                ))
                .expect("Should have space for action");
            actions
                .push(Action::StopTimer(TimerType::Nak, transaction.config.id))
                .expect("Should have space for action");
            actions
                .push(Action::NotifyTransactionFinished(
                    TransactionFinishedParams {
                        id: transaction.config.id,
                        condition_code: condition,
                    },
                ))
                .expect("Should have space for action");
            actions
                .push(Action::TransactionComplete(transaction.config.id))
                .expect("Should have space for action");
        }
        Ok(())
    }
}
