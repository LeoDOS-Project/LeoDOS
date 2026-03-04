use super::CfdpStream;
use crate::cfdp::api::runner::sender_runner::SenderRunner;
use crate::cfdp::api::Command;
use crate::cfdp::filestore::FileStore;
use crate::cfdp::pdu::EntityId;
use core::net::SocketAddr;

#[cfg(feature = "tokio")]
use tokio::sync::mpsc;

/// Capacity of the internal event queue.
const INTERNAL_EVENT_QUEUE_SIZE: usize = 16;
/// Capacity of the command queue.
const COMMAND_QUEUE_SIZE: usize = 4;

/// High-level sender that manages outgoing CFDP file transfers.
#[cfg(feature = "tokio")]
pub struct CfdpSender {
    /// The underlying sender runner containing the state machine.
    runner: SenderRunner,
    /// Channel for submitting put commands.
    command_tx: mpsc::Sender<Command>,
}

/// High-level sender that manages outgoing CFDP file transfers.
#[cfg(feature = "cfs")]
pub struct CfdpSender<'a> {
    /// The underlying sender runner containing the state machine.
    runner: SenderRunner<'a>,
}

#[cfg(feature = "tokio")]
impl CfdpSender {
    /// Creates a new sender with the given local entity ID.
    pub fn new(my_entity_id: u32) -> Self {
        let (internal_event_tx, internal_event_rx) = mpsc::channel(INTERNAL_EVENT_QUEUE_SIZE);
        let (command_tx, command_rx) = mpsc::channel(COMMAND_QUEUE_SIZE);

        let runner = SenderRunner::new(
            my_entity_id,
            internal_event_tx,
            internal_event_rx,
            command_rx,
        );

        Self {
            runner,
            command_tx,
        }
    }

    /// Initiates a file transfer and returns a stream to await its completion.
    pub async fn put(
        &mut self,
        sender_file_name: &[u8],
        receiver_file_name: &[u8],
        dest_entity_id: EntityId,
        dest_endpoint: SocketAddr,
    ) -> Result<CfdpStream, ()> {
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        let command = Command::Put {
            sender_file_name: heapless::String::try_from(sender_file_name)
                .map_err(|_| ())?,
            receiver_file_name: heapless::String::try_from(receiver_file_name)
                .map_err(|_| ())?,
            dest_entity_id,
            dest_endpoint,
            result_sender: result_tx,
        };

        self.command_tx.send(command).await.map_err(|_| ())?;

        Ok(CfdpStream::new(result_rx))
    }

    /// Runs the sender event loop, processing commands and I/O indefinitely.
    pub async fn run<F, S>(&mut self, filestore: F, socket: &S) -> !
    where
        F: FileStore + Clone + Send + 'static,
        S: super::super::runner::UdpSocket,
    {
        self.runner.run(filestore, socket).await
    }
}

#[cfg(feature = "cfs")]
impl<'a> CfdpSender<'a> {
    /// Creates a new sender with the given local entity ID.
    pub fn new(my_entity_id: u32) -> Self {
        Self {
            runner: SenderRunner::new(my_entity_id),
        }
    }

    /// Initiates a file transfer and returns a stream to await its completion.
    pub async fn put(
        &mut self,
        sender_file_name: &[u8],
        receiver_file_name: &[u8],
        dest_entity_id: EntityId,
        dest_endpoint: SocketAddr,
    ) -> CfdpStream<'a> {
        todo!("Implement put method for cfs feature")
    }

    /// Runs the sender event loop, processing commands and I/O indefinitely.
    pub async fn run<F, S>(&mut self, filestore: F, socket: &S) -> !
    where
        F: FileStore,
        S: super::super::runner::UdpSocket,
    {
        self.runner.run(filestore, socket).await
    }
}
