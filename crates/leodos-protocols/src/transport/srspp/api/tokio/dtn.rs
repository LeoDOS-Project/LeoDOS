//! Delay-tolerant send wrapper around [`SrsppSender`].
//!
//! Wraps a [`SrsppSender`] with reachability gating: when the target
//! is not reachable per the supplied [`Reachable`] oracle, the message
//! is queued in an in-memory store instead of being transmitted.
//! Stored messages are drained the next time the target becomes
//! reachable.
//!
//! Designed for the ground-station tool, where targets are
//! satellites that go in and out of line-of-sight. Reuses the
//! protocol-level [`Reachable`] trait from
//! [`crate::transport::srspp::dtn`]; the cFS-side `MessageStore`
//! trait is ground-station-targeted via its u16 bitmap, so we keep a
//! plain `HashMap`-keyed-by-`Address` store here instead.

use std::collections::HashMap;
use std::collections::VecDeque;

use zerocopy::IntoBytes;
use zerocopy::Immutable;

use crate::buffer_pool::BufferPool;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::transport::srspp::dtn::Reachable;
use crate::transport::srspp::rto::RtoPolicy;

use super::SrsppError;
use super::SrsppSender;

/// Wraps a [`SrsppSender`] with reachability-gated send + in-memory
/// store-and-forward.
pub struct SrsppDtnSender<
    'pool,
    L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>,
    Rto: RtoPolicy,
    P: BufferPool + 'pool,
    R: Reachable,
    const WIN: usize,
    const MTU: usize,
> {
    inner: SrsppSender<'pool, L, Rto, P, WIN, MTU>,
    reachable: R,
    origin: Address,
    store: HashMap<Address, VecDeque<Vec<u8>>>,
    capacity_per_target: usize,
}

impl<
    'pool,
    L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>,
    Rto: RtoPolicy,
    P: BufferPool + 'pool,
    R: Reachable,
    const WIN: usize,
    const MTU: usize,
> SrsppDtnSender<'pool, L, Rto, P, R, WIN, MTU>
{
    /// Build a DTN-capable sender. `origin` is the local address used
    /// in `Reachable::is_reachable(origin, target)` calls.
    /// `capacity_per_target` caps the per-target queue length; older
    /// messages are dropped when the cap is exceeded.
    pub fn new(
        inner: SrsppSender<'pool, L, Rto, P, WIN, MTU>,
        reachable: R,
        origin: Address,
        capacity_per_target: usize,
    ) -> Self {
        Self {
            inner,
            reachable,
            origin,
            store: HashMap::new(),
            capacity_per_target,
        }
    }

    /// Send a message. If the target is reachable now, attempt to
    /// drain any previously-stored messages for reachable targets
    /// first, then send. Otherwise, queue the message.
    pub async fn send(
        &mut self,
        target: Address,
        data: &(impl IntoBytes + Immutable + ?Sized),
    ) -> Result<(), SrsppError> {
        let bytes = data.as_bytes();
        if self.reachable.is_reachable(self.origin, target) {
            self.drain_now().await?;
            self.inner.send(target, bytes).await
        } else {
            self.enqueue(target, bytes.to_vec());
            Ok(())
        }
    }

    /// End-of-stream marker for `target`. Only delivered if the target
    /// is currently reachable; otherwise dropped (EOS does not buffer
    /// since it only makes sense within an active connection).
    pub async fn send_eos(&mut self, target: Address) -> Result<(), SrsppError> {
        if self.reachable.is_reachable(self.origin, target) {
            self.inner.send_eos(target).await
        } else {
            Ok(())
        }
    }

    /// Wait for all queued (non-DTN) data to be acknowledged.
    pub async fn flush(&mut self) -> Result<(), SrsppError> {
        self.inner.flush().await
    }

    /// Run the sender's poll cycle and opportunistically drain any
    /// stored messages whose targets are now reachable.
    pub async fn poll(&mut self) -> Result<(), SrsppError> {
        self.drain_now().await?;
        self.inner.poll().await
    }

    /// Number of messages currently held for `target`.
    pub fn pending(&self, target: Address) -> usize {
        self.store.get(&target).map(|q| q.len()).unwrap_or(0)
    }

    /// Total messages buffered across all targets.
    pub fn pending_total(&self) -> usize {
        self.store.values().map(|q| q.len()).sum()
    }

    fn enqueue(&mut self, target: Address, data: Vec<u8>) {
        let queue = self.store.entry(target).or_default();
        if queue.len() >= self.capacity_per_target {
            queue.pop_front();
        }
        queue.push_back(data);
    }

    async fn drain_now(&mut self) -> Result<(), SrsppError> {
        let targets: Vec<Address> = self
            .store
            .iter()
            .filter(|(_, q)| !q.is_empty())
            .map(|(addr, _)| *addr)
            .filter(|addr| self.reachable.is_reachable(self.origin, *addr))
            .collect();
        for target in targets {
            while let Some(data) = self
                .store
                .get_mut(&target)
                .and_then(|q| q.pop_front())
            {
                self.inner.send(target, data.as_slice()).await?;
            }
        }
        Ok(())
    }
}
