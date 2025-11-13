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

#[derive(Debug)]
pub enum CfdpError {
    SendError,
    ChannelClosed,
    FileStoreError,
}

pub(crate) const MAX_CONCURRENT_IO: usize = 8;
pub(crate) const COMMAND_QUEUE_SIZE: usize = 4;
pub(crate) const INTERNAL_EVENT_QUEUE_SIZE: usize = 16;
pub(crate) const RECEIVED_FILE_QUEUE_SIZE: usize = 8;

#[cfg(feature = "tokio")]
pub(crate) enum Command {
    Put {
        sender_file_name: heapless::String<256>,
        receiver_file_name: heapless::String<256>,
        dest_entity_id: EntityId,
        dest_endpoint: std::net::SocketAddr,
        result_sender: oneshot::Sender<Result<TransactionFinishedParams, CfdpError>>,
    },
}

#[cfg(feature = "cfs")]
pub(crate) enum Command<'a> {
    Put {
        sender_file_name: heapless::String<256>,
        receiver_file_name: heapless::String<256>,
        dest_entity_id: EntityId,
        dest_endpoint: core::net::SocketAddr,
        result_sender: OneshotSender<'a, TransactionFinishedParams>,
    },
}

pub(crate) enum InternalEvent {
    SourceActions(Vec<SenderAction, 8>),
    DestActions(Vec<ReceiverAction, 8>),
    FileSizeReady {
        temp_id: u32,
        size: u64,
    },
    FileStoreGetSizeError {
        temp_id: u32,
    },
    FileDataReady {
        id: TransactionId,
        data: Vec<u8, FILE_DATA_CHUNK_SIZE>,
        offset: u64,
    },
    FileStoreReadError(TransactionId),
    FileDataWritten {
        id: TransactionId,
        offset: u64,
        len: usize,
    },
    FileStoreWriteError(TransactionId),
    TransactionFinished(TransactionFinishedParams),
}
