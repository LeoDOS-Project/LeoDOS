//! Concurrent per-connection receive via [`SrsppListener::listen`].
//!
//! When `MAX_STREAMS > 1`, multiple sources can send concurrently.
//! `listen()` spawns a handler for each new source address, running
//! up to `MAX_STREAMS` handlers simultaneously. Each handler is
//! given a [`SrsppTxHandle`] bound to that source, backed by its
//! own [`SenderMachine`], so per-connection sequence counters are
//! independent.

use core::cell::Cell;
use core::future::Future;
use core::task::Poll;

use futures::FutureExt;
use heapless::Vec;
use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::runtime::time::sleep;
use leodos_utils::future_pool::FuturePool;

use crate::buffer_pool::BufferPool;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::transport::srspp::api::cfs::TimerSet;
use crate::transport::srspp::api::cfs::TransportError;
use crate::transport::srspp::dtn::AlwaysReachable;
use crate::transport::srspp::dtn::MessageStore;
use crate::transport::srspp::dtn::NoStore;
use crate::transport::srspp::dtn::Reachable;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::rto::RtoPolicy;
use crate::utils::cell::SyncRefCell;

use super::receiver::ACK_BUFFER_SIZE;
use super::receiver::MultiReceiverState;
use super::receiver::handle_timeouts as receiver_handle_timeouts;
use super::receiver::next_deadline as receiver_next_deadline;
use super::receiver::process_data;
use super::sender::DtnContext;
use super::sender::SenderState;
use super::sender::SrsppTxHandle;
use super::sender::duration_until;
use super::sender::handle_timeouts as sender_handle_timeouts;
use super::sender::next_deadline as sender_next_deadline;
use super::sender::process_ack;
use super::sender::transmit;

/// A scoped receiver for one source address.
///
/// Created by [`SrsppListener::listen`] for each new connection.
/// `recv()` only returns messages from this source.
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

        let ret = self.receiver.with_mut(|s| {
            let stream = s.streams.get_mut(&self.source).unwrap();
            stream.machine.consume_message(f).unwrap()
        });
        Ok(ret)
    }
}

/// One pre-allocated tx slot. Bound to a peer on accept and reset on
/// release so the slot's [`SenderMachine`] can be rebound to a new
/// source for the next connection.
struct TxSlot<'pool, E, P: BufferPool + 'pool, const WIN: usize, const MTU: usize> {
    target: Cell<Option<Address>>,
    state: SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
}

/// Per-handler dial capability: allocates a fresh per-target tx
/// from the listener's slot pool.
pub struct SrsppDial<
    'a,
    'pool,
    E,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    const WIN: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> {
    tx_slots: &'a [TxSlot<'pool, E, P, WIN, MTU>],
    dtn: &'a SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
}

impl<
    'a,
    'pool,
    E: Clone,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    const WIN: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppDial<'a, 'pool, E, P, S, Re, WIN, MTU, MAX_STREAMS>
{
    /// Allocates a free slot bound to `target` and returns a tx
    /// handle for it. Returns `None` if all slots are in use.
    pub fn to(&self, target: Address) -> Option<SrsppTxHandle<'a, 'pool, E, S, Re, P, WIN, MTU>> {
        let slot_idx = self
            .tx_slots
            .iter()
            .position(|slot| slot.target.get() == Some(target))
            .or_else(|| {
                self.tx_slots
                    .iter()
                    .position(|slot| slot.target.get().is_none())
            })?;
        let slot = &self.tx_slots[slot_idx];
        if slot.target.get() != Some(target) {
            slot.state.with_mut(|s| {
                s.machine.reset();
                s.actions.clear();
                s.timers = TimerSet::new();
                s.closed = false;
                s.error = None;
            });
            slot.target.set(Some(target));
        }
        Some(SrsppTxHandle {
            sender: &slot.state,
            dtn: self.dtn,
            origin: self.origin,
            target,
        })
    }
}

/// SRSPP server endpoint with per-source handlers.
///
/// Owns a multi-stream receiver and a fixed pool of `MAX_STREAMS`
/// pre-allocated sender slots. [`listen`](Self::listen) drives I/O,
/// accepts new sources, and invokes the handler with a [`SrsppStream`]
/// scoped to that source plus a [`SrsppTxHandle`] bound to a fresh
/// per-source [`SenderMachine`]. Sequence counters are independent
/// per connection.
pub struct SrsppListener<
    'pool,
    E,
    P: BufferPool + 'pool,
    S: MessageStore = NoStore,
    Re: Reachable = AlwaysReachable,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const WIN: usize = 8,
    const MTU: usize = 512,
    const MAX_STREAMS: usize = 1,
> {
    receiver: SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    tx_slots: Vec<TxSlot<'pool, E, P, WIN, MTU>, MAX_STREAMS>,
    dtn: SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
}

