use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::api::CfdpError;

#[cfg(feature = "tokio")]
use tokio::sync::oneshot;

#[cfg(feature = "cfs")]
use leodos_libcfs::runtime::sync::oneshot::{Receiver, RecvError};

/// Errors that can occur when waiting for a CFDP stream to complete.
#[derive(Debug)]
pub enum StreamError {
    /// The result channel was closed before a result was received.
    ChannelClosed,
    /// The underlying CFDP transaction failed.
    TransactionFailed(CfdpError),
}

/// A handle to an in-progress CFDP file transfer that can be awaited.
#[cfg(feature = "tokio")]
pub struct CfdpStream {
    /// Channel receiving the final transaction result.
    result: oneshot::Receiver<Result<TransactionFinishedParams, CfdpError>>,
}

/// A handle to an in-progress CFDP file transfer that can be awaited.
#[cfg(feature = "cfs")]
pub struct CfdpStream<'a> {
    /// Channel receiving the final transaction result.
    result: Receiver<'a, TransactionFinishedParams>,
}

#[cfg(feature = "tokio")]
impl CfdpStream {
    /// Creates a new stream from a result receiver.
    pub(crate) fn new(result: oneshot::Receiver<Result<TransactionFinishedParams, CfdpError>>) -> Self {
        Self { result }
    }

    /// Waits for the file transfer to complete and returns the result.
    pub async fn wait_for_completion(self) -> Result<TransactionFinishedParams, StreamError> {
        match self.result.await {
            Ok(Ok(params)) => Ok(params),
            Ok(Err(e)) => Err(StreamError::TransactionFailed(e)),
            Err(_) => Err(StreamError::ChannelClosed),
        }
    }
}

#[cfg(feature = "cfs")]
impl<'a> CfdpStream<'a> {
    /// Creates a new stream from a result receiver.
    pub(crate) fn new(result: Receiver<'a, TransactionFinishedParams>) -> Self {
        Self { result }
    }

    /// Waits for the file transfer to complete and returns the result.
    pub async fn wait_for_completion(self) -> Result<TransactionFinishedParams, RecvError> {
        self.result.recv().await
    }
}
