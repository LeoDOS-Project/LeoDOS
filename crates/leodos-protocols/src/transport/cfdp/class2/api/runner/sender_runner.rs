use super::UdpSocket;
use crate::cfdp::api::{CfdpError, Command, InternalEvent};
use crate::cfdp::filestore::FileStore;
use crate::cfdp::machine::sender::{Action as SenderAction, Event as SenderEvent, SenderMachine};
use crate::cfdp::machine::{
    TimerType, TransactionFinishedParams, TransactionId, FILE_DATA_CHUNK_SIZE,
};
use crate::cfdp::pdu::{parse_pdu, EntityId, EofPdu, FinishedPdu, MetadataPdu, PduHeader};
use heapless::index_map::FnvIndexMap;
use heapless::Vec;
use zerocopy::FromBytes;

#[cfg(feature = "tokio")]
use tokio::sync::{mpsc, oneshot};
#[cfg(feature = "tokio")]
use tokio::time::{sleep, Duration, Instant};

/// Maximum number of concurrent pending timers.
const MAX_PENDING_TIMERS: usize = 16;
/// Maximum number of remote entity endpoint mappings.
const MAX_ENDPOINT_MAP_SIZE: usize = 16;
/// Maximum number of outstanding result channels for pending puts.
const MAX_RESULT_CHANNELS: usize = 16;
/// Maximum number of put requests awaiting file size resolution.
const MAX_PENDING_PUTS: usize = 4;

/// A timer waiting to fire for a specific transaction.
#[derive(Debug, Clone, Copy)]
struct PendingTimer {
    /// The kind of timer (ACK, NAK, inactivity, etc.).
    timer_type: TimerType,
    /// The transaction this timer belongs to.
    transaction_id: TransactionId,
    /// The instant at which this timer expires.
    expires_at: Instant,
}

/// A put request waiting for its file size to be resolved.
#[derive(Debug)]
struct PendingPut {
    /// Temporary ID to correlate the file size response.
    temp_id: u32,
    /// Source file path in the local filestore.
    sender_file_name: heapless::String<256>,
    /// Destination file path in the remote filestore.
    receiver_file_name: heapless::String<256>,
    /// The CFDP entity ID of the destination.
    dest_entity_id: EntityId,
    /// Channel to report the transaction result back to the caller.
    result_sender: oneshot::Sender<Result<TransactionFinishedParams, CfdpError>>,
}

/// Drives the sender state machine with async I/O, timers, and command processing.
#[cfg(feature = "tokio")]
pub(crate) struct SenderRunner {
    /// Local CFDP entity ID.
    my_entity_id: u32,
    /// The synchronous sender state machine.
    source_handler: SenderMachine,
    /// Sender half for posting internal events.
    internal_event_tx: mpsc::Sender<InternalEvent>,
    /// Receiver half for consuming internal events.
    internal_event_rx: mpsc::Receiver<InternalEvent>,
    /// Receiver half for user commands.
    command_rx: mpsc::Receiver<Command>,
    /// Maps transaction IDs to their result channels.
    result_channels: FnvIndexMap<
        TransactionId,
        oneshot::Sender<Result<TransactionFinishedParams, CfdpError>>,
        MAX_RESULT_CHANNELS,
    >,
    /// Maps entity IDs to their network addresses.
    endpoint_map: FnvIndexMap<EntityId, core::net::SocketAddr, MAX_ENDPOINT_MAP_SIZE>,
    /// Active timers awaiting expiry.
    pending_timers: Vec<PendingTimer, MAX_PENDING_TIMERS>,
    /// Put requests awaiting file size resolution.
    pending_puts: Vec<PendingPut, MAX_PENDING_PUTS>,
    /// Counter for generating temporary IDs for pending puts.
    next_temp_id: u32,
    /// The transaction ID most recently created by a put request.
    last_created_tx_id: Option<TransactionId>,
}

/// Drives the sender state machine with async I/O (cFS variant).
#[cfg(feature = "cfs")]
pub(crate) struct SenderRunner<'a> {
    /// Local CFDP entity ID.
    my_entity_id: u32,
    /// The synchronous sender state machine.
    source_handler: SenderMachine,
    /// Phantom data for lifetime parameter.
    _phantom: core::marker::PhantomData<&'a ()>,
}

