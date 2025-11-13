use crate::cfdp::filestore::FileStore;
use crate::cfdp::machine::receiver::{Action as ReceiverAction, Event as ReceiverEvent, ReceiverMachine};
use crate::cfdp::machine::{TimerType, TransactionId};
use crate::cfdp::pdu::{parse_pdu, AckPdu, EntityId, NakPdu, PduHeader};
use zerocopy::FromBytes;
use crate::cfdp::api::net::ReceivedFile;
use crate::cfdp::api::InternalEvent;
use super::UdpSocket;
use heapless::index_map::FnvIndexMap;
use heapless::Vec;

#[cfg(feature = "tokio")]
use tokio::sync::mpsc;
#[cfg(feature = "tokio")]
use tokio::time::{sleep, Duration, Instant};

const MAX_PENDING_TIMERS: usize = 16;
const MAX_ENDPOINT_MAP_SIZE: usize = 16;

#[derive(Debug, Clone, Copy)]
struct PendingTimer {
    timer_type: TimerType,
    transaction_id: TransactionId,
    expires_at: Instant,
}

#[cfg(feature = "tokio")]
pub(crate) struct ReceiverRunner {
    my_entity_id: u32,
    dest_handler: ReceiverMachine,
    internal_event_tx: mpsc::Sender<InternalEvent>,
    internal_event_rx: mpsc::Receiver<InternalEvent>,
    received_file_tx: mpsc::Sender<ReceivedFile>,
    endpoint_map: FnvIndexMap<EntityId, core::net::SocketAddr, MAX_ENDPOINT_MAP_SIZE>,
    pending_timers: Vec<PendingTimer, MAX_PENDING_TIMERS>,
}

#[cfg(feature = "cfs")]
pub(crate) struct ReceiverRunner<'a> {
    my_entity_id: u32,
    dest_handler: ReceiverMachine,
    _phantom: core::marker::PhantomData<&'a ()>,
}

#[cfg(feature = "tokio")]
impl ReceiverRunner {
    pub(crate) fn new(
        my_entity_id: u32,
        internal_event_tx: mpsc::Sender<InternalEvent>,
        internal_event_rx: mpsc::Receiver<InternalEvent>,
        received_file_tx: mpsc::Sender<ReceivedFile>,
    ) -> Self {
        Self {
            my_entity_id,
            dest_handler: ReceiverMachine::new(my_entity_id),
            internal_event_tx,
            internal_event_rx,
            received_file_tx,
            endpoint_map: FnvIndexMap::new(),
            pending_timers: Vec::new(),
        }
    }

    fn build_pdu_header(
        &self,
        buffer: &mut [u8],
        pdu_data_len: u16,
        dest_entity_id: EntityId,
        transaction_id: TransactionId,
    ) -> Result<usize, ()> {
        let header_size = core::mem::size_of::<PduHeader>();
        if buffer.len() < header_size {
            return Err(());
        }

        buffer[..header_size].fill(0);

        let header = PduHeader::mut_from_bytes(&mut buffer[..header_size])
            .map_err(|_| ())?;

        header.set_pdu_type(crate::cfdp::pdu::PduType::FileDirective);

        header.data_field_len = pdu_data_len.into();
        header.id_and_seq_num_len = 7;
        header.source_entity_id = self.my_entity_id.into();
        header.transaction_seq_num = transaction_id.sequence_number;
        header.dest_entity_id = dest_entity_id;

        Ok(header_size)
    }

    pub(crate) async fn run<F, S>(&mut self, filestore: F, socket: &S) -> !
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        let mut rx_buf = [0u8; 4096];

