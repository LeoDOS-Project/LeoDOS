//! The synchronous state machine for the receiving-side of a CFDP transaction.
//!
//! This module contains the [`ReceiverMachine`], which processes [`Event`]s and
//! produces [`Action`]s. It is responsible for handling incoming file data,
//! tracking missing segments, and sending acknowledgments back to the sender.
//! It is designed to be completely independent of the underlying I/O and timing
//! mechanisms, making it portable and easily testable.

pub mod action;
pub mod event;
pub mod transaction;

use crate::transport::cfdp::CfdpError;
use crate::transport::cfdp::class2::machine::MAX_CONCURRENT_TRANSACTIONS;
use crate::transport::cfdp::class2::machine::TimerType;
use crate::transport::cfdp::class2::machine::receiver::action::Action;
use crate::transport::cfdp::class2::machine::receiver::action::Actions;
use crate::transport::cfdp::class2::machine::receiver::event::Event;
use crate::transport::cfdp::class2::machine::receiver::transaction::Transaction;
use crate::transport::cfdp::class2::machine::receiver::transaction::TransactionState;
use crate::transport::cfdp::class2::machine::tracker::SegmentTracker;
use crate::transport::cfdp::class2::machine::transaction::TransactionConfig;
use crate::transport::cfdp::class2::machine::transaction::TransactionId;
use crate::transport::cfdp::filestore::FileStore;
use crate::transport::cfdp::pdu::EntityId;
use crate::transport::cfdp::pdu::Pdu;
use crate::transport::cfdp::pdu::PduVariant;
use crate::transport::cfdp::pdu::file_data::FileDataPdu;
use crate::transport::cfdp::pdu::file_directive::ConditionCode;
use crate::transport::cfdp::pdu::file_directive::DirectiveCode;
use crate::transport::cfdp::pdu::file_directive::ack::TransactionStatus;
use crate::transport::cfdp::pdu::file_directive::eof::EofPdu;
use crate::transport::cfdp::pdu::file_directive::metadata::ChecksumType;
use crate::transport::cfdp::pdu::file_directive::metadata::MetadataPdu;
use crate::transport::cfdp::pdu::file_directive::prompt::PromptResponse;
use crate::transport::cfdp::pdu::tlv::fault_handler_override::FaultHandlerSet;
use crate::transport::cfdp::pdu::tlv::fault_handler_override::HandlerCode;
use crate::transport::cfdp::pdu::tlv::filestore_request::FilestoreRequest;
use crate::transport::cfdp::pdu::tlv::filestore_request::TlvFilestoreRequest;
use crate::transport::cfdp::pdu::tlv::filestore_response::FilestoreResponse;
use heapless::LinearMap;
use heapless::Vec;

/// Manages the state of all active receiving ('destination') transactions.
///
/// This struct holds a map of all transactions for which this entity is the receiver.
/// It should be driven by a `Runner` which feeds it events and executes the
/// resulting actions.
#[derive(Debug, Default)]
pub struct ReceiverMachine {
    /// A map of active transactions, keyed by their unique `TransactionId`.
    transactions: LinearMap<TransactionId, Transaction, MAX_CONCURRENT_TRANSACTIONS>,
    id: EntityId,
}

impl ReceiverMachine {
    /// Creates a new `ReceiverMachine`.
    pub fn new(id: EntityId) -> Self {
        Self {
            transactions: LinearMap::new(),
            id,
        }
    }