#[cfg(feature = "tokio")]
impl SenderRunner {
    /// Creates a new `SenderRunner` with the given channels and entity ID.
    pub(crate) fn new(
        my_entity_id: u32,
        internal_event_tx: mpsc::Sender<InternalEvent>,
        internal_event_rx: mpsc::Receiver<InternalEvent>,
        command_rx: mpsc::Receiver<Command>,
    ) -> Self {
        Self {
            my_entity_id,
            source_handler: SenderMachine::new(my_entity_id),
            internal_event_tx,
            internal_event_rx,
            command_rx,
            result_channels: FnvIndexMap::new(),
            endpoint_map: FnvIndexMap::new(),
            pending_timers: Vec::new(),
            pending_puts: Vec::new(),
            next_temp_id: 0,
            last_created_tx_id: None,
        }
    }

    /// Returns the transaction ID of the most recently created transaction.
    fn get_latest_transaction_id(&mut self) -> Option<TransactionId> {
        self.last_created_tx_id
    }

    /// Serializes a PDU header into the buffer and returns the header size.
    fn build_pdu_header(
        &self,
        buffer: &mut [u8],
        pdu_data_len: u16,
        dest_entity_id: EntityId,
        transaction_id: TransactionId,
        is_file_directive: bool,
    ) -> Result<usize, ()> {
        let header_size = core::mem::size_of::<PduHeader>();
        if buffer.len() < header_size {
            return Err(());
        }

        buffer[..header_size].fill(0);

        let header = PduHeader::mut_from_bytes(&mut buffer[..header_size]).map_err(|_| ())?;

        if is_file_directive {
            header.set_pdu_type(crate::cfdp::pdu::PduType::FileDirective);
        } else {
            header.set_pdu_type(crate::cfdp::pdu::PduType::FileData);
        }

        header.data_field_len = pdu_data_len.into();
        header.id_and_seq_num_len = 7;
        header.source_entity_id = transaction_id.source_entity_id;
        header.transaction_seq_num = transaction_id.sequence_number;
        header.dest_entity_id = dest_entity_id;

        Ok(header_size)
    }

    /// Runs the sender event loop, processing commands, packets, and timers.
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
                Some(command) = self.command_rx.recv() => {
                    self.handle_command(command, &filestore).await;
                }
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