impl<
    'pool,
    E: Clone,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    R: ReceiverBackend,
    const WIN: usize,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppListener<'pool, E, P, S, Re, R, WIN, MTU, MAX_STREAMS>
{
    /// Creates a new listener with `MAX_STREAMS` pre-allocated tx
    /// slots. Each slot reserves `buf_size` bytes from `pool` for its
    /// sender's send buffer.
    pub fn new(
        pool: &'pool P,
        buf_size: usize,
        sender_config: SenderConfig,
        receiver_config: ReceiverConfig,
        store: S,
        reachable: Re,
    ) -> Result<Self, P::Error> {
        let origin = sender_config.source_address;
        let ack_delay = Duration::from_millis(receiver_config.ack_delay_ticks);
        let mut tx_slots = Vec::new();
        for _ in 0..MAX_STREAMS {
            let machine = SenderMachine::new(sender_config.clone(), pool, buf_size)?;
            let _ = tx_slots.push(TxSlot {
                target: Cell::new(None),
                state: SyncRefCell::new(SenderState {
                    machine,
                    actions: SenderActions::new(),
                    timers: TimerSet::new(),
                    closed: false,
                    error: None,
                }),
            });
        }
        Ok(Self {
            receiver: SyncRefCell::new(MultiReceiverState {
                config: receiver_config,
                streams: heapless::LinearMap::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
            tx_slots,
            dtn: SyncRefCell::new(DtnContext { store, reachable }),
            origin,
        })
    }

    /// Drives I/O on `link` and dispatches a handler future for every
    /// new source that sends a packet.
    ///
    /// The handler receives a [`SrsppStream`] for receiving from the
    /// originating source and an [`SrsppTxHandle`] bound to that
    /// source for sending replies. Up to `MAX_STREAMS` handlers run
    /// concurrently. Returns only on a global error (link failure).
    pub async fn listen<'a, L, Rto, F, Fut>(
        &'a self,
        mut link: L,
        rto_policy: Rto,
        handler: F,
    ) -> Result<(), TransportError<E>>
    where
        L: NetworkRead<Error = E> + NetworkWrite<Error = E>,
        Rto: RtoPolicy,
        F: Fn(
            SrsppStream<'a, E, R, MAX_STREAMS>,
            SrsppTxHandle<'a, 'pool, E, S, Re, P, WIN, MTU>,
            SrsppDial<'a, 'pool, E, P, S, Re, WIN, MTU, MAX_STREAMS>,
        ) -> Fut,
        Fut: Future<Output = Result<(), TransportError<E>>>,
    {
        let mut tx_buffer = [0u8; MTU];
        let mut recv_buffer = [0u8; MTU];
        let mut ack_buffer = [0u8; ACK_BUFFER_SIZE];

        let pool = FuturePool::<Fut, MAX_STREAMS>::new();
        pin_utils::pin_mut!(pool);

        loop {
            if let Some(e) = self.receiver.with(|s| s.error.clone()) {
                return Err(e);
            }

            self.spawn_handlers(&handler, pool.as_mut());

            for slot_idx in 0..MAX_STREAMS {
                let bound = self.tx_slots[slot_idx].target.get().is_some();
                if !bound {
                    continue;
                }
                if let Err(e) = transmit(
                    &self.tx_slots[slot_idx].state,
                    &mut tx_buffer,
                    &rto_policy,
                    &mut link,
                )
                .await
                {
                    self.set_errors(e.clone());
                    return Err(e);
                }
            }

            let timeout = self.next_timeout();

            core::future::poll_fn(|cx| {
                pool.as_mut().poll_all(cx);
                Poll::Ready(())
            })
            .await;

            self.release_finished_slots();

            let event = {
                let read_fut = link.read(&mut recv_buffer).fuse();
                let sleep_fut = sleep(timeout).fuse();
                pin_utils::pin_mut!(read_fut, sleep_fut);
                futures::select_biased! {
                    r = read_fut => Some(r),
                    _ = sleep_fut => None,
                }
            };

            match event {
                Some(Ok(len)) => {
                    if let Err(e) = self
                        .handle_incoming(&recv_buffer[..len], &mut ack_buffer, &mut link)
                        .await
                    {
                        self.set_errors(e.clone());
                        return Err(e);
                    }
                }
                Some(Err(e)) => {
                    let err = TransportError::Network(e);
                    self.set_errors(err.clone());
                    return Err(err);
                }
                None => {
                    if let Err(e) =
                        receiver_handle_timeouts(&self.receiver, &mut ack_buffer, &mut link).await
                    {
                        self.set_errors(e.clone());
                        return Err(e);
                    }
                    for slot_idx in 0..MAX_STREAMS {
                        if self.tx_slots[slot_idx].target.get().is_none() {
                            continue;
                        }
                        if let Err(e) = sender_handle_timeouts(
                            &self.tx_slots[slot_idx].state,
                            &mut tx_buffer,
                            &rto_policy,
                            &mut link,
                        )
                        .await
                        {
                            self.set_errors(e.clone());
                            return Err(e);
                        }
                    }
                }
            }
        }
    }

    fn spawn_handlers<'a, F, Fut>(
        &'a self,
        handler: &F,
        mut pool: core::pin::Pin<&mut FuturePool<Fut, MAX_STREAMS>>,
    ) where
        F: Fn(
            SrsppStream<'a, E, R, MAX_STREAMS>,
            SrsppTxHandle<'a, 'pool, E, S, Re, P, WIN, MTU>,
            SrsppDial<'a, 'pool, E, P, S, Re, WIN, MTU, MAX_STREAMS>,
        ) -> Fut,
        Fut: Future<Output = Result<(), TransportError<E>>>,
    {
        let mut new_sources: [Option<Address>; MAX_STREAMS] = [None; MAX_STREAMS];
        let mut count = 0usize;
        self.receiver.with(|s| {
            for (source, _) in s.streams.iter() {
                if self
                    .tx_slots
                    .iter()
                    .any(|slot| slot.target.get() == Some(*source))
                {
                    continue;
                }
                if count < MAX_STREAMS {
                    new_sources[count] = Some(*source);
                    count += 1;
                }
            }
        });

        for source in new_sources.into_iter().flatten() {
            if !pool.has_free_slot() {
                break;
            }
            let Some(slot_idx) = self
                .tx_slots
                .iter()
                .position(|slot| slot.target.get().is_none())
            else {
                break;
            };
            self.bind_slot(slot_idx, source);
            let stream = SrsppStream {
                receiver: &self.receiver,
                source,
            };
            let tx = SrsppTxHandle {
                sender: &self.tx_slots[slot_idx].state,
                dtn: &self.dtn,
                origin: self.origin,
                target: source,
            };
            let dial = SrsppDial {
                tx_slots: &self.tx_slots,
                dtn: &self.dtn,
                origin: self.origin,
            };
            pool.as_mut().try_spawn(handler(stream, tx, dial));
        }
    }

    fn bind_slot(&self, slot_idx: usize, target: Address) {
        let slot = &self.tx_slots[slot_idx];
        slot.state.with_mut(|s| {
            s.machine.reset();
            s.actions.clear();
            s.timers = TimerSet::new();
            s.closed = false;
            s.error = None;
        });
        slot.target.set(Some(target));
    }

    fn release_finished_slots(&self) {
        for slot in self.tx_slots.iter() {
            let Some(addr) = slot.target.get() else { continue };
            let still_active = self.receiver.with(|s| s.streams.contains_key(&addr));
            let idle = slot.state.with(|s| s.machine.is_idle());
            if !still_active && idle {
                slot.state.with_mut(|s| {
                    s.machine.reset();
                    s.actions.clear();
                    s.timers = TimerSet::new();
                });
                slot.target.set(None);
            }
        }
    }

    async fn handle_incoming<L>(
        &self,
        packet: &[u8],
        ack_buffer: &mut [u8],
        link: &mut L,
    ) -> Result<(), TransportError<E>>
    where
        L: NetworkWrite<Error = E>,
    {
        let Ok(parsed) = SrsppPacket::parse(packet) else {
            return Ok(());
        };
        match parsed.srspp_type() {
            Ok(SrsppType::Data) | Ok(SrsppType::Eos) => {
                process_data(&self.receiver, ack_buffer, packet, link).await
            }
            Ok(SrsppType::Ack) => self.dispatch_ack(packet),
            Err(_) => Ok(()),
        }
    }

    fn dispatch_ack(&self, packet: &[u8]) -> Result<(), TransportError<E>> {
        let Ok(ack) = SrsppAckPacket::parse(packet) else { return Ok(()) };
        let from = ack.srspp_header.source_address();
        let Some(slot) = self
            .tx_slots
            .iter()
            .find(|slot| slot.target.get() == Some(from))
        else {
            return Ok(());
        };
        process_ack(&slot.state, packet)
    }

    fn next_timeout(&self) -> Duration {
        let r = receiver_next_deadline(&self.receiver);
        let mut earliest = r;
        for slot in self.tx_slots.iter() {
            if slot.target.get().is_none() {
                continue;
            }
            let s = sender_next_deadline(&slot.state);
            earliest = match (earliest, s) {
                (Some(a), Some(b)) => Some(if a < b { a } else { b }),
                (a, b) => a.or(b),
            };
        }
        duration_until(earliest)
    }

    fn set_errors(&self, err: TransportError<E>) {
        self.receiver.with_mut(|s| s.error = Some(err.clone()));
        for slot in self.tx_slots.iter() {
            slot.state.with_mut(|s| s.error = Some(err.clone()));
        }
    }
}

