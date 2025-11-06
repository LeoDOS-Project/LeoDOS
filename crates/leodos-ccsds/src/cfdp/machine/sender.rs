//! Synchronous state machine for the sending-side of a CFDP transaction.
//!
//! This module contains the [`SenderMachine`], which processes [`Event`]s and
//! produces [`Action`]s. It is responsible for initiating file transfers,
//! sending file data, and handling acknowledgments from the receiver. It is
//! designed to be completely independent of the underlying I/O and timing mechanisms,
//! making it portable and easily testable.

use crate::cfdp::machine::transaction::{
    Transaction, TransactionConfig, TransactionId, TransactionState,
};
use crate::cfdp::machine::{
    TimerType, TransactionFinishedParams, FILE_DATA_CHUNK_SIZE, MAX_ACTIONS_PER_EVENT,
    MAX_CONCURRENT_TRANSACTIONS,
};
use crate::cfdp::pdu::{AckPdu, ConditionCode, EntityId, NakPdu, Pdu};
use heapless::index_map::FnvIndexMap;
use heapless::Vec;
use zerocopy::byteorder::network_endian::U32;

/// The default timeout in seconds to wait for an expected ACK PDU.
const ACK_TIMEOUT_SECONDS: u64 = 10;

/// Manages the state of all active sending ('source') transactions.
///
/// This struct holds a map of all transactions for which this entity is the sender.
/// It should be driven by a `Runner` which feeds it events and executes the
/// resulting actions.
#[derive(Debug, Clone)]
pub struct SenderMachine {
    /// A map of active transactions, keyed by their unique `TransactionId`.
    transactions: FnvIndexMap<TransactionId, Transaction, MAX_CONCURRENT_TRANSACTIONS>,
    /// The next transaction sequence number to be used for a new `PutRequest`.
    next_seq_num: u32,
    /// The CFDP Entity ID of this local entity.
    my_entity_id: U32,
}

/// Represents all possible inputs that can drive the `SenderMachine`.
#[derive(Debug)]
pub enum Event<'a> {
    /// A user request to send a file to a remote entity.
    PutRequest {
        source_file_name: Vec<u8, 256>,
        destination_file_name: Vec<u8, 256>,
        dest_entity_id: EntityId,
        file_size: u64,
    },
    /// A PDU has been received from a remote entity for a transaction this machine is handling.
    PduReceived {
        pdu: Pdu<'a>,
        transaction_id: TransactionId,
    },
    /// A timer, previously requested via an `Action`, has expired.
    TimerExpired(TimerType, TransactionId),
    /// A chunk of file data, requested via a `RequestFileData` action, is ready to be sent.
    FileDataReady {
        id: TransactionId,
        data: Vec<u8, FILE_DATA_CHUNK_SIZE>,
        offset: u64,
    },
}

/// Represents all possible outputs from the `SenderMachine`.
///
/// These are instructions for the `Runner` to execute, such as sending PDUs,
/// managing timers, or interacting with the filestore.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Instructs the `Runner` to serialize and send a Metadata PDU.
    SendMetadata {
        destination: EntityId,
        transaction_id: TransactionId,
        file_size: u64,
        source_file_name: Vec<u8, 256>,
        dest_file_name: Vec<u8, 256>,
    },
    /// Instructs the `Runner` to serialize and send a File Data PDU.
    SendFileData {
        destination: EntityId,
        transaction_id: TransactionId,
        offset: u64,
        data: Vec<u8, FILE_DATA_CHUNK_SIZE>,
    },
    /// Instructs the `Runner` to serialize and send an EOF PDU.
    SendEof {
        destination: EntityId,
        transaction_id: TransactionId,
        condition_code: ConditionCode,
        file_size: u64,
    },
    /// Instructs the `Runner` to serialize and send a Finished PDU.
    SendFinished {
        destination: EntityId,
        transaction_id: TransactionId,
        condition_code: ConditionCode,
    },
    /// Instructs the `Runner` to read a chunk of data from the filestore.
    RequestFileData {
        id: TransactionId,
        offset: u64,
        length: u64,
    },
    /// Instructs the `Runner` to start a timer for a specific transaction.
    StartTimer(TimerType, u64, TransactionId),
    /// Instructs the `Runner` to stop a timer for a specific transaction.
    StopTimer(TimerType, TransactionId),
    /// Instructs the `Runner` to notify the user that a transaction has finished.
    NotifyTransactionFinished(TransactionFinishedParams),
    /// Instructs the `Runner` that a transaction is complete and can be cleaned up.
    TransactionComplete(TransactionId),
}