    /// Returns the earliest timer deadline, or one hour from now if none.
    fn next_timer_deadline(&self) -> Instant {
        self.pending_timers
            .iter()
            .map(|t| t.expires_at)
            .min()
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600))
    }

    /// Checks all pending timers and fires expired ones.
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
            self.handle_timer_expiry(timer.timer_type, timer.transaction_id, filestore, socket)
                .await;
        }
    }

    /// Processes a user command (e.g., a put request).
    async fn handle_command<F>(&mut self, command: Command, filestore: &F)
    where
        F: FileStore + Clone + Send + 'static,
    {
        match command {
            Command::Put {
                sender_file_name,
                receiver_file_name,
                dest_entity_id,
                dest_endpoint,
                result_sender,
            } => {
                if self
                    .endpoint_map
                    .insert(dest_entity_id, dest_endpoint)
                    .is_err()
                {
                    eprintln!("Warning: Endpoint map full");
                    let _ = result_sender.send(Err(CfdpError::SendError));
                    return;
                }

                let temp_id = self.next_temp_id;
                self.next_temp_id = self.next_temp_id.wrapping_add(1);

                let pending_put = PendingPut {
                    temp_id,
                    sender_file_name: sender_file_name.clone(),
                    receiver_file_name: receiver_file_name.clone(),
                    dest_entity_id,
                    result_sender,
                };

                if self.pending_puts.push(pending_put).is_err() {
                    eprintln!("Warning: Too many pending puts");
                    return;
                }

                let tx = self.internal_event_tx.clone();
                let filestore = filestore.clone();
                let file_name = sender_file_name.clone();

                tokio::spawn(async move {
                    let file_name_str = file_name.as_str();

                    match filestore.file_size(file_name_str).await {
                        Ok(size) => {
                            let event = InternalEvent::FileSizeReady { temp_id, size };
                            if tx.send(event).await.is_err() {
                                eprintln!("Failed to send FileSizeReady event");
                            }
                        }
                        Err(e) => {
                            eprintln!("FileStore error getting size for {:?}: {:?}", file_name, e);
                            let event = InternalEvent::FileStoreGetSizeError { temp_id };
                            if tx.send(event).await.is_err() {
                                eprintln!("Failed to send FileStoreGetSizeError event");
                            }
                        }
                    }
                });
            }
        }
    }

    /// Parses an incoming packet and feeds it to the sender state machine.
    async fn handle_packet<F, S>(
        &mut self,
        buffer: &[u8],
        remote_addr: core::net::SocketAddr,
        filestore: &F,
        socket: &S,
    ) where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        let Some(pdu) = parse_pdu(buffer) else {
            eprintln!("Failed to parse PDU from {}", remote_addr);
            return;
        };

        if pdu.header().source_entity_id.get() != self.my_entity_id {
            return;
        }

        let id = TransactionId {
            source_entity_id: pdu.header().source_entity_id,
            sequence_number: pdu.header().transaction_seq_num,
        };

        let event = SenderEvent::PduReceived {
            pdu,
            transaction_id: id,
        };

        match self.source_handler.handle(event) {
            Ok(actions) => {
                self.process_source_actions(actions, filestore, socket)
                    .await;
            }
            Err(_) => {
                eprintln!("SenderMachine failed to handle PDU");
            }
        }
    }

    /// Feeds a timer expiry event to the sender state machine.
    async fn handle_timer_expiry<F, S>(
        &mut self,
        timer_type: TimerType,
        id: TransactionId,
        filestore: &F,
        socket: &S,
    ) where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        let event = SenderEvent::TimerExpired(timer_type, id);

        match self.source_handler.handle(event) {
            Ok(actions) => {
                self.process_source_actions(actions, filestore, socket)
                    .await;
            }
            Err(_) => {
                eprintln!("SenderMachine failed to handle timer expiry");
            }
        }
    }

    /// Processes an internal event from a spawned I/O task.
    async fn handle_internal_event<F, S>(&mut self, event: InternalEvent, filestore: &F, socket: &S)
    where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        match event {
            InternalEvent::SourceActions(actions) => {
                self.process_source_actions(actions, filestore, socket)
                    .await;
            }
            InternalEvent::FileSizeReady { temp_id, size } => {
                let pending_idx = self.pending_puts.iter().position(|p| p.temp_id == temp_id);
                if let Some(idx) = pending_idx {
                    let pending = self.pending_puts.remove(idx);
                    let event = SenderEvent::PutRequest {
                        source_file_name: pending
                            .sender_file_name
                            .as_bytes()
                            .iter()
                            .copied()
                            .collect(),
                        destination_file_name: pending
                            .receiver_file_name
                            .as_bytes()
                            .iter()
                            .copied()
                            .collect(),
                        dest_entity_id: pending.dest_entity_id,
                        file_size: size,
                    };

                    match self.source_handler.handle(event) {
                        Ok(actions) => {
                            for action in &actions {
                                if let SenderAction::SendMetadata { transaction_id, .. } = action {
                                    self.last_created_tx_id = Some(*transaction_id);
                                    if self
                                        .result_channels
                                        .insert(*transaction_id, pending.result_sender)
                                        .is_err()
                                    {
                                        eprintln!("Warning: Result channels full");
                                    }
                                    break;
                                }
                            }
                            self.process_source_actions(actions, filestore, socket)
                                .await;
                        }
                        Err(_) => {
                            eprintln!("SenderMachine failed to handle PutRequest");
                            let _ = pending.result_sender.send(Err(CfdpError::SendError));
                        }
                    }
                }
            }
            InternalEvent::FileStoreGetSizeError { temp_id } => {
                let pending_idx = self.pending_puts.iter().position(|p| p.temp_id == temp_id);
                if let Some(idx) = pending_idx {
                    let pending = self.pending_puts.remove(idx);
                    let _ = pending.result_sender.send(Err(CfdpError::FileStoreError));
                }
            }
            InternalEvent::FileDataReady { id, data, offset } => {
                let event = SenderEvent::FileDataReady { id, data, offset };
                match self.source_handler.handle(event) {
                    Ok(actions) => {
                        self.process_source_actions(actions, filestore, socket)
                            .await;
                    }
                    Err(_) => {
                        eprintln!("SenderMachine failed to handle FileDataReady");
                    }
                }
            }
            InternalEvent::FileStoreReadError(id) => {
                eprintln!("FileStore read error for transaction {:?}", id);
            }
            InternalEvent::TransactionFinished(params) => {
                if let Some(sender) = self.result_channels.remove(&params.id) {
                    if sender.send(Ok(params)).is_err() {
                        eprintln!("Failed to send transaction result");
                    }
                }

                if let Some(dest_id) = self.source_handler.get_transaction_dest_id(&params.id) {
                    self.endpoint_map.remove(&dest_id);
                }
            }
            _ => {}
        }
    }

    /// Executes sender actions by performing I/O, managing timers, etc.
    async fn process_source_actions<F, S>(
        &mut self,
        actions: Vec<SenderAction, 8>,
        filestore: &F,
        socket: &S,
    ) where
        F: FileStore + Clone + Send + 'static,
        S: UdpSocket,
    {
        for action in actions {
            match action {
                SenderAction::RequestFileData { id, offset, length } => {
                    let file_name = self
                        .source_handler
                        .get_transaction_filestore_name(&id)
                        .unwrap_or_default()
                        .to_string();

                    let tx = self.internal_event_tx.clone();
                    let filestore = filestore.clone();

                    tokio::spawn(async move {
                        let mut buffer = [0u8; FILE_DATA_CHUNK_SIZE];
                        let read_len = length.min(FILE_DATA_CHUNK_SIZE as u64);

                        match filestore
                            .read_chunk(&file_name, offset, read_len, &mut buffer)
                            .await
                        {
                            Ok(bytes_read) => {
                                let event = InternalEvent::FileDataReady {
                                    id,
                                    data: Vec::from_slice(&buffer[..bytes_read])
                                        .unwrap_or_default(),
                                    offset,
                                };

                                if tx.send(event).await.is_err() {
                                    eprintln!("Failed to send FileDataReady event");
                                }
                            }
                            Err(e) => {
                                eprintln!(
                                    "FileStore read error for {:?} at offset {}: {:?}",
                                    file_name, offset, e
                                );
                                let event = InternalEvent::FileStoreReadError(id);
                                if tx.send(event).await.is_err() {
                                    eprintln!("Failed to send FileStoreReadError event");
                                }
                            }
                        }
                    });
                }
                SenderAction::SendMetadata {
                    destination,
                    transaction_id,
                    file_size,
                    source_file_name,
                    dest_file_name,
                } => {
                    if let Some(&endpoint) = self.endpoint_map.get(&destination) {
                        let mut buffer = [0u8; 1024];
                        let header_size = core::mem::size_of::<PduHeader>();

                        match MetadataPdu::builder()
                            .buffer(&mut buffer[header_size..])
                            .segmentation_control(0)
                            .file_size(file_size.into())
                            .source_file_name(&source_file_name)
                            .dest_file_name(&dest_file_name)
                            .build()
                        {
                            Ok(pdu) => {
                                let pdu_len = core::mem::size_of_val(pdu);

                                match self.build_pdu_header(
                                    &mut buffer,
                                    pdu_len as u16,
                                    destination,
                                    transaction_id,
                                    true,
                                ) {
                                    Ok(header_len) => {
                                        let total_len = header_len + pdu_len;
                                        match socket.send_to(&buffer[..total_len], endpoint).await {
                                            Ok(_) => eprintln!("Sent Metadata PDU to {}", endpoint),
                                            Err(_) => eprintln!(
                                                "Failed to send Metadata PDU to {}",
                                                endpoint
                                            ),
                                        }
                                    }
                                    Err(_) => eprintln!("Failed to build PDU header"),
                                }
                            }
                            Err(_) => {
                                eprintln!("Failed to build Metadata PDU");
                            }
                        }
                    } else {
                        eprintln!("No endpoint found for entity {}", destination.get());
                    }
                }
                SenderAction::SendFileData {
                    destination,
                    transaction_id,
                    offset,
                    data,
                } => {
                    if let Some(&endpoint) = self.endpoint_map.get(&destination) {
                        let header_size = core::mem::size_of::<PduHeader>();
                        let pdu_data_len = core::mem::size_of::<u64>() + data.len();
                        let mut buffer = [0u8; FILE_DATA_CHUNK_SIZE + 128];

                        if buffer.len() >= header_size + pdu_data_len {
                            let pdu_buf = &mut buffer[header_size..header_size + pdu_data_len];

                            pdu_buf[0..8].copy_from_slice(&offset.to_be_bytes());
                            pdu_buf[8..8 + data.len()].copy_from_slice(&data);

                            match self.build_pdu_header(
                                &mut buffer,
                                pdu_data_len as u16,
                                destination,
                                transaction_id,
                                false,
                            ) {
                                Ok(header_len) => {
                                    let total_len = header_len + pdu_data_len;
                                    match socket.send_to(&buffer[..total_len], endpoint).await {
                                        Ok(_) => eprintln!(
                                            "Sent FileData PDU to {} (offset: {}, len: {})",
                                            endpoint,
                                            offset,
                                            data.len()
                                        ),
                                        Err(_) => {
                                            eprintln!("Failed to send FileData PDU to {}", endpoint)
                                        }
                                    }
                                }
                                Err(_) => eprintln!("Failed to build PDU header"),
                            }
                        } else {
                            eprintln!("FileData too large for buffer");
                        }
                    } else {
                        eprintln!("No endpoint found for entity {}", destination.get());
                    }
                }
                SenderAction::SendEof {
                    destination,
                    transaction_id,
                    condition_code,
                    file_size,
                } => {
                    if let Some(&endpoint) = self.endpoint_map.get(&destination) {
                        let mut buffer = [0u8; 256];
                        let header_size = core::mem::size_of::<PduHeader>();

                        match EofPdu::builder()
                            .buffer(&mut buffer[header_size..])
                            .condition_code(condition_code)
                            .file_checksum(0.into())
                            .file_size(file_size.into())
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
                                    true,
                                ) {
                                    Ok(header_len) => {
                                        let total_len = header_len + pdu_len;
                                        match socket.send_to(&buffer[..total_len], endpoint).await {
                                            Ok(_) => eprintln!("Sent EOF PDU to {}", endpoint),
                                            Err(_) => {
                                                eprintln!("Failed to send EOF PDU to {}", endpoint)
                                            }
                                        }
                                    }
                                    Err(_) => eprintln!("Failed to build PDU header"),
                                }
                            }
                            Err(_) => {
                                eprintln!("Failed to build EOF PDU");
                            }
                        }
                    } else {
                        eprintln!("No endpoint found for entity {}", destination.get());
                    }
                }
                SenderAction::SendFinished {
                    destination,
                    transaction_id,
                    condition_code,
                } => {
                    if let Some(&endpoint) = self.endpoint_map.get(&destination) {
                        let mut buffer = [0u8; 256];
                        let header_size = core::mem::size_of::<PduHeader>();

                        match FinishedPdu::builder()
                            .buffer(&mut buffer[header_size..])
                            .condition_code(condition_code)
                            .delivery_code(0)
                            .file_status(0)
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
                                    true,
                                ) {
                                    Ok(header_len) => {
                                        let total_len = header_len + pdu_len;
                                        match socket.send_to(&buffer[..total_len], endpoint).await {
                                            Ok(_) => eprintln!("Sent Finished PDU to {}", endpoint),
                                            Err(_) => eprintln!(
                                                "Failed to send Finished PDU to {}",
                                                endpoint
                                            ),
                                        }
                                    }
                                    Err(_) => eprintln!("Failed to build PDU header"),
                                }
                            }
                            Err(_) => {
                                eprintln!("Failed to build Finished PDU");
                            }
                        }
                    } else {
                        eprintln!("No endpoint found for entity {}", destination.get());
                    }
                }
                SenderAction::StartTimer(timer_type, duration_secs, id) => {
                    let timer = PendingTimer {
                        timer_type,
                        transaction_id: id,
                        expires_at: Instant::now() + Duration::from_secs(duration_secs),
                    };
                    if self.pending_timers.push(timer).is_err() {
                        eprintln!(
                            "Warning: Too many pending timers, cannot start timer for {:?}",
                            id
                        );
                    }
                }
                SenderAction::StopTimer(timer_type, id) => {
                    self.pending_timers
                        .retain(|t| !(t.timer_type == timer_type && t.transaction_id == id));
                }
                SenderAction::NotifyTransactionFinished(params) => {
                    let event = InternalEvent::TransactionFinished(params);
                    if self.internal_event_tx.send(event).await.is_err() {
                        eprintln!("Failed to send TransactionFinished event");
                    }
                }
                SenderAction::TransactionComplete(id) => {
                    self.result_channels.remove(&id);
                }
            }
        }
    }
}

#[cfg(feature = "cfs")]
impl<'a> SenderRunner<'a> {
    /// Creates a new `SenderRunner` with the given entity ID.
    pub(crate) fn new(my_entity_id: u32) -> Self {
        Self {
            my_entity_id,
            source_handler: SenderMachine::new(my_entity_id),
            _phantom: core::marker::PhantomData,
        }
    }

    /// Placeholder run loop for the cFS platform (not yet implemented).
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
