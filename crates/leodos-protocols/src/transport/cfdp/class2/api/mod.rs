use heapless::Vec;
use crate::cfdp::machine::receiver::Action as ReceiverAction;
use crate::cfdp::machine::sender::Action as SenderAction;
use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::machine::TransactionId;
use crate::cfdp::machine::FILE_DATA_CHUNK_SIZE;
use crate::cfdp::pdu::EntityId;

#[cfg(feature = "tokio")]
use tokio::sync::oneshot;

#[cfg(feature = "cfs")]
use leodos_libcfs::runtime::sync::oneshot::Sender as OneshotSender;

pub mod net;
pub mod runner;
pub mod user;

/// Errors that can occur in the high-level CFDP API.
#[derive(Debug)]
pub enum CfdpError {
    /// Failed to send a PDU or internal message.
    SendError,
    /// An internal communication channel was closed unexpectedly.
    ChannelClosed,
    /// A filestore operation failed.
    FileStoreError,
}

/// Maximum number of concurrent I/O operations.
pub(crate) const MAX_CONCURRENT_IO: usize = 8;
/// Capacity of the command channel between user API and runner.
pub(crate) const COMMAND_QUEUE_SIZE: usize = 4;
/// Capacity of the internal event channel within the runner.
pub(crate) const INTERNAL_EVENT_QUEUE_SIZE: usize = 16;
/// Capacity of the received file notification channel.
pub(crate) const RECEIVED_FILE_QUEUE_SIZE: usize = 8;

/// Commands sent from the user API to the runner task.
#[cfg(feature = "tokio")]
pub(crate) enum Command {
    /// Request to initiate a file transfer to a remote entity.
    Put {
        /// Source file path in the local filestore.
        sender_file_name: heapless::String<256>,
        /// Destination file path in the remote filestore.
        receiver_file_name: heapless::String<256>,
        /// The CFDP entity ID of the destination.
        dest_entity_id: EntityId,
        /// Network address of the destination entity.
        dest_endpoint: std::net::SocketAddr,
        /// Channel to report the transaction result back to the caller.
        result_sender: oneshot::Sender<Result<TransactionFinishedParams, CfdpError>>,
    },
}

/// Commands sent from the user API to the runner task.
#[cfg(feature = "cfs")]
pub(crate) enum Command<'a> {
    /// Request to initiate a file transfer to a remote entity.
    Put {
        /// Source file path in the local filestore.
        sender_file_name: heapless::String<256>,
        /// Destination file path in the remote filestore.
        receiver_file_name: heapless::String<256>,
        /// The CFDP entity ID of the destination.
        dest_entity_id: EntityId,
        /// Network address of the destination entity.
        dest_endpoint: core::net::SocketAddr,
        /// Channel to report the transaction result back to the caller.
        result_sender: OneshotSender<'a, TransactionFinishedParams>,
    },
}

/// Internal events passed between spawned I/O tasks and the runner loop.
pub(crate) enum InternalEvent {
    /// Sender state machine produced actions to execute.
    SourceActions(Vec<SenderAction, 8>),
    /// Receiver state machine produced actions to execute.
    DestActions(Vec<ReceiverAction, 8>),
    /// The file size for a pending put request has been resolved.
    FileSizeReady {
        /// Temporary ID correlating this result to a pending put.
        temp_id: u32,
        /// The resolved file size in bytes.
        size: u64,
    },
    /// Failed to retrieve the file size for a pending put request.
    FileStoreGetSizeError {
        /// Temporary ID correlating this error to a pending put.
        temp_id: u32,
    },
    /// A chunk of file data has been read from the filestore.
    FileDataReady {
        /// The transaction this data belongs to.
        id: TransactionId,
        /// The read file data.
        data: Vec<u8, FILE_DATA_CHUNK_SIZE>,
        /// Byte offset within the file where this data begins.
        offset: u64,
    },
    /// A filestore read operation failed.
    FileStoreReadError(TransactionId),
    /// A chunk of file data has been written to the filestore.
    FileDataWritten {
        /// The transaction this write belongs to.
        id: TransactionId,
        /// Byte offset within the file where data was written.
        offset: u64,
        /// Number of bytes written.
        len: usize,
    },
    /// A filestore write operation failed.
    FileStoreWriteError(TransactionId),
    /// A transaction has completed.
    TransactionFinished(TransactionFinishedParams),
}
