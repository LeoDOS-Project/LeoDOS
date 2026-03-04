use super::ReceivedFile;
use crate::cfdp::api::runner::receiver_runner::ReceiverRunner;
use crate::cfdp::filestore::FileStore;

#[cfg(feature = "tokio")]
use tokio::sync::mpsc;

/// Capacity of the internal event queue.
const INTERNAL_EVENT_QUEUE_SIZE: usize = 16;
/// Capacity of the received file notification queue.
const RECEIVED_FILE_QUEUE_SIZE: usize = 8;

/// High-level receiver that manages incoming CFDP file transfers.
#[cfg(feature = "tokio")]
pub struct CfdpReceiver {
    /// The underlying receiver runner containing the state machine.
    runner: ReceiverRunner,
    /// Channel for receiving completed file notifications.
    received_file_rx: mpsc::Receiver<ReceivedFile>,
}

/// High-level receiver that manages incoming CFDP file transfers.
#[cfg(feature = "cfs")]
pub struct CfdpReceiver<'a> {
    /// The underlying receiver runner containing the state machine.
    runner: ReceiverRunner<'a>,
}

#[cfg(feature = "tokio")]
impl CfdpReceiver {
    /// Creates a new receiver with the given local entity ID.
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

    /// Waits for and returns the next successfully received file.
    pub async fn accept(&mut self) -> Option<ReceivedFile> {
        self.received_file_rx.recv().await
    }

    /// Runs the receiver event loop, processing incoming PDUs indefinitely.
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
    /// Creates a new receiver with the given local entity ID.
    pub fn new(my_entity_id: u32) -> Self {
        Self {
            runner: ReceiverRunner::new(my_entity_id),
        }
    }

    /// Waits for and returns the next successfully received file.
    pub async fn accept(&mut self) -> ReceivedFile {
        todo!("Implement accept method for cfs feature")
    }

    /// Runs the receiver event loop, processing incoming PDUs indefinitely.
    pub async fn run<F, S>(&mut self, filestore: F, socket: &S) -> !
    where
        F: FileStore,
        S: super::super::runner::UdpSocket,
    {
        self.runner.run(filestore, socket).await
    }
}