    /// The primary state machine logic for the receiver.
    ///
    /// It takes an `Event` as input and returns a `Vec` of `Action`s for the
    /// `Runner` to execute. This function is pure and has no side effects.
    pub fn handle<'a>(
        &mut self,
        file_store: &mut impl FileStore,
        event: Event<'a>,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        actions.clear();
        match event {
            Event::PduReceived {
                pdu,
                transaction_id,
            } => self.handle_pdu_received(file_store, transaction_id, pdu, actions)?,
            Event::FileDataWritten {
                transaction_id,
                offset,
                len,
            } => self.handle_file_data_written(transaction_id, offset, len, actions)?,
            Event::TimerExpired {
                timer_type,
                transaction_id,
            } => self.handle_timer_expired(timer_type, transaction_id, actions)?,
            Event::ChecksumVerified {
                transaction_id,
                is_valid,
            } => self.handle_checksum_verified(transaction_id, is_valid, actions)?,
            Event::SuspendRequest { transaction_id } => {
                self.handle_suspend_request(transaction_id, actions)?
            }
            Event::ResumeRequest { transaction_id } => {
                self.handle_resume_request(transaction_id, actions)?
            }
            Event::FilestoreResponsesReceived {
                transaction_id,
                responses,
            } => self.handle_filestore_responses(transaction_id, responses, actions)?,
        }
        Ok(())
    }

    fn handle_filestore_responses<'a>(
        &mut self,
        transaction_id: TransactionId,
        responses: Vec<FilestoreResponse, 4, u8>,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get(&transaction_id) else {
            return Ok(());
        };

        if transaction.state != TransactionState::WaitingForFilestoreResponses {
            return Ok(()); // Ignore stale event
        }

        actions.push(Action::NotifyFileReceived {
            transaction_id,
            file_name: transaction.config.destination_file_id.clone(),
            file_size: transaction.config.file_size,
        })?;

        actions.push(Action::SendAck {
            destination_id: transaction.config.transaction_id.source_id,
            transaction_id,
            directive_code: DirectiveCode::Eof,
            condition_code: ConditionCode::NoError,
            transaction_status: TransactionStatus::Active,
        })?;

        actions.push(Action::SendFinished {
            destination_id: transaction.config.transaction_id.source_id,
            transaction_id,
            condition_code: ConditionCode::NoError,
            filestore_responses: responses,
        })?;

        self.terminate_transaction(transaction_id, ConditionCode::NoError, actions)?;
        Ok(())
    }

    fn handle_suspend_request<'a>(
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

    fn handle_resume_request<'a>(
        &mut self,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        transaction.suspended = false;
        match transaction.state {
            TransactionState::WaitingForNakData => {
                actions.push(Action::StartTimer {
                    timer_type: TimerType::Nak,
                    seconds: transaction.nak_timeout_secs,
                    transaction_id,
                })?;
                actions.push(Action::StartTimer {
                    timer_type: TimerType::Inactivity,
                    seconds: transaction.config.inactivity_timeout_secs,
                    transaction_id,
                })?;
            }
            TransactionState::ReceivingFileData => {
                if transaction.keep_alive_interval_secs != 0 {
                    actions.push(Action::StartTimer {
                        timer_type: TimerType::KeepAlive,
                        seconds: transaction.keep_alive_interval_secs,
                        transaction_id,
                    })?;
                }
                actions.push(Action::StartTimer {
                    timer_type: TimerType::Inactivity,
                    seconds: transaction.config.inactivity_timeout_secs,
                    transaction_id,
                })?;
            }
            _ => {}
        }
        actions.push(Action::NotifyResumed {
            transaction_id,
            progress: transaction.progress,
        })?;
        Ok(())
    }

    fn handle_timer_expired<'a>(
        &mut self,
        timer_type: TimerType,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        match timer_type {
            TimerType::Nak => self.handle_nak_timeout(transaction_id, actions)?,
            TimerType::Inactivity => self.handle_inactivity(transaction_id, actions)?,
            TimerType::Ack => {}
            TimerType::KeepAlive => {
                if let Some(transaction) = self.transactions.get(&transaction_id) {
                    actions.push(Action::SendKeepAlive {
                        transaction_id,
                        destination_id: transaction.config.transaction_id.source_id,
                        progress: transaction.progress,
                    })?;

                    if transaction.keep_alive_interval_secs != 0 {
                        actions.push(Action::StartTimer {
                            timer_type: TimerType::KeepAlive,
                            seconds: transaction.keep_alive_interval_secs,
                            transaction_id,
                        })?;
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_file_data_written<'a>(
        &mut self,
        transaction_id: TransactionId,
        offset: u64,
        len: usize,
        _actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        transaction.progress = offset + len as u64;
        transaction.tracker.add_segment(offset, len as u64)?;
        Ok(())
    }

    fn handle_pdu_received<'a>(
        &mut self,
        file_store: &mut impl FileStore,
        transaction_id: TransactionId,
        pdu: &'a Pdu,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        if let Some(transaction) = self.transactions.get(&transaction_id) {
            match pdu.variant()? {
                PduVariant::FileData(data) => {
                    self.handle_file_data(transaction_id, pdu.large_file_flag(), data, actions)?;
                }
                PduVariant::Eof(eof) => {
                    self.handle_eof(transaction_id, pdu, eof, actions)?;
                }
                PduVariant::Prompt(prompt)
                    if !transaction.suspended
                        && prompt.prompt_response() == PromptResponse::KeepAlive =>
                {
                    actions.push(Action::SendKeepAlive {
                        transaction_id,
                        destination_id: transaction.config.transaction_id.source_id,
                        progress: transaction.progress,
                    })?;
                }
                _ => {}
            }
        } else {
            match pdu.variant()? {
                PduVariant::Metadata(meta) => self.handle_metadata(
                    file_store,
                    transaction_id,
                    meta,
                    actions,
                    pdu.large_file_flag(),
                )?,
                PduVariant::Eof(eof) => {
                    actions.push(Action::SendAck {
                        transaction_id,
                        destination_id: pdu.source_entity_id()?,
                        directive_code: DirectiveCode::Eof,
                        condition_code: eof.condition_code()?,
                        transaction_status: TransactionStatus::Unrecognized,
                    })?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn create_transaction(
        &mut self,
        file_store: &mut impl FileStore,
        transaction_id: TransactionId,
        source_file_name: &[u8],
        destination_file_name: &[u8],
        file_size: u64,
        checksum_type: ChecksumType,
        fault_handlers: FaultHandlerSet,
    ) -> Result<(), CfdpError> {
        let config = TransactionConfig {
            transaction_id,
            destination_id: self.id,
            source_file_id: file_store.intern(source_file_name)?,
            destination_file_id: file_store.intern(destination_file_name)?,
            file_size,
            inactivity_timeout_secs: 30,
            checksum_type,
            fault_handlers,
        };
        let transaction = Transaction {
            config,
            state: TransactionState::ReceivingFileData,
            suspended: false,
            progress: 0,
            tracker: SegmentTracker::new(file_size),
            nak_retries: 0,
            nak_limit: 0,
            nak_timeout_secs: 10,
            keep_alive_interval_secs: 60,
            filestore_requests: Vec::new(),
        };
        self.transactions
            .insert(transaction_id, transaction)
            .map_err(|_| CfdpError::Custom("Too many concurrent transactions"))?;
        Ok(())
    }

    fn handle_metadata<'a>(
        &mut self,
        file_store: &mut impl FileStore,
        transaction_id: TransactionId,
        meta: &MetadataPdu,
        actions: &mut Actions<'a>,
        large_file_flag: bool,
    ) -> Result<(), CfdpError> {
        if self.transactions.contains_key(&transaction_id) {
            return Ok(());
        }
        let (source_file_name, destination_file_name, _options) =
            meta.variable_fields(large_file_flag)?;
        let file_size = meta.file_size(large_file_flag)?;
        let checksum_type = meta.checksum_type()?;
        let fault_handlers = meta.fault_handler_overrides(large_file_flag)?;
        self.create_transaction(
            file_store,
            transaction_id,
            source_file_name,
            destination_file_name,
            file_size,
            checksum_type,
            fault_handlers,
        )?;
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        let mut fs_requests = Vec::new();
        for tlv in meta.filestore_requests(large_file_flag)? {
            let req = TlvFilestoreRequest::from_tlv(tlv)?;
            let id1 = file_store.intern(req.first_file_name()?)?;
            let id2 = req.second_file_name()?.map(|name| file_store.intern(name));
            let fs_request = FilestoreRequest {
                action: req.action()?,
                first_file_name: id1,
                second_file_name: id2.transpose()?,
            };
            fs_requests
                .push(fs_request)
                .map_err(|_| CfdpError::Custom("Too many filestore requests in Metadata PDU"))?;
        }
        transaction.filestore_requests = fs_requests;
        if transaction.keep_alive_interval_secs != 0 {
            actions.push(Action::StartTimer {
                timer_type: TimerType::KeepAlive,
                seconds: transaction.keep_alive_interval_secs,
                transaction_id,
            })?;
        }
        actions.push(Action::StartTimer {
            timer_type: TimerType::Inactivity,
            seconds: 30,
            transaction_id,
        })?;
        Ok(())
    }

    fn handle_file_data<'a>(
        &mut self,
        transaction_id: TransactionId,
        large_file_flag: bool,
        file_data: FileDataPdu<'a>,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get(&transaction_id) else {
            return Ok(());
        };
        actions.push(Action::StopTimer {
            timer_type: Some(TimerType::Inactivity),
            transaction_id,
        })?;
        actions.push(Action::WriteFileData {
            transaction_id,
            data: file_data.file_data(large_file_flag)?,
            offset: file_data.offset(large_file_flag)?,
        })?;
        if !transaction.suspended {
            actions.push(Action::StartTimer {
                timer_type: TimerType::Inactivity,
                seconds: transaction.config.inactivity_timeout_secs,
                transaction_id,
            })?;
        }
        Ok(())
    }

    fn handle_eof<'a>(
        &mut self,
        transaction_id: TransactionId,
        pdu: &Pdu,
        eof: &EofPdu,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let condition_code = eof.condition_code()?;

        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            actions.push(Action::SendAck {
                transaction_id,
                destination_id: pdu.source_entity_id()?,
                directive_code: DirectiveCode::Eof,
                condition_code,
                transaction_status: TransactionStatus::Active,
            })?;
            return Ok(());
        };

        // Spec 4.11.2.{6,7} imply timers are suspended/stopped during state changes.
        actions.push(Action::StopTimer {
            timer_type: Some(TimerType::Inactivity),
            transaction_id,
        })?;

        // Per Spec 4.7.2, upon receiving an EOF PDU (which requires an ACK per 4.6.4.3.5 and 4.6.6.1.2),
        // we must immediately issue the Expected Response.
        actions.push(Action::SendAck {
            transaction_id,
            destination_id: transaction.config.transaction_id.source_id,
            directive_code: DirectiveCode::Eof,
            condition_code,
            transaction_status: TransactionStatus::Active,
        })?;

        // --- STEP 2: Decide what to do NEXT based on the condition code ---
        match condition_code {
            ConditionCode::NoError => {
                if transaction.tracker.is_complete() {
                    // We have the whole file. Proceed to checksum verification.
                    // (Spec 4.6.1.2.8)
                    actions.push(Action::VerifyChecksum {
                        transaction_id,
                        checksum_type: transaction.config.checksum_type,
                        expected_checksum: eof.file_checksum(),
                    })?;
                    transaction.state = TransactionState::VerifyingChecksum;
                } else {
                    // We are missing data. In addition to the ACK we just sent,
                    // we must now request the missing segments. (Spec 4.6.4.3.3)
                    self.send_nak(transaction_id, actions)?;
                }
            }
            _ => {
                // This is the CANCELLATION case (e.g., condition is CancelReceived, etc.).
                // Per Spec 4.6.6.1.1, receiving an EOF(cancel) causes a Notice of Completion (Canceled).
                // Our 'handle_fault' function will trigger this termination.
                self.handle_fault(transaction_id, condition_code, actions)?;
            }
        }

        Ok(())
    }

    fn handle_checksum_verified<'a>(
        &mut self,
        transaction_id: TransactionId,
        is_valid: bool,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        if transaction.state != TransactionState::VerifyingChecksum {
            return Ok(());
        }

        if !is_valid {
            self.handle_fault(transaction_id, ConditionCode::FileChecksumFailure, actions)?;
            return Ok(());
        }

        if !transaction.filestore_requests.is_empty() {
            actions.push(Action::ExecuteFilestoreRequests {
                transaction_id,
                requests: transaction.filestore_requests.clone(),
            })?;
            transaction.state = TransactionState::WaitingForFilestoreResponses;
        } else {
            // No filestore requests, so we can finish immediately.
            actions.push(Action::NotifyFileReceived {
                transaction_id,
                file_name: transaction.config.destination_file_id.clone(),
                file_size: transaction.config.file_size,
            })?;
            actions.push(Action::SendAck {
                destination_id: transaction.config.transaction_id.source_id,
                transaction_id,
                directive_code: DirectiveCode::Eof,
                condition_code: ConditionCode::NoError,
                transaction_status: TransactionStatus::Active,
            })?;
            actions.push(Action::SendFinished {
                destination_id: transaction.config.transaction_id.source_id,
                transaction_id,
                condition_code: ConditionCode::NoError,
                filestore_responses: Vec::new(),
            })?;
            self.terminate_transaction(transaction_id, ConditionCode::NoError, actions)?;
        }
        Ok(())
    }

    fn send_nak<'a>(
        &mut self,
        transaction_id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&transaction_id) else {
            return Ok(());
        };
        actions.push(Action::SendNak {
            destination_id: transaction.config.transaction_id.source_id,
            transaction_id,
            start_of_scope: 0,
            end_of_scope: transaction.config.file_size,
        })?;
        transaction.state = TransactionState::WaitingForNakData;
        actions.push(Action::StartTimer {
            timer_type: TimerType::Nak,
            seconds: transaction.nak_timeout_secs,
            transaction_id: transaction.config.transaction_id,
        })?;
        Ok(())
    }

    fn handle_nak_timeout<'a>(
        &mut self,
        id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.get_mut(&id) else {
            return Ok(());
        };
        if transaction.nak_retries >= transaction.nak_limit {
            self.handle_fault(id, ConditionCode::NakLimitReached, actions)?;
        } else {
            transaction.nak_retries += 1;
            self.send_nak(id, actions)?;
        }
        Ok(())
    }

    fn handle_inactivity<'a>(
        &mut self,
        id: TransactionId,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        self.handle_fault(id, ConditionCode::KeepAliveLimitReached, actions)
    }

    fn terminate_transaction<'a>(
        &mut self,
        id: TransactionId,
        condition_code: ConditionCode,
        actions: &mut Actions<'a>,
    ) -> Result<(), CfdpError> {
        let Some(transaction) = self.transactions.remove(&id) else {
            return Ok(());
        };
        actions.push(Action::TerminateTransaction {
            transaction_id: transaction.config.transaction_id,
            condition_code,
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
                self.terminate_transaction(transaction_id, condition_code, actions)?;
            }
            HandlerCode::Ignore => {
                actions.push(Action::NotifyFault {
                    transaction_id,
                    condition_code,
                })?;
            }
            HandlerCode::Suspend => {
                self.handle_suspend_request(transaction_id, actions)?;
            }
            HandlerCode::Abandon => {
                // Abandon is similar to Cancel but implies no further PDUs will be sent.
                // For a receiver, it's effectively the same as Cancel.
                self.terminate_transaction(transaction_id, condition_code, actions)?;
            }
        }
        Ok(())
    }
}
