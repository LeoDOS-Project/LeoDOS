use crate::cfdp::filestore::FileStore;
use crate::cfdp::high_level::user::CfdpUser;
use crate::cfdp::high_level::CfdpError;
use crate::cfdp::high_level::Command;
use crate::cfdp::high_level::CommandResult;
use crate::cfdp::high_level::InternalEvent;
use crate::cfdp::machine::receiver::Action as ReceiverAction;
use crate::cfdp::machine::receiver::Event as ReceiverEvent;
use crate::cfdp::machine::receiver::ReceiverMachine;
use crate::cfdp::machine::sender::Action as SenderAction;
use crate::cfdp::machine::sender::Event as SenderEvent;
use crate::cfdp::machine::sender::SenderMachine;
use crate::cfdp::machine::TimerType;
use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::machine::TransactionId;
use crate::cfdp::machine::FILE_DATA_CHUNK_SIZE;
use crate::cfdp::pdu::parse_pdu;
use crate::cfdp::pdu::AckPdu;
use crate::cfdp::pdu::EntityId;
use crate::cfdp::pdu::EofPdu;
use crate::cfdp::pdu::PduHeader;
use heapless::Vec;
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::sleep;
use zerocopy::FromZeros;
use zerocopy::IntoBytes;

/// A background task that runs the CFDP protocol logic.
pub struct Runner<F: FileStore, U: CfdpUser> {
    pub(crate) my_entity_id: u32,
    pub(crate) socket: tokio::net::UdpSocket,
    pub(crate) filestore: F,
    pub(crate) source_handler: SenderMachine,
    pub(crate) dest_handler: ReceiverMachine,
    pub(crate) user_callbacks: U,
    pub(crate) command_receiver: mpsc::Receiver<Command>,
    pub(crate) timer_sender: mpsc::Sender<(TimerType, TransactionId)>,
    pub(crate) timer_receiver: mpsc::Receiver<(TimerType, TransactionId)>,
    pub(crate) internal_event_sender: mpsc::Sender<InternalEvent>,
    pub(crate) internal_event_receiver: mpsc::Receiver<InternalEvent>,
    pub(crate) result_channels: HashMap<TransactionId, oneshot::Sender<CommandResult>>,
    pub(crate) endpoint_map: HashMap<EntityId, std::net::SocketAddr>,
}

