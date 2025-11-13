use crate::cfdp::machine::TransactionFinishedParams;
use crate::cfdp::api::CfdpError;

#[cfg(feature = "tokio")]
use tokio::sync::oneshot;

#[cfg(feature = "cfs")]
use leodos_libcfs::runtime::sync::oneshot::{Receiver, RecvError};

#[derive(Debug)]
pub enum StreamError {
    ChannelClosed,
    TransactionFailed(CfdpError),
}

#[cfg(feature = "tokio")]
pub struct CfdpStream {
    result: oneshot::Receiver<Result<TransactionFinishedParams, CfdpError>>,
}

#[cfg(feature = "cfs")]
pub struct CfdpStream<'a> {
    result: Receiver<'a, TransactionFinishedParams>,
}

#[cfg(feature = "tokio")]
impl CfdpStream {
    pub(crate) fn new(result: oneshot::Receiver<Result<TransactionFinishedParams, CfdpError>>) -> Self {
        Self { result }
    }

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
    pub(crate) fn new(result: Receiver<'a, TransactionFinishedParams>) -> Self {
        Self { result }
    }

    pub async fn wait_for_completion(self) -> Result<TransactionFinishedParams, RecvError> {
        self.result.recv().await
    }
}
