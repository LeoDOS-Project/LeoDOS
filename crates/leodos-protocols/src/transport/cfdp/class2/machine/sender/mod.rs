//! Synchronous state machine for the sending-side of a CFDP transaction.
//!
//! This module contains the [`SenderMachine`], which processes [`Event`]s and
//! produces [`Action`]s. It is responsible for initiating file transfers,
//! sending file data, and handling acknowledgments from the receiver. It is
//! designed to be completely independent of the underlying I/O and timing mechanisms,
//! making it portable and easily testable.

/// Output actions produced by the sender state machine.
pub mod action;
/// Input events that drive the sender state machine.
pub mod event;
/// Transaction state and configuration for the sender.
pub mod transaction;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::class2::machine::FILE_DATA_CHUNK_SIZE;
use crate::transport::cfdp::class2::machine::MAX_CONCURRENT_TRANSACTIONS;
use crate::transport::cfdp::class2::machine::PromptType;
use crate::transport::cfdp::class2::machine::TimerType;
use crate::transport::cfdp::class2::machine::sender::action::Action;
use crate::transport::cfdp::class2::machine::sender::action::Actions;
use crate::transport::cfdp::class2::machine::sender::event::Event;
use crate::transport::cfdp::class2::machine::sender::transaction::Transaction;
use crate::transport::cfdp::class2::machine::sender::transaction::TransactionState;
use crate::transport::cfdp::class2::machine::transaction::TransactionConfig;
use crate::transport::cfdp::class2::machine::transaction::TransactionId;
use crate::transport::cfdp::filestore::FileId;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduVariant;
use crate::transport::cfdp::pdu::TransactionSeqNum;
use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::pdu::file_directive::ack::AckPdu;
use crate::transport::cfdp::pdu::file_directive::ack::AckedDirectiveCode;
use crate::transport::cfdp::pdu::file_directive::ack::TransactionStatus;
use crate::transport::cfdp::pdu::file_directive::finished::FinishedPdu;
use crate::transport::cfdp::pdu::file_directive::keepalive::KeepAlivePdu;
use crate::transport::cfdp::pdu::file_directive::metadata::ChecksumType;
use crate::transport::cfdp::pdu::file_directive::nak::NakPdu;
use crate::transport::cfdp::pdu::tlv::fault_handler_override::HandlerCode;
use heapless::LinearMap;

/// Manages the state of all active sending ('source') transactions.
///
/// This struct holds a map of all transactions for which this entity is the sender.
/// It should be driven by a `Runner` which feeds it events and executes the
/// resulting actions.
#[derive(Debug)]
pub struct SenderMachine {
    /// A map of active transactions, keyed by their unique `TransactionId`.
    transactions: LinearMap<TransactionId, Transaction, MAX_CONCURRENT_TRANSACTIONS>,
    /// The next transaction sequence number to be used for a new `PutRequest`.
    next_seq_num: TransactionSeqNum,
    /// The CFDP Entity ID of this local entity.
    id: EntityId,
    /// The keep-alive limit for transactions.
    keep_alive_limit: u64,
    /// The timeout in seconds to wait for an expected ACK PDU.
    ack_timeout_secs: u16,
}

impl SenderMachine {
    /// Creates a new `SenderMachine` with the given local entity ID.
    pub fn new(id: EntityId, ack_timeout_secs: u16, keep_alive_limit: u64) -> Self {
        Self {
            transactions: LinearMap::new(),
            next_seq_num: TransactionSeqNum::default(),
            id,
            ack_timeout_secs,
            keep_alive_limit,
        }
    }

