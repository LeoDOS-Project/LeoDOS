use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::cfdp::high_level::CfdpError;
use crate::cfdp::high_level::Command;
use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::pdu::EntityId;

use heapless::Vec;

/// A lightweight, cloneable handle for interacting with a specific remote CFDP peer.
#[derive(Clone)]
pub struct CfdpConnection {
    /// Channel for sending commands to the runner task.
    pub(crate) command_sender: mpsc::Sender<Command>,
    /// The CFDP entity ID of the remote peer.
    pub(crate) dest_entity_id: EntityId,
    /// Network address of the remote peer.
    pub(crate) dest_endpoint: std::net::SocketAddr,
}

impl CfdpConnection {
    /// Initiates a file transfer (`put`) to the remote entity.
    pub async fn put(
        &self,
        source_file_name: &[u8],
        dest_file_name: &[u8],
    ) -> Result<TransactionFinishedParams, CfdpError> {
        let (result_sender, result_receiver) = oneshot::channel();
        let command = Command::Put {
            sender_file_name: Vec::from_slice(source_file_name.as_bytes()).unwrap(),
            receiver_file_name: Vec::from_slice(dest_file_name.as_bytes()).unwrap(),
            dest_entity_id: self.dest_entity_id,
            dest_endpoint: self.dest_endpoint,
            result_sender,
        };
        self.command_sender
            .send(command)
            .await
            .map_err(|_| CfdpError::ChannelClosed)?;

        result_receiver
            .await
            .map_err(|_| CfdpError::ChannelClosed)?
    }
}
