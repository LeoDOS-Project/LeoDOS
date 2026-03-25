//! Concurrent per-connection receive via [`serve`](SrsppRxHandle::serve).
//!
//! When `MAX_STREAMS > 1`, multiple sources can send concurrently.
//! `serve()` spawns a handler for each new source address, running
//! up to `MAX_STREAMS` handlers simultaneously.

use core::future::Future;
use core::task::Poll;

use leodos_utils::future_pool::FuturePool;

use crate::network::isl::address::Address;
use crate::transport::srspp::api::cfs::TransportError;
use crate::transport::srspp::dtn::MessageStore;
use crate::transport::srspp::dtn::Reachable;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::utils::cell::SyncRefCell;

use super::receiver::MultiReceiverState;
use super::receiver::SrsppRxHandle;
use super::sender::SrsppTxHandle;

/// A scoped receiver for one source address.
///
/// Created by [`serve`](SrsppRxHandle::serve) for each new
/// connection. `recv()` only returns messages from this source.
pub struct SrsppStream<'a, E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    receiver: &'a SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    source: Address,
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppStream<'a, E, R, MAX_STREAMS>
{
    /// Returns the source address of this connection.
    pub fn source(&self) -> Address {
        self.source
    }

    /// Receives the next message from this source.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, TransportError<E>> {
        core::future::poll_fn(|_cx| {
            self.receiver.with_mut(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                let Some(stream) = s.streams.get_mut(&self.source) else {
                    return Poll::Pending;
                };
                let Some(msg) = stream.machine.take_message() else {
                    return Poll::Pending;
                };
                let len = msg.len().min(buf.len());
                buf[..len].copy_from_slice(&msg[..len]);
                Poll::Ready(Ok(len))
            })
        })
        .await
    }

    /// Receives and processes a message in-place with a closure.
    pub async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, TransportError<E>>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        // Wait for a message to be available
        core::future::poll_fn(|_cx| {
            self.receiver.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                let Some(stream) = s.streams.get(&self.source) else {
                    return Poll::Pending;
                };
                stream
                    .machine
                    .has_message()
                    .then_some(Poll::Ready(Ok(())))
                    .unwrap_or(Poll::Pending)
            })
        })
        .await?;

        // Consume the message
        let ret = self.receiver.with_mut(|s| {
            let stream = s.streams.get_mut(&self.source).unwrap();
            stream.machine.consume_message(f).unwrap()
        });
        Ok(ret)
    }
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppRxHandle<'a, E, R, MAX_STREAMS>
{
    /// Runs concurrent per-connection handlers.
    ///
    /// For each new source address that sends data, calls
    /// `handler` with a [`SrsppStream`] scoped to that source
    /// and a copy of `tx`. Up to `MAX_STREAMS` handlers run
    /// concurrently.
    ///
    /// Returns only on a global error (link failure).
    /// Individual handler errors free the slot silently.
    pub async fn serve<S, Re, F, Fut, const WIN: usize, const BUF: usize, const MTU: usize>(
        &self,
        tx: SrsppTxHandle<'a, E, S, Re, WIN, BUF, MTU>,
        handler: F,
    ) -> Result<(), TransportError<E>>
    where
        S: MessageStore,
        Re: Reachable,
        F: Fn(SrsppStream<'a, E, R, MAX_STREAMS>, SrsppTxHandle<'a, E, S, Re, WIN, BUF, MTU>) -> Fut,
        Fut: Future<Output = Result<(), TransportError<E>>>,
    {
        #[allow(unused_mut)]
        let mut pool = FuturePool::<Fut, MAX_STREAMS>::new();
        let mut assigned: [Option<Address>; MAX_STREAMS] = [None; MAX_STREAMS];
        pin_utils::pin_mut!(pool);

        core::future::poll_fn(|cx| {
            // Check for global error
            let has_error = self.receiver.with(|s| s.error.clone());
            if let Some(e) = has_error {
                return Poll::Ready(Err(e));
            }

            // Accept: find new unassigned sources
            let mut new_sources: [Option<Address>; MAX_STREAMS] = [None; MAX_STREAMS];
            let mut new_count = 0;
            self.receiver.with(|s| {
                for (source, _) in s.streams.iter() {
                    if assigned.contains(&Some(*source)) {
                        continue;
                    }
                    if !pool.as_ref().has_free_slot() {
                        break;
                    }
                    if new_count < MAX_STREAMS {
                        new_sources[new_count] = Some(*source);
                        new_count += 1;
                    }
                }
            });

            for addr in new_sources.into_iter().flatten() {
                let Some(slot_idx) = assigned.iter().position(|a| a.is_none()) else {
                    break;
                };
                assigned[slot_idx] = Some(addr);
                let stream = SrsppStream {
                    receiver: self.receiver,
                    source: addr,
                };
                pool.as_mut().try_spawn(handler(stream, tx));
            }

            // Poll active handlers
            pool.as_mut().poll_all(cx);

            Poll::Pending
        })
        .await
    }
}
