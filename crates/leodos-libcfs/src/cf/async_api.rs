//! Async API for CFDP file transfers.

use crate::cf::types::{CfdpClass, TxnState, TxnStatus};
use crate::cf::{engine, tx_file};
use crate::error::Error;
use core::ffi::CStr;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

/// Errors from a CFDP file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
    /// The transfer could not be initiated.
    InitFailed(Error),
    /// The transfer failed after starting.
    TransferFailed(TxnStatus),
}

impl From<Error> for TransferError {
    fn from(e: Error) -> Self {
        TransferError::InitFailed(e)
    }
}

impl From<TxnStatus> for TransferError {
    fn from(s: TxnStatus) -> Self {
        TransferError::TransferFailed(s)
    }
}

/// High-level CFDP engine handle.
pub struct CfEngine {
    local_entity_id: u32,
}

impl CfEngine {
    /// Creates a new engine handle for the given entity ID.
    pub fn new(local_entity_id: u32) -> Self {
        Self { local_entity_id }
    }

    /// Returns the local entity ID.
    pub fn local_entity_id(&self) -> u32 {
        self.local_entity_id
    }

    /// Runs one engine cycle.
    pub fn cycle(&self) {
        engine::cycle();
    }

    /// Initiates a file transfer, returning a pollable future.
    pub fn send_file(
        &self,
        src: &CStr,
        dst: &CStr,
        cfdp_class: CfdpClass,
        keep: bool,
        chan: u8,
        priority: u8,
        dest_id: u32,
    ) -> Result<TransferFuture, TransferError> {
        let seq_before = engine::engine_seq_num();

        tx_file(src, dst, cfdp_class, keep, chan, priority, dest_id)?;

        Ok(TransferFuture {
            seq_num: seq_before.wrapping_add(1),
            src_eid: self.local_entity_id,
        })
    }
}

/// Future that resolves when a CFDP transfer completes.
pub struct TransferFuture {
    seq_num: u32,
    src_eid: u32,
}

impl TransferFuture {
    /// Returns the transaction sequence number.
    pub fn seq_num(&self) -> u32 {
        self.seq_num
    }

    /// Returns the source entity ID.
    pub fn src_eid(&self) -> u32 {
        self.src_eid
    }
}

impl Future for TransferFuture {
    type Output = Result<TxnStatus, TransferError>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        match engine::find_transaction_by_seq(self.seq_num, self.src_eid) {
            Some(txn) => {
                let state = txn.state();
                match state {
                    TxnState::Undef | TxnState::Hold => {
                        let status = txn.get_status();
                        if status == TxnStatus::NoError {
                            Poll::Ready(Ok(status))
                        } else {
                            Poll::Ready(Err(status.into()))
                        }
                    }
                    TxnState::Drop => {
                        let status = txn.get_status();
                        Poll::Ready(Err(status.into()))
                    }
                    _ => Poll::Pending,
                }
            }
            None => Poll::Ready(Ok(TxnStatus::NoError)),
        }
    }
}