// NOTE: This is the full, new implementation block for the Runner.
// Replace the existing one entirely.
impl<F, U> Runner<F, U>
where
    F: FileStore + Send + Sync + Clone + 'static,
    U: CfdpUser + Send + 'static,
{
    pub async fn run(mut self) {
        let mut rx_buf = [0u8; 4096];
        loop {
            tokio::select! {
                biased;
                Some(internal_event) = self.internal_event_receiver.recv() => {
                    match internal_event {
                        InternalEvent::SourceActions(actions) => self.process_source_actions(actions).await,
                        InternalEvent::DestActions(actions) => self.process_destination_actions(actions).await,
                    }
                }
                Some(command) = self.command_receiver.recv() => {
                    self.handle_command(command).await;
                }
                Ok((len, remote_addr)) = self.socket.recv_from(&mut rx_buf) => {
                    self.handle_packet(&rx_buf[..len], remote_addr).await;
                }
                Some((timer_type, id)) = self.timer_receiver.recv() => {
                    self.handle_timer(timer_type, id).await;
                }
            }
        }
    }

    // Command, Packet, and Timer handlers remain largely the same
    async fn handle_command(&mut self, command: Command) {
        match command {
            Command::Put {
                sender_file_name: source_file_name,
                receiver_file_name: destination_file_name,
                dest_entity_id,
                dest_endpoint,
                result_sender,
            } => {
                let file_size = match self
                    .filestore
                    .file_size(core::str::from_utf8(&source_file_name).unwrap())
                    .await
                {
                    Ok(size) => size,
                    Err(e) => {
                        eprintln!(
                            "Filestore error getting size for {:?}: {:?}",
                            source_file_name, e
                        );
                        let _ = result_sender.send(Err(CfdpError::FileStoreError));
                        return;
                    }
                };

                self.endpoint_map.insert(dest_entity_id, dest_endpoint);
                let event = SenderEvent::PutRequest {
                    source_file_name,
                    destination_file_name,
                    dest_entity_id,
                    file_size,
                };

                match self.source_handler.handle(event) {
                    Ok(actions) => {
                        for action in &actions {
                            if let SenderAction::SendMetadata { transaction_id, .. } = action {
                                self.result_channels.insert(*transaction_id, result_sender);
                                break;
                            }
                        }
                        self.process_source_actions(actions).await;
                    }
                    Err(_) => {
                        eprintln!("SenderMachine failed to handle PutRequest.");
                        let _ = result_sender.send(Err(CfdpError::SendError));
                    }
                }
            }
        }
    }
    async fn handle_packet(&mut self, buffer: &[u8], remote_addr: std::net::SocketAddr) {
        let Some((header, pdu)) = parse_pdu(buffer) else {
            return;
        };
        let id = TransactionId {
            source_entity_id: header.source_entity_id,
            sequence_number: header.transaction_seq_num,
        };
        self.endpoint_map.insert(id.source_entity_id, remote_addr);

        if header.dest_entity_id.get() == self.my_entity_id {
            if let Ok(actions) = self.dest_handler.handle(ReceiverEvent::PduReceived {
                pdu,
                transaction_id: id,
            }) {
                self.process_destination_actions(actions).await;
            }
        } else if header.source_entity_id.get() == self.my_entity_id {
            if let Ok(actions) = self.source_handler.handle(SenderEvent::PduReceived {
                pdu,
                transaction_id: id,
            }) {
                self.process_source_actions(actions).await;
            }
        }
    }
    async fn handle_timer(&mut self, timer_type: TimerType, id: TransactionId) {
        if id.source_entity_id.get() == self.my_entity_id {
            if let Ok(actions) = self
                .source_handler
                .handle(SenderEvent::TimerExpired(timer_type, id))
            {
                self.process_source_actions(actions).await;
            }
        } else {
            if let Ok(actions) = self
                .dest_handler
                .handle(ReceiverEvent::TimerExpired(timer_type, id))
            {
                self.process_destination_actions(actions).await;
            }
        }
    }

    // Action processors are now modified to spawn tasks for I/O
    async fn process_source_actions(&mut self, actions: Vec<SenderAction, 8>) {
        for action in actions {
            match action {
                SenderAction::RequestFileData { id, offset, length } => {
                    // This is an I/O action, so we spawn it.
                    let filestore = self.filestore.clone();
                    let mut source_handler = self.source_handler.clone();
                    let internal_sender = self.internal_event_sender.clone();
                    let name = self
                        .source_handler
                        .get_transaction_filestore_name(&id)
                        .unwrap_or_default()
                        .to_string();

                    tokio::spawn(async move {
                        let mut read_buffer = [0u8; FILE_DATA_CHUNK_SIZE];
                        let slice_len = length.min(FILE_DATA_CHUNK_SIZE as u64) as usize;

                        match filestore
                            .read_chunk(&name, offset, length, &mut read_buffer[..slice_len])
                            .await
                        {
                            Ok(bytes_read) => {
                                let data = Vec::from_slice(&read_buffer[..bytes_read]).unwrap();
                                let event = SenderEvent::FileDataReady { id, data, offset };
                                if let Ok(next_actions) = source_handler.handle(event) {
                                    // Send the resulting actions back to the main loop
                                    let _ = internal_sender
                                        .send(InternalEvent::SourceActions(next_actions))
                                        .await;
                                }
                            }
                            Err(e) => {
                                eprintln!("Filestore read error for transaction {:?}: {:?}", id, e)
                            }
                        }
                    });
                }
                _ => self.process_immediate_source_action(action).await,
            }
        }
    }

    async fn process_destination_actions(&mut self, actions: Vec<ReceiverAction, 8>) {
        for action in actions {
            match action {
                ReceiverAction::WriteFileData { id, data, offset } => {
                    // This is an I/O action, so we spawn it.
                    let mut filestore = self.filestore.clone();
                    let mut dest_handler = self.dest_handler.clone();
                    let internal_sender = self.internal_event_sender.clone();
                    let name = self
                        .dest_handler
                        .get_transaction_filestore_name(&id)
                        .unwrap_or_default()
                        .to_string();

                    tokio::spawn(async move {
                        if filestore.write_chunk(&name, offset, &data).await.is_ok() {
                            let event = ReceiverEvent::FileDataWritten {
                                id,
                                offset,
                                len: data.len(),
                            };
                            if let Ok(next_actions) = dest_handler.handle(event) {
                                // Send the resulting actions back to the main loop
                                let _ = internal_sender
                                    .send(InternalEvent::DestActions(next_actions))
                                    .await;
                            }
                        } else {
                            eprintln!("Filestore write error for transaction {:?}", id);
                            // In a real system, you might want to send a fault event back here.
                        }
                    });
                }
                _ => self.process_immediate_destination_action(action).await,
            }
        }
    }

    // Helper for non-I/O actions that can be executed immediately
    async fn process_immediate_source_action(&mut self, action: SenderAction) {
        match action {
            SenderAction::SendMetadata {
                destination,
                transaction_id,
                file_size,
                source_file_name,
                dest_file_name,
            } => {
                let mut buffer = [0u8; 1024];
                let pdu = crate::cfdp::pdu::MetadataPdu::builder()
                    .buffer(&mut buffer)
                    .segmentation_control(0)
                    .file_size(file_size.into())
                    .source_file_name(&source_file_name)
                    .dest_file_name(&dest_file_name)
                    .build();
                let data_len = match pdu {
                    Ok(pdu) => pdu.as_bytes().len(),
                    Err(_) => {
                        eprintln!("Failed to build MetadataPdu.");
                        return;
                    }
                };
                self.send_pdu(destination, transaction_id, &buffer[..data_len], true)
                    .await;
            }
            SenderAction::SendFileData {
                destination,
                transaction_id,
                offset,
                data,
            } => {
                let mut buffer = [0u8; FILE_DATA_CHUNK_SIZE + 64];
                offset.write_to_prefix(&mut buffer).unwrap();
                let data_start = core::mem::size_of::<u64>();
                buffer[data_start..data_start + data.len()].copy_from_slice(&data);
                let data_len = core::mem::size_of::<u64>() + data.len();
                self.send_pdu(destination, transaction_id, &buffer[..data_len], false)
                    .await;
            }
            SenderAction::SendEof {
                destination,
                transaction_id,
                condition_code,
                file_size,
            } => {
                let mut buffer = [0u8; 128];
                let pdu = EofPdu::builder()
                    .buffer(&mut buffer)
                    .condition_code(condition_code)
                    .file_checksum(0.into())
                    .file_size(file_size.into())
                    .build()
                    .unwrap();
                self.send_pdu(destination, transaction_id, pdu.as_bytes(), true)
                    .await;
            }
            SenderAction::SendFinished {
                destination,
                transaction_id,
                condition_code,
            } => {
                let mut buffer = [0u8; 128];
                let pdu = crate::cfdp::pdu::FinishedPdu::builder()
                    .buffer(&mut buffer)
                    .condition_code(condition_code)
                    .delivery_code(0)
                    .file_status(0)
                    .build()
                    .unwrap();
                self.send_pdu(destination, transaction_id, pdu.as_bytes(), true)
                    .await;
            }
            SenderAction::StartTimer(timer_type, duration, id) => {
                let sender = self.timer_sender.clone();
                tokio::spawn(async move {
                    sleep(Duration::from_secs(duration)).await;
                    let _ = sender.send((timer_type, id)).await;
                });
            }
            SenderAction::StopTimer(_, _) => {}
            SenderAction::NotifyTransactionFinished(params) => {
                self.finish_transaction(params).await;
            }
            SenderAction::TransactionComplete(id) => {
                self.result_channels.remove(&id);
            }
            SenderAction::RequestFileData { .. } => {
                unreachable!("Should be handled in process_source_actions")
            }
        }
    }

    async fn process_immediate_destination_action(&mut self, action: ReceiverAction) {
        match action {
            ReceiverAction::SendAck {
                destination,
                transaction_id,
                directive_code,
                condition_code,
            } => {
                let mut buffer = [0u8; 128];
                let pdu = AckPdu::builder()
                    .buffer(&mut buffer)
                    .directive_subtype_code(directive_code)
                    .condition_code(condition_code)
                    .transaction_status(2)
                    .build()
                    .unwrap();
                self.send_pdu(destination, transaction_id, pdu.as_bytes(), true)
                    .await;
            }
            ReceiverAction::SendNak {
                destination,
                transaction_id,
                start_of_scope,
                end_of_scope,
                segment_requests,
            } => {
                let mut buffer = [0u8; 1024];
                let pdu = crate::cfdp::pdu::NakPdu::builder()
                    .buffer(&mut buffer)
                    .start_of_scope(start_of_scope.into())
                    .end_of_scope(end_of_scope.into())
                    .segment_requests(&segment_requests)
                    .build();
                let data_len = match pdu {
                    Ok(pdu) => pdu.as_bytes().len(),
                    Err(_) => {
                        eprintln!("Failed to build NakPdu.");
                        return;
                    }
                };
                self.send_pdu(destination, transaction_id, &buffer[..data_len], true)
                    .await;
            }
            ReceiverAction::StartTimer(timer_type, duration, id) => {
                let sender = self.timer_sender.clone();
                tokio::spawn(async move {
                    sleep(Duration::from_secs(duration)).await;
                    let _ = sender.send((timer_type, id)).await;
                });
            }
            ReceiverAction::StopTimer(_, _) => {}
            ReceiverAction::NotifyFileReceived(params) => {
                self.user_callbacks.on_file_received(params).await;
            }
            ReceiverAction::NotifyTransactionFinished(params) => {
                self.finish_transaction(params).await;
            }
            ReceiverAction::TransactionComplete(_id) => {}
            ReceiverAction::WriteFileData { .. } => {
                unreachable!("Should be handled in process_destination_actions")
            }
        }
    }

    async fn send_pdu(
        &mut self,
        destination: EntityId,
        id: TransactionId,
        pdu_data: &[u8],
        is_directive: bool,
    ) {
        let mut buffer = [0u8; FILE_DATA_CHUNK_SIZE + 64];
        let header_len = core::mem::size_of::<PduHeader>();

        let mut header = PduHeader::new_zeroed();
        header.source_entity_id = self.my_entity_id.into();
        header.dest_entity_id = destination;
        header.transaction_seq_num = id.sequence_number;
        if is_directive {
            header.set_pdu_type(crate::cfdp::pdu::PduType::FileDirective);
        } else {
            header.set_pdu_type(crate::cfdp::pdu::PduType::FileData);
        }
        header.data_field_len.set(pdu_data.len() as u16);
        header.write_to_prefix(&mut buffer).unwrap();

        buffer[header_len..header_len + pdu_data.len()].copy_from_slice(pdu_data);
        let total_len = header_len + pdu_data.len();

        if let Some(endpoint) = self.endpoint_map.get(&destination) {
            if let Err(e) = self.socket.send_to(&buffer[..total_len], *endpoint).await {
                eprintln!("Socket send error for Entity ID {destination:?}: {e:?}");
            }
        } else {
            eprintln!("Cannot send PDU: No endpoint found for Entity ID {destination:?}");
        }
    }

    async fn finish_transaction(&mut self, params: TransactionFinishedParams) {
        self.user_callbacks.on_transaction_finished(params).await;
        if let Some(channel) = self.result_channels.remove(&params.id) {
            let _ = channel.send(Ok(params));
        }
        if params.id.source_entity_id.get() == self.my_entity_id {
            if let Some(dest_id) = self.source_handler.get_transaction_dest_id(&params.id) {
                self.endpoint_map.remove(&dest_id);
            }
        } else {
            self.endpoint_map.remove(&params.id.source_entity_id);
        }
    }
}
