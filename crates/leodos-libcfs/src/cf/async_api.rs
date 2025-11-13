//! Async API for CFDP file transfers.

use crate::cf::types::{CfdpClass, TxnState, TxnStatus};
use crate::cf::{engine, tx_file};
use crate::error::Error;
use core::ffi::CStr;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferError {
    InitFailed(Error),
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

pub struct CfEngine {
    local_entity_id: u32,
}

impl CfEngine {
    pub fn new(local_entity_id: u32) -> Self {
        Self { local_entity_id }
    }

    pub fn local_entity_id(&self) -> u32 {
        self.local_entity_id
    }

    pub fn cycle(&self) {
        engine::cycle();
    }

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

pub struct TransferFuture {
    seq_num: u32,
    src_eid: u32,
}

impl TransferFuture {
    pub fn seq_num(&self) -> u32 {
        self.seq_num
    }

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
