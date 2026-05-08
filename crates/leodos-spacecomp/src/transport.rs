//! Transport traits and contextualized channels for SpaceComp.
#![allow(async_fn_in_trait)]

use leodos_libcfs::cfe::es::pool::MemPool;
use leodos_libcfs::error::CfsError;
use leodos_protocols::transport::srspp::api::cfs::EndpointListener;
use leodos_protocols::transport::srspp::api::cfs::EndpointSender;
use leodos_protocols::transport::srspp::dtn::MessageStore;
use leodos_protocols::transport::srspp::dtn::Reachable;
use leodos_protocols::transport::srspp::machine::receiver::ReceiverBackend;

use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

use crate::bufwriter::BufWriter;
use crate::packet::OpCode;
use crate::packet::SpaceCompMessage;
use crate::reader::RecordIter;
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
    /// Returns a batched writer for fixed-size records.
    fn batched<'a, 'b, T: IntoBytes + Immutable>(
        &'a mut self,
        buf: &'b mut [u8],
    ) -> BufWriter<'a, 'b, T, Self>
    where
        Self: Sized,
    {
        BufWriter::new(self, buf)
    }
}

impl<T: Tx> Tx for &mut T {
    async fn send(&mut self, data: &[u8]) -> Result<(), SpaceCompError> {
        T::send(self, data).await
    }
    async fn done(&mut self) -> Result<(), SpaceCompError> {
        T::done(self).await
    }
    fn partition_id(&self) -> u8 {
        T::partition_id(self)
    }
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
    /// Receives a batch and returns an iterator over fixed-size records.
    async fn recv_batch<'b, T: FromBytes + Immutable + KnownLayout + 'b>(
        &mut self,
        buf: &'b mut [u8],
    ) -> Option<Result<RecordIter<'b, T>, SpaceCompError>> {
        match self.recv(buf).await {
            None => None,
            Some(Err(e)) => Some(Err(e)),
            Some(Ok(len)) => Some(Ok(RecordIter::new(&buf[..len]))),
        }
    }
}

// ── Contextualized SRSPP channels ───────────────────────────

/// SRSPP-backed sender with target + job context.
pub struct SpaceCompTx<
    'a,
    'pool,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const MTU: usize,
> {
    tx: EndpointSender<'a, 'pool, CfsError, MemPool, S, R, WIN, MTU>,
    job_id: u16,
    partition_id: u8,
    buf: [u8; 512],
}

impl<'a, 'pool, S: MessageStore, R: Reachable, const WIN: usize, const MTU: usize>
    SpaceCompTx<'a, 'pool, S, R, WIN, MTU>
{
    /// Wraps a per-target endpoint sender. The sender's bound target
    /// is the next-stage destination for this role.
    pub fn new(
        tx: EndpointSender<'a, 'pool, CfsError, MemPool, S, R, WIN, MTU>,
        job_id: u16,
        partition_id: u8,
    ) -> Self {
        Self {
            tx,
            job_id,
            partition_id,
            buf: [0u8; 512],
        }
    }

    /// Waits for all queued data to be acknowledged by the bound target.
    pub async fn flush(&mut self) -> Result<(), SpaceCompError> {
        self.tx.flush().await?;
        Ok(())
    }
}

impl<'a, 'pool, S: MessageStore, R: Reachable, const WIN: usize, const MTU: usize> Tx
    for SpaceCompTx<'a, 'pool, S, R, WIN, MTU>
{
    async fn send(&mut self, data: &[u8]) -> Result<(), SpaceCompError> {
        let m = SpaceCompMessage::builder()
            .buffer(&mut self.buf)
            .op_code(OpCode::DataChunk)
            .job_id(self.job_id)
            .payload_len(data.len())
            .build()?;
        m.payload_mut().copy_from_slice(data);
        self.tx.send(m.as_bytes()).await?;
        Ok(())
    }

    async fn done(&mut self) -> Result<(), SpaceCompError> {
        let m = SpaceCompMessage::builder()
            .buffer(&mut self.buf)
            .op_code(OpCode::PhaseDone)
            .job_id(self.job_id)
            .payload_len(0)
            .build()?;
        self.tx.send(m.as_bytes()).await?;
        self.tx.flush().await?;
        Ok(())
    }

    fn partition_id(&self) -> u8 {
        self.partition_id
    }
}

/// SRSPP-backed receiver with job context + PhaseDone tracking.
pub struct SpaceCompRx<'a, 'l, R: ReceiverBackend, const MAX_STREAMS: usize> {
    rx: &'a mut EndpointListener<'l, CfsError, R, MAX_STREAMS>,
    job_id: u16,
    expected_done: u8,
    done_count: u8,
}

impl<'a, 'l, R: ReceiverBackend, const MAX_STREAMS: usize> SpaceCompRx<'a, 'l, R, MAX_STREAMS> {
    pub fn new(
        rx: &'a mut EndpointListener<'l, CfsError, R, MAX_STREAMS>,
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
        if self.done_count == self.expected_done {
            return None;
        }
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
            let Ok((_source, maybe)) = result.await else {
                return Some(Err(SpaceCompError::Cfs(CfsError::ExternalResourceFail)));
            };
            let Some(inner) = maybe else { continue };
            if let Some(len) = inner {
                return Some(Ok(len));
            } else {
                self.done_count += 1;
                if self.done_count == self.expected_done {
                    return None;
                }
            }
        }
    }

    async fn recv_with<T>(
        &mut self,
        mut f: impl FnMut(&[u8]) -> T,
    ) -> Option<Result<T, SpaceCompError>> {
        loop {
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