impl SenderMachine {
    /// Creates a new `SenderMachine` with the given local entity ID.
    pub fn new(my_entity_id: u32) -> Self {
        Self {
            transactions: FnvIndexMap::new(),
            next_seq_num: 0,
            my_entity_id: U32::new(my_entity_id),
        }
    }

    /// Retrieves the source file name for a given transaction.
    /// Used by the `Runner` to fulfill `RequestFileData` actions.
    pub fn get_transaction_filestore_name(&self, id: &TransactionId) -> Option<&str> {
        self.transactions
            .get(id)
            .and_then(|txn| core::str::from_utf8(&txn.config.source_file_name).ok())
    }

    /// Retrieves the destination entity ID for a given transaction.
    pub fn get_transaction_dest_id(&self, id: &TransactionId) -> Option<EntityId> {
        self.transactions
            .get(id)
            .map(|txn| txn.config.dest_entity_id)
    }

    /// The primary state machine logic for the sender.
    ///
    /// It takes an `Event` as input and returns a `Vec` of `Action`s for the
    /// `Runner` to execute. This function is pure and has no side effects.
    pub fn handle<'a>(
        &mut self,
        event: Event<'a>,
    ) -> Result<Vec<Action, MAX_ACTIONS_PER_EVENT>, ()> {
        let mut actions = Vec::new();

        match event {
            Event::PutRequest {
                source_file_name,
                destination_file_name,
                dest_entity_id,
                file_size
            } => {
                let id = TransactionId {
                    source_entity_id: self.my_entity_id,
                    sequence_number: self.next_seq_num.into(),
                };
                self.next_seq_num = self.next_seq_num.wrapping_add(1);

                let config = TransactionConfig {
                    id,
                    dest_entity_id,
                    source_file_name: source_file_name.clone(),
                    destination_file_name: destination_file_name.clone(),
                    file_size,
                    ack_limit: 5,
                    nak_limit: 5,
                    nak_timeout_secs: 10,
                    inactivity_timeout_secs: 30,
                };
                let transaction = Transaction {
                    config,
                    state: TransactionState::SendingFileData,
                    progress: 0,
                    tracker: None,
                    ack_retries: 0,
                    nak_retries: 0,
                };
                self.transactions
                    .insert(id, transaction)
                    .map_err(|_| ())
                    .expect("Should have space for action");

                actions
                    .push(Action::SendMetadata {
                        destination: dest_entity_id,
                        transaction_id: id,
                        file_size: 0,
                        source_file_name,
                        dest_file_name: destination_file_name,
                    })
                    .expect("Should have space for action");
                actions
                    .push(Action::RequestFileData {
                        id,
                        offset: 0,
                        length: FILE_DATA_CHUNK_SIZE as u64,
                    })
                    .expect("Should have space for action");
            }
            Event::FileDataReady { id, data, offset } => {
                let mut should_send_eof = false;
                if let Some(transaction) = self.transactions.get_mut(&id) {
                    let data_len = data.len() as u64;
                    actions
                        .push(Action::SendFileData {
                            destination: U32::new(0), // Placeholder, runner will get this from map
                            transaction_id: id,
                            offset,
                            data,
                        })
                        .expect("Should have space for action");
                    transaction.progress = offset + data_len;

                    if transaction.progress < transaction.config.file_size {
                        actions
                            .push(Action::RequestFileData {
                                id,
                                offset: transaction.progress,
                                length: FILE_DATA_CHUNK_SIZE as u64,
                            })
                            .expect("Should have space for action");
                    } else {
                        should_send_eof = true;
                    }
                }
                if should_send_eof {
                    self.send_eof(id, &mut actions)
                        .expect("Should have space for action");
                }
            }
            Event::PduReceived {
                pdu,
                transaction_id,
            } => match pdu {
                Pdu::Ack(ack) => self
                    .handle_ack(transaction_id, ack, &mut actions)
                    .expect("Should have space for action"),
                Pdu::Nak(nak) => self
                    .handle_nak(transaction_id, nak, &mut actions)
                    .expect("Should have space for action"),
                _ => {}
            },
            Event::TimerExpired(timer_type, id) => match timer_type {
                TimerType::Ack => {
                    let mut retransmit = None;
                    if let Some(transaction) = self.transactions.get_mut(&id) {
                        if transaction.ack_retries >= transaction.config.ack_limit {
                            self.fault_transaction(
                                id,
                                ConditionCode::AckLimitReached,
                                &mut actions,
                            )
                            .expect("Should have space for action");
                        } else {
                            transaction.ack_retries += 1;
                            retransmit = Some(transaction.state.clone());
                        }
                    }
                    if let Some(state) = retransmit {
                        match state {
                            TransactionState::WaitingForEofAck => self
                                .send_eof(id, &mut actions)
                                .expect("Should have space for action"),
                            TransactionState::WaitingForFinishedAck => self
                                .send_finished(id, ConditionCode::NoError, &mut actions)
                                .expect("Should have space for action"),
                            _ => {}
                        }
                    }
                }
                TimerType::Nak | TimerType::Inactivity => {}
            },
        }
        Ok(actions)
    }

    fn send_eof(
        &mut self,
        id: TransactionId,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.get_mut(&id) {
            actions
                .push(Action::SendEof {
                    destination: U32::new(0),
                    transaction_id: id,
                    condition_code: ConditionCode::NoError,
                    file_size: transaction.config.file_size,
                })
                .expect("Should have space for action");
            transaction.state = TransactionState::WaitingForEofAck;
            actions
                .push(Action::StartTimer(TimerType::Ack, ACK_TIMEOUT_SECONDS, id))
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn send_finished(
        &mut self,
        id: TransactionId,
        condition: ConditionCode,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.get_mut(&id) {
            actions
                .push(Action::SendFinished {
                    destination: U32::new(0),
                    transaction_id: id,
                    condition_code: condition,
                })
                .expect("Should have space for action");
            transaction.state = TransactionState::WaitingForFinishedAck;
            actions
                .push(Action::StartTimer(TimerType::Ack, ACK_TIMEOUT_SECONDS, id))
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn handle_ack(
        &mut self,
        id: TransactionId,
        ack: &AckPdu,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        let mut should_send_finished = false;
        let mut should_complete = false;

        if let Some(transaction) = self.transactions.get_mut(&id) {
            match transaction.state {
                TransactionState::WaitingForEofAck if ack.directive_code == 0x04 => {
                    should_send_finished = true;
                }
                TransactionState::WaitingForFinishedAck if ack.directive_code == 0x05 => {
                    should_complete = true;
                }
                _ => {}
            }
        }

        if should_send_finished {
            actions
                .push(Action::StopTimer(TimerType::Ack, id))
                .expect("Should have space for action");
            self.send_finished(id, ack.condition_code(), actions)
                .expect("Should have space for action");
        }
        if should_complete {
            actions
                .push(Action::StopTimer(TimerType::Ack, id))
                .expect("Should have space for action");
            self.fault_transaction(id, ack.condition_code(), actions)
                .expect("Should have space for action");
        }
        Ok(())
    }

    fn handle_nak(
        &mut self,
        id: TransactionId,
        nak: &NakPdu,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if self.transactions.get_mut(&id).is_some() {
            actions
                .push(Action::StopTimer(TimerType::Ack, id))
                .expect("Should have space for action");
            if let Some(segments) = nak.segment_requests() {
                for segment in segments {
                    actions
                        .push(Action::RequestFileData {
                            id,
                            offset: segment.offset.get(),
                            length: segment.length.get(),
                        })
                        .expect("Should have space for action");
                }
            }
        }
        Ok(())
    }

    fn fault_transaction(
        &mut self,
        id: TransactionId,
        condition: ConditionCode,
        actions: &mut Vec<Action, MAX_ACTIONS_PER_EVENT>,
    ) -> Result<(), ()> {
        if let Some(transaction) = self.transactions.remove(&id) {
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