        loop {
            while let Ok(event) = self.internal_event_rx.try_recv() {
                self.handle_internal_event(event, &filestore, socket).await;
            }

            self.check_timers(&filestore, socket).await;

            let next_timer = self.next_timer_deadline();

            tokio::select! {
                Ok((len, remote_addr)) = socket.recv_from(&mut rx_buf) => {
                    self.handle_packet(&rx_buf[..len], remote_addr, &filestore, socket).await;
                }
                _ = sleep(next_timer.duration_since(Instant::now())), if next_timer > Instant::now() => {
                }
                Some(event) = self.internal_event_rx.recv() => {
                    self.handle_internal_event(event, &filestore, socket).await;
                }
            }
        }
    }

    fn next_timer_deadline(&self) -> Instant {
        self.pending_timers
            .iter()
            .map(|t| t.expires_at)
            .min()
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600))
    }

    async fn check_timers<F, S>(&mut self, filestore: &F, socket: &S)
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        let now = Instant::now();
        let mut expired = Vec::<PendingTimer, MAX_PENDING_TIMERS>::new();

        self.pending_timers.retain(|timer| {
            if timer.expires_at <= now {
                if expired.push(*timer).is_err() {
                    eprintln!("Warning: Too many expired timers, dropping timer");
                }
                false
            } else {
                true
            }
        });

        for timer in expired {
            self.handle_timer_expiry(timer.timer_type, timer.transaction_id, filestore, socket).await;
        }
    }

    async fn handle_packet<F, S>(&mut self, buffer: &[u8], remote_addr: core::net::SocketAddr, filestore: &F, socket: &S)
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        let Some(pdu) = parse_pdu(buffer) else {
            eprintln!("Failed to parse PDU from {}", remote_addr);
            return;
        };

        if pdu.header().dest_entity_id.get() != self.my_entity_id {
            return;
        }

        let id = TransactionId {
            source_entity_id: pdu.header().source_entity_id,
            sequence_number: pdu.header().transaction_seq_num,
        };

        if self.endpoint_map.insert(id.source_entity_id, remote_addr).is_err() {
            eprintln!("Warning: Endpoint map full, cannot track endpoint for entity {}", id.source_entity_id.get());
        }

        let event = ReceiverEvent::PduReceived {
            pdu,
            transaction_id: id,
        };

        match self.dest_handler.handle(event) {
            Ok(actions) => {
                self.process_destination_actions(actions, filestore, socket).await;
            }
            Err(_) => {
                eprintln!("ReceiverMachine failed to handle PDU");
            }
        }
    }

    async fn handle_timer_expiry<F, S>(&mut self, timer_type: TimerType, id: TransactionId, filestore: &F, socket: &S)
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        let event = ReceiverEvent::TimerExpired(timer_type, id);

        match self.dest_handler.handle(event) {
            Ok(actions) => {
                self.process_destination_actions(actions, filestore, socket).await;
            }
            Err(_) => {
                eprintln!("ReceiverMachine failed to handle timer expiry");
            }
        }
    }

    async fn handle_internal_event<F, S>(&mut self, event: InternalEvent, filestore: &F, socket: &S)
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        match event {
            InternalEvent::DestActions(actions) => {
                self.process_destination_actions(actions, filestore, socket).await;
            }
            InternalEvent::FileDataWritten { id, offset, len } => {
                let event = ReceiverEvent::FileDataWritten { id, offset, len };
                match self.dest_handler.handle(event) {
                    Ok(actions) => {
                        self.process_destination_actions(actions, filestore, socket).await;
                    }
                    Err(_) => {
                        eprintln!("ReceiverMachine failed to handle FileDataWritten");
                    }
                }
            }
            InternalEvent::FileStoreWriteError(id) => {
                eprintln!("FileStore write error for transaction {:?}", id);
            }
            _ => {}
        }
    }

    async fn process_destination_actions<F, S>(&mut self, actions: Vec<ReceiverAction, 8>, filestore: &F, socket: &S)
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        for action in actions {
            match action {
                ReceiverAction::WriteFileData { id, data, offset } => {
                    let file_name = self.dest_handler
                        .get_transaction_filestore_name(&id)
                        .unwrap_or_default()
                        .to_string();

                    let tx = self.internal_event_tx.clone();
                    let mut filestore = filestore.clone();
                    let data_len = data.len();

                    tokio::spawn(async move {
                        match filestore.write_chunk(&file_name, offset, &data).await {
                            Ok(_) => {
                                let result = InternalEvent::FileDataWritten {
                                    id,
                                    offset,
                                    len: data_len,
                                };

                                if tx.send(result).await.is_err() {
                                    eprintln!("Failed to send FileDataWritten event");
                                }
                            }
                            Err(e) => {
                                eprintln!("FileStore write error for {:?} at offset {}: {:?}", file_name, offset, e);
                                let result = InternalEvent::FileStoreWriteError(id);
                                if tx.send(result).await.is_err() {
                                    eprintln!("Failed to send FileStoreWriteError event");
                                }
                            }
                        }
                    });
                }
                ReceiverAction::SendAck {
                    destination,
                    transaction_id,
                    directive_code,
                    condition_code,
                } => {
                    if let Some(&endpoint) = self.endpoint_map.get(&destination) {
                        let mut buffer = [0u8; 256];
                        let header_size = core::mem::size_of::<PduHeader>();

                        match AckPdu::builder()
                            .buffer(&mut buffer[header_size..])
                            .directive_subtype_code(directive_code)
                            .condition_code(condition_code)
                            .transaction_status(2)
                            .transaction_id(transaction_id)
                            .dest_entity_id(destination)
                            .build()
                        {
                            Ok(pdu) => {
                                let pdu_len = core::mem::size_of_val(pdu);

                                match self.build_pdu_header(
                                    &mut buffer,
                                    pdu_len as u16,
                                    destination,
                                    transaction_id,
                                ) {
                                    Ok(header_len) => {
                                        let total_len = header_len + pdu_len;
                                        match socket.send_to(&buffer[..total_len], endpoint).await {
                                            Ok(_) => eprintln!("Sent ACK PDU to {}", endpoint),
                                            Err(_) => eprintln!("Failed to send ACK PDU to {}", endpoint),
                                        }
                                    }
                                    Err(_) => eprintln!("Failed to build PDU header"),
                                }
                            }
                            Err(_) => {
                                eprintln!("Failed to build ACK PDU");
                            }
                        }
                    } else {
                        eprintln!("No endpoint found for entity {}", destination.get());
                    }
                }
                ReceiverAction::SendNak {
                    destination,
                    transaction_id,
                    start_of_scope,
                    end_of_scope,
                    segment_requests,
                } => {
                    if let Some(&endpoint) = self.endpoint_map.get(&destination) {
                        let mut buffer = [0u8; 1024];
                        let header_size = core::mem::size_of::<PduHeader>();

                        let segments: &[(zerocopy::byteorder::network_endian::U64, zerocopy::byteorder::network_endian::U64)] = &segment_requests;

                        match NakPdu::builder()
                            .buffer(&mut buffer[header_size..])
                            .start_of_scope(start_of_scope.into())
                            .end_of_scope(end_of_scope.into())
                            .segment_requests(segments)
                            .build()
                        {
                            Ok(pdu) => {
                                let pdu_len = core::mem::size_of_val(pdu);

                                match self.build_pdu_header(
                                    &mut buffer,
                                    pdu_len as u16,
                                    destination,
                                    transaction_id,
                                ) {
                                    Ok(header_len) => {
                                        let total_len = header_len + pdu_len;
                                        match socket.send_to(&buffer[..total_len], endpoint).await {
                                            Ok(_) => eprintln!("Sent NAK PDU to {}", endpoint),
                                            Err(_) => eprintln!("Failed to send NAK PDU to {}", endpoint),
                                        }
                                    }
                                    Err(_) => eprintln!("Failed to build PDU header"),
                                }
                            }
                            Err(_) => {
                                eprintln!("Failed to build NAK PDU");
                            }
                        }
                    } else {
                        eprintln!("No endpoint found for entity {}", destination.get());
                    }
                }
                ReceiverAction::StartTimer(timer_type, duration_secs, id) => {
                    let timer = PendingTimer {
                        timer_type,
                        transaction_id: id,
                        expires_at: Instant::now() + Duration::from_secs(duration_secs),
                    };
                    if self.pending_timers.push(timer).is_err() {
                        eprintln!("Warning: Too many pending timers, cannot start timer for {:?}", id);
                    }
                }
                ReceiverAction::StopTimer(timer_type, id) => {
                    self.pending_timers.retain(|t| {
                        !(t.timer_type == timer_type && t.transaction_id == id)
                    });
                }
                ReceiverAction::NotifyFileReceived(params) => {
                    let file_name_str = core::str::from_utf8(&params.file_name)
                        .unwrap_or_default();

                    let received_file = ReceivedFile {
                        file_name: heapless::String::try_from(file_name_str)
                            .unwrap_or_default(),
                        length: params.length,
                        remote_entity_id: params.id.source_entity_id,
                    };

                    if self.received_file_tx.send(received_file).await.is_err() {
                        eprintln!("Failed to send received file notification");
                    }
                }
                ReceiverAction::NotifyTransactionFinished(params) => {
                    eprintln!("Transaction {:?} finished with condition {:?}",
                        params.id, params.condition_code);
                }
                ReceiverAction::TransactionComplete(id) => {
                    self.endpoint_map.remove(&id.source_entity_id);
                }
            }
        }
    }
}

#[cfg(feature = "cfs")]
impl<'a> ReceiverRunner<'a> {
    pub(crate) fn new(my_entity_id: u32) -> Self {
        Self {
            my_entity_id,
            dest_handler: ReceiverMachine::new(my_entity_id),
            _phantom: core::marker::PhantomData,
        }
    }

    pub(crate) async fn run<F, S>(&mut self, _filestore: F, _socket: &S) -> !
    where
        F: FileStore,
        S: UdpSocket,
    {
        loop {
            core::future::pending::<()>().await;
        }
    }
}