    fn create_transation<'a>(
        &mut self,
        destination_id: EntityId,
        source_file_id: FileId,
        destination_file_id: FileId,
        file_size: u64,
        checksum_type: ChecksumType,
    ) -> Result<TransactionId, CfdpError> {
        let transaction_id = TransactionId {
            source_id: self.id,
            seq_num: self.next_seq_num,
        };
        self.next_seq_num.increment();
        let config = TransactionConfig {
            transaction_id,
            destination_id,
            source_file_id,
            destination_file_id,
            file_size,
            inactivity_timeout_secs: 30,
            checksum_type,
            fault_handlers: Default::default(),
        };
        let transaction = Transaction {
            config,
            state: TransactionState::SendingFileData,
            suspended: false,
            progress: 0,
            ack_retries: 0,
            file_checksum: None,
            last_receiver_progress: 0,
            ack_limit: 5,
            keep_alive_limit: self.keep_alive_limit,
        };
        self.transactions
            .insert(transaction_id, transaction)
            .map_err(|_| CfdpError::TooManyConcurrentTransactions)?;
        Ok(transaction_id)
    }

    /// The primary state machine logic for the sender.
    ///
    /// It takes an `Event` as input and returns a `Vec` of `Action`s for the
    /// `Runner` to execute. This function is pure and has no side effects.
    pub fn handle<'a>(
        &mut self,
        actions: &mut Actions<'a>,
        event: Event<'a>,
    ) -> Result<(), CfdpError> {
        actions.clear();
        match event {
            Event::PutRequest {
                source_file_name,
                destination_file_name,
                destination_id,
                file_size,
                checksum_type,
            } => self.handle_put_request(
                destination_id,
                source_file_name,
                destination_file_name,
                file_size,
                checksum_type,
                actions,
            )?,
            Event::DataSegmentReady {
                transaction_id,
                data,
                offset,
            } => self.handle_data_segment_ready(transaction_id, offset, data, actions)?,
            Event::ChecksumReady {
                transaction_id,
                checksum,
            } => self.handle_checksum(transaction_id, checksum, actions)?,
            Event::PduReceived {
                pdu,
                transaction_id,
            } => self.handle_pdu_received(pdu, transaction_id, actions)?,
            Event::TimerExpired {
                timer_type,
                transaction_id,
            } => self.handle_timer_expired(transaction_id, timer_type, actions)?,
            Event::PromptRequest {
                transaction_id,
                prompt_type,
            } => self.handle_prompt(transaction_id, prompt_type, actions)?,
            Event::SuspendRequest { transaction_id } => {
                self.handle_suspend(transaction_id, actions)?
            }
            Event::ResumeRequest { transaction_id } => {
                self.handle_resume(transaction_id, actions)?
            }
        }
        Ok(())
    }

    fn handle_prompt<'a>(
        &mut self,
        transaction_id: TransactionId,
        prompt_type: PromptType,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        match prompt_type {
            PromptType::Nak => self.handle_prompt_nak(transaction_id, actions)?,
            PromptType::KeepAlive => self.handle_prompt_keep_alive(transaction_id, actions)?,
        }
        Ok(())
    }

    fn handle_prompt_keep_alive<'a>(
        &mut self,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get(&transaction_id) else {
            return Ok(());
        };

        // It only makes sense to send a Prompt(KeepAlive) while we are actively sending
        // data and before the EOF has been sent.
        match transaction.state {
            TransactionState::SendingFileData if !transaction.suspended => {
                actions.push(Action::SendPrompt {
                    transaction_id,
                    destination_id: transaction.config.destination_id,
                    prompt_type: PromptType::KeepAlive,
                })?;
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_pdu_received<'a>(
        &mut self,
        pdu: &'a Pdu,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        match pdu.variant()? {
            PduVariant::Finished(v) => self.handle_finished(transaction_id, v, actions)?,
            PduVariant::Ack(v) => self.handle_ack(transaction_id, v, actions)?,
            // Process active data transfers only if not suspended
            PduVariant::Nak(v) if !transaction.suspended => {
                self.handle_nak(transaction_id, v, actions)?
            }
            PduVariant::KeepAlive(v) if !transaction.suspended => {
                self.handle_keep_alive(transaction_id, v, actions)?
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_data_segment_ready<'a>(
        &mut self,
        transaction_id: TransactionId,
        offset: u64,
        data: &'a [u8],
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };

        if transaction.suspended {
            return Ok(());
        }

        actions.push(Action::SendFileData {
            destination_id: transaction.config.destination_id,
            transaction_id,
            offset,
            data,
        })?;

        transaction.progress = offset + data.len() as u64;

        if transaction.progress >= transaction.config.file_size {
            actions.push(Action::CalculateChecksum {
                transaction_id,
                checksum_type: transaction.config.checksum_type,
            })?;
        } else {
            actions.push(Action::ReadDataSegment {
                transaction_id,
                start_offset: transaction.progress,
                end_offset: transaction.progress + FILE_DATA_CHUNK_SIZE as u64,
            })?;
        }
        Ok(())
    }

    fn handle_put_request<'a>(
        &mut self,
        destination_id: EntityId,
        source_file_name: FileId,
        destination_file_name: FileId,
        file_size: u64,
        checksum_type: ChecksumType,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let transaction_id = self.create_transation(
            destination_id,
            source_file_name,
            destination_file_name,
            file_size,
            checksum_type,
        )?;
        actions.push(Action::SendMetadata {
            destination_id,
            transaction_id,
            file_size: 0,
            source_file_name,
            destination_file_name,
            checksum_type,
        })?;
        actions.push(Action::ReadDataSegment {
            transaction_id,
            start_offset: 0,
            end_offset: FILE_DATA_CHUNK_SIZE as u64,
        })?;
        Ok(())
    }

    fn handle_checksum<'a>(
        &mut self,
        transaction_id: TransactionId,
        checksum: u32,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        transaction.file_checksum = Some(checksum);
        self.send_eof(transaction_id, checksum, actions)
    }

    fn handle_timer_expired<'a>(
        &mut self,
        transaction_id: TransactionId,
        timer_type: TimerType,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        match timer_type {
            TimerType::Ack => {
                let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
                    return Ok(());
                };
                if transaction.ack_retries >= transaction.ack_limit {
                    // Exceeded ACK retries, fault the transaction
                    self.handle_terminate(transaction_id, ConditionCode::AckLimitReached, actions)?;
                    return Ok(());
                }
                // Retry sending the EOF PDU
                transaction.ack_retries += 1;
                if transaction.state != TransactionState::WaitingForEofAck {
                    return Ok(());
                }
                let checksum = transaction
                    .file_checksum
                    .ok_or_else(|| CfdpError::Custom("Checksum not calculated"))?;
                self.send_eof(transaction_id, checksum, actions)?;
            }
            TimerType::Inactivity => {
                let Some(transaction) = self.transactions.get(&transaction_id) else {
                    return Ok(());
                };
                if transaction.state != TransactionState::WaitingForFinishedPdu {
                    return Ok(());
                }
                self.handle_fault(transaction_id, ConditionCode::InactivityDetected, actions)?;
            }
            TimerType::Nak => {}
            TimerType::KeepAlive => {}
        }
        Ok(())
    }

    fn handle_prompt_nak<'a>(
        &mut self,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get(&transaction_id) else {
            return Ok(());
        };
        if transaction.state != TransactionState::SendingFileData {
            return Ok(());
        }
        actions.push(Action::SendPrompt {
            destination_id: transaction.config.destination_id,
            transaction_id,
            prompt_type: PromptType::Nak,
        })?;
        Ok(())
    }

    fn handle_suspend<'a>(
        &mut self,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        if transaction.suspended {
            return Ok(());
        }
        transaction.suspended = true;
        actions.push(Action::StopTimer {
            transaction_id,
            timer_type: None,
        })?;
        actions.push(Action::NotifySuspended { transaction_id })?;
        Ok(())
    }

    fn handle_resume<'a>(
        &mut self,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        if !transaction.suspended {
            return Ok(());
        }
        let progress = transaction.progress;
        transaction.suspended = false;
        match transaction.state {
            TransactionState::SendingFileData => {
                actions.push(Action::ReadDataSegment {
                    transaction_id,
                    start_offset: transaction.progress,
                    end_offset: transaction.progress + FILE_DATA_CHUNK_SIZE as u64,
                })?;
            }
            TransactionState::WaitingForEofAck => {
                let checksum = transaction
                    .file_checksum
                    .ok_or(CfdpError::Custom("Checksum missing on resume"))?;
                self.send_eof(transaction_id, checksum, actions)?;
            }
            TransactionState::WaitingForFinishedPdu => {
                actions.push(Action::StartTimer {
                    timer_type: TimerType::Inactivity,
                    seconds: transaction.config.inactivity_timeout_secs,
                    transaction_id,
                })?;
            }
            TransactionState::WaitingForChecksum => {
                actions.push(Action::CalculateChecksum {
                    transaction_id,
                    checksum_type: transaction.config.checksum_type,
                })?;
            }
        }
        actions.push(Action::NotifyResumed {
            transaction_id,
            progress,
        })?;
        Ok(())
    }

    fn handle_finished<'a>(
        &mut self,
        transaction_id: TransactionId,
        fin: &FinishedPdu,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get(&transaction_id) else {
            return Ok(());
        };
        if transaction.state != TransactionState::WaitingForFinishedPdu {
            return Err(CfdpError::Custom(
                "Received Finished PDU in unexpected transaction state",
            ));
        }

        // Per Spec 4.11.1.1.2.b, a sender's timers should be terminated upon completion.
        actions.push(Action::StopTimer {
            transaction_id,
            timer_type: None,
        })?;

        // Per Spec 4.7.2, upon receiving a PDU requiring an ACK (which Finished does, per 4.6.4.3.5),
        // we must immediately issue the Expected Response.
        actions.push(Action::SendAck {
            transaction_id,
            destination_id: transaction.config.destination_id,
            acked_directive_code: AckedDirectiveCode::Finished,
            condition_code: fin.condition_code()?,
            transaction_status: TransactionStatus::Terminated,
        })?;

        self.handle_terminate(transaction_id, fin.condition_code()?, actions)?;
        Ok(())
    }

    fn send_eof<'a>(
        &mut self,
        transaction_id: TransactionId,
        checksum: u32,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        actions.push(Action::SendEof {
            destination_id: transaction.config.destination_id,
            transaction_id,
            condition_code: ConditionCode::NoError,
            file_size: transaction.config.file_size,
            checksum,
        })?;
        transaction.state = TransactionState::WaitingForEofAck;
        actions.push(Action::StartTimer {
            timer_type: TimerType::Ack,
            seconds: self.ack_timeout_secs,
            transaction_id,
        })?;
        Ok(())
    }

    fn handle_ack<'a>(
        &mut self,
        transaction_id: TransactionId,
        ack: &AckPdu,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        match (transaction.state, ack.acked_directive_code()?) {
            (TransactionState::WaitingForEofAck, AckedDirectiveCode::Eof) => {
                actions.push(Action::StopTimer {
                    timer_type: Some(TimerType::Ack),
                    transaction_id,
                })?;
                transaction.state = TransactionState::WaitingForFinishedPdu;
                actions.push(Action::StartTimer {
                    timer_type: TimerType::Inactivity,
                    seconds: transaction.config.inactivity_timeout_secs,
                    transaction_id,
                })?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_nak<'a>(
        &mut self,
        transaction_id: TransactionId,
        nak: NakPdu<'a>,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        if self.transactions.get(&transaction_id).is_none() {
            return Ok(());
        }
        actions.push(Action::StopTimer {
            timer_type: Some(TimerType::Ack),
            transaction_id,
        })?;
        actions.push(Action::ReadDataSegmentBatch {
            transaction_id,
            segments: nak.segment_requests()?,
        })?;
        Ok(())
    }

    fn handle_keep_alive<'a>(
        &mut self,
        transaction_id: TransactionId,
        keepalive: KeepAlivePdu<'_>,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };

        let receiver_progress = keepalive.progress();

        if receiver_progress > transaction.last_receiver_progress {
            transaction.last_receiver_progress = receiver_progress;
        }

        let discrepancy = transaction
            .progress
            .saturating_sub(transaction.last_receiver_progress);

        if discrepancy > transaction.keep_alive_limit {
            self.handle_fault(
                transaction_id,
                ConditionCode::KeepAliveLimitReached,
                actions,
            )?;
        }

        // If the limit is not exceeded, we do nothing further. The progress is updated and the transaction continues.
        Ok(())
    }

    fn handle_terminate<'a>(
        &mut self,
        id: TransactionId,
        condition: ConditionCode,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.remove(&id) else {
            return Ok(());
        };
        actions.push(Action::TerminateTransaction {
            transaction_id: transaction.config.transaction_id,
            condition_code: condition,
        })?;
        Ok(())
    }

    fn handle_fault<'a>(
        &mut self,
        transaction_id: TransactionId,
        condition_code: ConditionCode,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get(&transaction_id) else {
            return Ok(());
        };
        match transaction
            .config
            .fault_handlers
            .get_handler(condition_code)
        {
            HandlerCode::Cancel => {
                self.handle_terminate(transaction_id, condition_code, actions)?;
            }
            HandlerCode::Ignore => {
                actions.push(Action::NotifyFault {
                    transaction_id,
                    condition_code,
                })?;
            }
            HandlerCode::Suspend => {
                self.handle_suspend(transaction_id, actions)?;
            }
            HandlerCode::Abandon => {
                // Abandon is similar to Cancel but implies no further PDUs will be sent.
                // For a receiver, it's effectively the same as Cancel.
                self.handle_terminate(transaction_id, condition_code, actions)?;
            }
        }
        Ok(())
    }
}
