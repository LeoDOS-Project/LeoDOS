use std::collections::HashMap;

use tokio::sync::mpsc;

use crate::cfdp::filestore::FileStore;
use crate::cfdp::high_level::connection::CfdpConnection;
use crate::cfdp::high_level::runner::Runner;
use crate::cfdp::high_level::user::CfdpUser;
use crate::cfdp::high_level::Command;
use crate::cfdp::machine::ReceiverMachine;
use crate::cfdp::machine::SenderMachine;

/// The main user-facing entry point for the CFDP API. Create this once.
pub struct CfdpEntity {
    command_sender: mpsc::Sender<Command>,
}

impl CfdpEntity {
    /// Creates a new `CfdpEntity` and the associated `Runner` task.
    ///
    /// The returned `Runner` struct must be spawned on a Tokio executor.
    pub fn new<F, U>(
        my_entity_id: u32,
        socket: tokio::net::UdpSocket,
        filestore: F,
        user_callbacks: U,
    ) -> (Self, Runner<F, U>)
    where
        // The Clone bound is still needed for FileStore, SenderMachine, and ReceiverMachine
        F: FileStore + Send + Sync + Clone + 'static,
        U: CfdpUser + Send + 'static, // CfdpUser does NOT need to be Clone
    {
        let (command_sender, command_receiver) = mpsc::channel(32);
        let (timer_sender, timer_receiver) = mpsc::channel(32);
        let (internal_event_sender, internal_event_receiver) = mpsc::channel(32);

        let entity = Self { command_sender };
        let runner = Runner {
            my_entity_id,
            socket,
            filestore,
            source_handler: SenderMachine::new(my_entity_id),
            dest_handler: ReceiverMachine::new(my_entity_id), // Don't forget this ID
            user_callbacks,
            command_receiver,
            timer_sender,
            timer_receiver,
            internal_event_sender,
            internal_event_receiver,
            result_channels: HashMap::new(),
            endpoint_map: HashMap::new(),
        };
        (entity, runner)
    }

    /// Creates a handle for communicating with a specific remote CFDP entity.
    pub fn connect(
        &self,
        dest_entity_id: u32,
        dest_endpoint: std::net::SocketAddr,
    ) -> CfdpConnection {
        CfdpConnection {
            command_sender: self.command_sender.clone(),
            dest_entity_id: dest_entity_id.into(),
            dest_endpoint,
        }
    }
}
