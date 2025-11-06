//! The high-level, portable async API for CFDP.

use heapless::Vec;
use tokio::sync::oneshot;

use crate::cfdp::machine::receiver::Action as ReceiverAction;
use crate::cfdp::machine::sender::Action as SenderAction;
use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::pdu::EntityId;

pub mod connection;
pub mod entity;
pub mod runner;
pub mod user;

#[derive(Debug)]
pub enum CfdpError {
    SendError,
    ChannelClosed,
    FileStoreError,
}

type CommandResult = Result<TransactionFinishedParams, CfdpError>;

/// A command sent from a user-facing handle to the background `Runner` task.
pub(crate) enum Command {
    Put {
        sender_file_name: Vec<u8, 256>,
        receiver_file_name: Vec<u8, 256>,
        dest_entity_id: EntityId,
        dest_endpoint: std::net::SocketAddr,
        result_sender: oneshot::Sender<CommandResult>,
    },
}

pub(crate) enum InternalEvent {
    // Actions produced by the sender state machine that need to be run
    SourceActions(Vec<SenderAction, 8>),
    // Actions produced by the destination state machine that need to be run
    DestActions(Vec<ReceiverAction, 8>),
}
