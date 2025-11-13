use super::CfdpStream;
use crate::cfdp::api::runner::sender_runner::SenderRunner;
use crate::cfdp::api::Command;
use crate::cfdp::filestore::FileStore;
use crate::cfdp::pdu::EntityId;
use core::net::SocketAddr;

#[cfg(feature = "tokio")]
use tokio::sync::mpsc;

const INTERNAL_EVENT_QUEUE_SIZE: usize = 16;
const COMMAND_QUEUE_SIZE: usize = 4;

#[cfg(feature = "tokio")]
pub struct CfdpSender {
    runner: SenderRunner,
    command_tx: mpsc::Sender<Command>,
}

#[cfg(feature = "cfs")]
pub struct CfdpSender<'a> {
    runner: SenderRunner<'a>,
}

#[cfg(feature = "tokio")]
impl CfdpSender {
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
    pub fn new(my_entity_id: u32) -> Self {
        Self {
            runner: SenderRunner::new(my_entity_id),
        }
    }

    pub async fn put(
        &mut self,
        sender_file_name: &[u8],
        receiver_file_name: &[u8],
        dest_entity_id: EntityId,
        dest_endpoint: SocketAddr,
    ) -> CfdpStream<'a> {
        todo!("Implement put method for cfs feature")
    }

    pub async fn run<F, S>(&mut self, filestore: F, socket: &S) -> !
    where
        F: FileStore,
        S: super::super::runner::UdpSocket,
    {
        self.runner.run(filestore, socket).await
    }
}
