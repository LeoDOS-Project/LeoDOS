use super::ReceivedFile;
use crate::cfdp::api::runner::receiver_runner::ReceiverRunner;
use crate::cfdp::filestore::FileStore;

#[cfg(feature = "tokio")]
use tokio::sync::mpsc;

const INTERNAL_EVENT_QUEUE_SIZE: usize = 16;
const RECEIVED_FILE_QUEUE_SIZE: usize = 8;

#[cfg(feature = "tokio")]
pub struct CfdpReceiver {
    runner: ReceiverRunner,
    received_file_rx: mpsc::Receiver<ReceivedFile>,
}

#[cfg(feature = "cfs")]
pub struct CfdpReceiver<'a> {
    runner: ReceiverRunner<'a>,
}

#[cfg(feature = "tokio")]
impl CfdpReceiver {
    pub fn new(my_entity_id: u32) -> Self {
        let (internal_event_tx, internal_event_rx) = mpsc::channel(INTERNAL_EVENT_QUEUE_SIZE);
        let (received_file_tx, received_file_rx) = mpsc::channel(RECEIVED_FILE_QUEUE_SIZE);

        let runner = ReceiverRunner::new(
            my_entity_id,
            internal_event_tx,
            internal_event_rx,
            received_file_tx,
        );

        Self {
            runner,
            received_file_rx,
        }
    }

    pub async fn accept(&mut self) -> Option<ReceivedFile> {
        self.received_file_rx.recv().await
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
impl<'a> CfdpReceiver<'a> {
    pub fn new(my_entity_id: u32) -> Self {
        Self {
            runner: ReceiverRunner::new(my_entity_id),
        }
    }

    pub async fn accept(&mut self) -> ReceivedFile {
        todo!("Implement accept method for cfs feature")
    }

    pub async fn run<F, S>(&mut self, filestore: F, socket: &S) -> !
    where
        F: FileStore,
        S: super::super::runner::UdpSocket,
    {
        self.runner.run(filestore, socket).await
    }
}
