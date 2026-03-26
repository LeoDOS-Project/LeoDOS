//! Transport traits and contextualized channels for SpaceComp.
#![allow(async_fn_in_trait)]

use leodos_libcfs::error::CfsError;
use leodos_protocols::network::isl::address::Address;
use leodos_protocols::transport::srspp::api::cfs::SrsppRxHandle;
use leodos_protocols::transport::srspp::api::cfs::SrsppTxHandle;
use leodos_protocols::transport::srspp::dtn::MessageStore;
use leodos_protocols::transport::srspp::dtn::Reachable;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverBackend;

use crate::packet::OpCode;
use crate::packet::SpaceCompMessage;
use crate::SpaceCompError;

/// Sends data to the assigned next-stage node.
///
/// Target address, job ID, and message framing are handled
/// internally. The role function just sends payload bytes.
pub trait Tx {
    /// Sends a data chunk to the next stage.
    async fn send(&mut self, data: &[u8]) -> Result<(), SpaceCompError>;
    /// Signals end of data to the next stage.
    async fn done(&mut self) -> Result<(), SpaceCompError>;
    /// Returns the partition/collector ID for this role.
    fn partition_id(&self) -> u8;
}

/// Receives data from upstream nodes.
///
/// Filters by job ID and handles PhaseDone counting
/// internally. Returns `None` when all senders are done.
pub trait Rx {
    async fn recv(&mut self, buf: &mut [u8]) -> Option<Result<usize, SpaceCompError>>;
    async fn recv_with<T>(
        &mut self,
        f: impl FnMut(&[u8]) -> T,
    ) -> Option<Result<T, SpaceCompError>>;
}

// ── Contextualized SRSPP channels ───────────────────────────

/// SRSPP-backed sender with target + job context.
pub struct SpaceCompTx<
    'a,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    tx: SrsppTxHandle<'a, CfsError, S, R, WIN, BUF, MTU>,
    target: Address,
    job_id: u16,
    partition_id: u8,
    buf: [u8; 512],
}

impl<'a, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize>
    SpaceCompTx<'a, S, R, WIN, BUF, MTU>
{
    pub fn new(
        tx: SrsppTxHandle<'a, CfsError, S, R, WIN, BUF, MTU>,
        target: Address,
        job_id: u16,
        partition_id: u8,
    ) -> Self {
        Self {
            tx,
            target,
            job_id,
            partition_id,
            buf: [0u8; 512],
        }
    }
}

impl<S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize> Tx
    for SpaceCompTx<'_, S, R, WIN, BUF, MTU>
{
    async fn send(&mut self, data: &[u8]) -> Result<(), SpaceCompError> {
        let m = SpaceCompMessage::builder()
            .buffer(&mut self.buf)
            .op_code(OpCode::DataChunk)
            .job_id(self.job_id)
            .payload_len(data.len())
            .build()?;
        m.payload_mut().copy_from_slice(data);
        self.tx.send(self.target, m.as_bytes()).await?;
        Ok(())
    }

    async fn done(&mut self) -> Result<(), SpaceCompError> {
        let m = SpaceCompMessage::builder()
            .buffer(&mut self.buf)
            .op_code(OpCode::PhaseDone)
            .job_id(self.job_id)
            .payload_len(0)
            .build()?;
        self.tx.send(self.target, m.as_bytes()).await?;
        Ok(())
    }

    fn partition_id(&self) -> u8 {
        self.partition_id
    }
}

/// SRSPP-backed receiver with job context + PhaseDone tracking.
pub struct SpaceCompRx<'a, 'rx, R: ReceiverBackend, const MAX_STREAMS: usize> {
    rx: &'a mut SrsppRxHandle<'rx, CfsError, R, MAX_STREAMS>,
    job_id: u16,
    expected_done: u8,
    done_count: u8,
}

impl<'a, 'rx, R: ReceiverBackend, const MAX_STREAMS: usize> SpaceCompRx<'a, 'rx, R, MAX_STREAMS> {
    pub fn new(
        rx: &'a mut SrsppRxHandle<'rx, CfsError, R, MAX_STREAMS>,
        job_id: u16,
        expected_done: u8,
    ) -> Self {
        Self {
            rx,
            job_id,
            expected_done,
            done_count: 0,
        }
    }
}

impl<R: ReceiverBackend, const MAX_STREAMS: usize> Rx for SpaceCompRx<'_, '_, R, MAX_STREAMS> {
    async fn recv(&mut self, buf: &mut [u8]) -> Option<Result<usize, SpaceCompError>> {
        loop {
            let result = self.rx.recv_with(|data| {
                let msg = SpaceCompMessage::parse(data).ok()?;
                if msg.job_id() != self.job_id {
                    return None;
                }
                match msg.op_code() {
                    Ok(OpCode::DataChunk) => {
                        let payload = msg.payload();
                        let n = payload.len().min(buf.len());
                        buf[..n].copy_from_slice(&payload[..n]);
                        Some(Some(n))
                    }
                    Ok(OpCode::PhaseDone) => Some(None),
                    _ => None,
                }
            });
            let Ok(maybe) = result.await else {
                return Some(Err(SpaceCompError::Cfs(CfsError::ExternalResourceFail)));
            };
            let Some(inner) = maybe else { continue };
            match inner {
                Some(len) => return Some(Ok(len)),
                None => {
                    self.done_count += 1;
                    if self.done_count >= self.expected_done {
                        return None;
                    }
                }
            }
        }
    }

    async fn recv_with<T>(
        &mut self,
        mut f: impl FnMut(&[u8]) -> T,
    ) -> Option<Result<T, SpaceCompError>> {
        loop {
            // Use recv into a local buffer, then call f
            let mut buf = [0u8; 8192];
            let len = match self.recv(&mut buf).await {
                Some(Ok(len)) => len,
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };
            return Some(Ok(f(&buf[..len])));
        }
    }
}
