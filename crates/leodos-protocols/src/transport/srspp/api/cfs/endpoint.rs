//! Shared SRSPP link with composable per-peer views.
//!
//! [`SrsppEndpoint`] owns one link plus a pool of pre-allocated sender
//! and receiver state, and exposes three orthogonal views:
//!
//! - [`SrsppEndpoint::sender`] — outbound bound to one target.
//! - [`SrsppEndpoint::receiver`] — inbound bound to one source.
//! - [`SrsppEndpoint::listener`] — inbound from any source.
//!
//! The endpoint demuxes incoming packets to the right view and routes
//! outgoing packets through the shared link. Conflict rules:
//!
//! 1. Two listeners are not allowed simultaneously.
//! 2. Two receivers bound to the same source are not allowed.
//! 3. Two senders bound to the same target are not allowed.
//! 4. A listener and any explicit receiver are mutually exclusive.
//! 5. A listener coexists with explicit senders; the listener handler
//!    obtains its reply path through the same sender pool, so a
//!    duplicate-target conflict surfaces as case 3.
//!
//! View handles release their slot on drop, so the slots are reusable
//! across the endpoint's lifetime.

use core::cell::Cell;
use core::future::poll_fn;
use core::task::Poll;

use futures::FutureExt;
use heapless::Vec;
use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::time::sleep;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

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
use crate::transport::srspp::machine::sender::SenderEvent;
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
use super::sender::drain_stored;
use super::sender::duration_until;
use super::sender::handle_timeouts as sender_handle_timeouts;
use super::sender::next_deadline as sender_next_deadline;
use super::sender::process_ack;
use super::sender::transmit;

/// Operating mode for the receive side of the endpoint.
///
/// Listener and Receivers are mutually exclusive (rule 4); a single
/// flag tracks which is active so view constructors can fail early.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Mode {
    None,
    Listener,
    Receivers,
}

/// Pre-allocated sender slot. Bound to a target on `sender()`, reset
/// and unbound on view drop.
struct TxSlot<'pool, E, P: BufferPool + 'pool, const WIN: usize, const MTU: usize> {
    target: Cell<Option<Address>>,
    state: SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
}

/// Errors produced by [`SrsppEndpoint`] view constructors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum EndpointError<E> {
    /// Underlying transport error.
    #[error(transparent)]
    Transport(#[from] TransportError<E>),
    /// A listener is already alive (rule 1).
    #[error("listener already exists")]
    ListenerExists,
    /// Cannot allocate explicit receiver while a listener is alive (rule 4).
    #[error("listener active")]
    ListenerActive,
    /// Cannot create listener while explicit receivers exist (rule 4).
    #[error("explicit receivers active")]
    ReceiversActive,
    /// A receiver is already bound to this source (rule 2).
    #[error("source already bound")]
    SourceBound(Address),
    /// A sender is already bound to this target (rule 3).
    #[error("target already bound")]
    TargetBound(Address),
    /// All sender slots are in use.
    #[error("no free tx slots")]
    NoTxSlots,
    /// All receiver slots are in use.
    #[error("no free rx slots")]
    NoRxSlots,
}

/// SRSPP link shared by composable per-peer views.
///
/// See the [module docs](self) for the conflict rules. `MAX_TX`
/// bounds concurrent senders, `MAX_STREAMS` bounds concurrent inbound
/// sources (used by both listener and explicit receivers).
pub struct SrsppEndpoint<
    'pool,
    E,
    P: BufferPool + 'pool,
    S: MessageStore = NoStore,
    Re: Reachable = AlwaysReachable,
    Rb: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const WIN: usize = 8,
    const MTU: usize = 512,
    const MAX_TX: usize = 4,
    const MAX_STREAMS: usize = 4,
> {
    tx_slots: Vec<TxSlot<'pool, E, P, WIN, MTU>, MAX_TX>,
    rx_state: SyncRefCell<MultiReceiverState<E, Rb, MAX_STREAMS>>,
    bound_sources: SyncRefCell<Vec<Address, MAX_STREAMS>>,
    mode: Cell<Mode>,
    dtn: SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
}

impl<
    'pool,
    E: Clone,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    Rb: ReceiverBackend,
    const WIN: usize,
    const MTU: usize,
    const MAX_TX: usize,
    const MAX_STREAMS: usize,
> SrsppEndpoint<'pool, E, P, S, Re, Rb, WIN, MTU, MAX_TX, MAX_STREAMS>
{
    /// Constructs a new endpoint with `MAX_TX` pre-allocated sender slots.
    ///
    /// Each sender slot reserves `buf_size` bytes from `pool` for its
    /// send buffer. The receive side allocates per-source state lazily
    /// as packets arrive.
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
        for _ in 0..MAX_TX {
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
            tx_slots,
            rx_state: SyncRefCell::new(MultiReceiverState {
                config: receiver_config,
                streams: heapless::LinearMap::new(),
                ack_delay,
                error: None,
            }),
            bound_sources: SyncRefCell::new(Vec::new()),
            mode: Cell::new(Mode::None),
            dtn: SyncRefCell::new(DtnContext { store, reachable }),
            origin,
        })
    }

    /// Allocates a sender bound to `target`.
    ///
    /// Returns [`EndpointError::TargetBound`] if a sender is already alive
    /// for `target`, or [`EndpointError::NoTxSlots`] if all slots are in use.
    pub fn sender(
        &self,
        target: Address,
    ) -> Result<EndpointSender<'_, 'pool, E, P, S, Re, WIN, MTU>, EndpointError<E>> {
        if self
            .tx_slots
            .iter()
            .any(|slot| slot.target.get() == Some(target))
        {
            return Err(EndpointError::TargetBound(target));
        }
        let Some(slot) = self
            .tx_slots
            .iter()
            .find(|slot| slot.target.get().is_none())
        else {
            return Err(EndpointError::NoTxSlots);
        };
        reset_slot(slot);
        slot.target.set(Some(target));
        Ok(EndpointSender {
            state: &slot.state,
            slot_target: &slot.target,
            dtn: &self.dtn,
            origin: self.origin,
            target,
        })
    }

    /// Allocates a receiver bound to `source`.
    ///
    /// Errors if a listener is alive (rule 4), the source is already
    /// bound (rule 2), or capacity is exhausted.
    pub fn receiver(
        &self,
        source: Address,
    ) -> Result<EndpointReceiver<'_, E, Rb, MAX_STREAMS>, EndpointError<E>> {
        if self.mode.get() == Mode::Listener {
            return Err(EndpointError::ListenerActive);
        }
        let already = self
            .bound_sources
            .with(|bs| bs.iter().any(|&a| a == source));
        if already {
            return Err(EndpointError::SourceBound(source));
        }
        let pushed = self
            .bound_sources
            .with_mut(|bs| bs.push(source).is_ok());
        if !pushed {
            return Err(EndpointError::NoRxSlots);
        }
        self.mode.set(Mode::Receivers);
        Ok(EndpointReceiver {
            rx_state: &self.rx_state,
            bound_sources: &self.bound_sources,
            mode: &self.mode,
            source,
        })
    }

    /// Allocates the listener.
    ///
    /// Errors if a listener already exists (rule 1) or any explicit
    /// receiver is active (rule 4).
    pub fn listener(
        &self,
    ) -> Result<EndpointListener<'_, E, Rb, MAX_STREAMS>, EndpointError<E>> {
        match self.mode.get() {
            Mode::Listener => return Err(EndpointError::ListenerExists),
            Mode::Receivers => return Err(EndpointError::ReceiversActive),
            Mode::None => {}
        }
        self.mode.set(Mode::Listener);
        Ok(EndpointListener {
            rx_state: &self.rx_state,
            mode: &self.mode,
        })
    }

    /// Drives the I/O loop. Reads packets from `link`, demuxes to the
    /// matching receiver stream or sender slot, and runs retransmission
    /// and ACK timers. Returns on link error.
    pub async fn run<L, Rto>(
        &self,
        mut link: L,
        rto_policy: Rto,
    ) -> Result<(), TransportError<E>>
    where
        L: NetworkRead<Error = E> + NetworkWrite<Error = E>,
        Rto: RtoPolicy,
    {
        let mut tx_buffer = [0u8; MTU];
        let mut recv_buffer = [0u8; MTU];
        let mut ack_buffer = [0u8; ACK_BUFFER_SIZE];

        loop {
            if let Some(e) = self.global_error() {
                return Err(e);
            }

            for slot in self.tx_slots.iter() {
                if slot.target.get().is_none() {
                    continue;
                }
                if let Err(e) = drain_stored(
                    &slot.state,
                    &self.dtn,
                    &mut tx_buffer,
                    self.origin,
                    &rto_policy,
                    &mut link,
                )
                .await
                {
                    self.set_errors(e.clone());
                    return Err(e);
                }
                if let Err(e) = transmit(
                    &slot.state,
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
                        receiver_handle_timeouts(&self.rx_state, &mut ack_buffer, &mut link)
                            .await
                    {
                        self.set_errors(e.clone());
                        return Err(e);
                    }
                    for slot in self.tx_slots.iter() {
                        if slot.target.get().is_none() {
                            continue;
                        }
                        if let Err(e) = sender_handle_timeouts(
                            &slot.state,
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
                let source = parsed.source_address();
                if self.mode.get() == Mode::Receivers {
                    let bound = self
                        .bound_sources
                        .with(|bs| bs.iter().any(|&a| a == source));
                    if !bound {
                        return Ok(());
                    }
                }
                process_data(&self.rx_state, ack_buffer, packet, link).await
            }
            Ok(SrsppType::Ack) => {
                let Ok(ack) = SrsppAckPacket::parse(packet) else {
                    return Ok(());
                };
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
            Err(_) => Ok(()),
        }
    }

    fn next_timeout(&self) -> Duration {
        let r = receiver_next_deadline(&self.rx_state);
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
        self.rx_state.with_mut(|s| s.error = Some(err.clone()));
        for slot in self.tx_slots.iter() {
            slot.state.with_mut(|s| s.error = Some(err.clone()));
        }
    }

    fn global_error(&self) -> Option<TransportError<E>> {
        self.rx_state.with(|s| s.error.clone())
    }
}

fn reset_slot<'pool, E, P, const WIN: usize, const MTU: usize>(
    slot: &TxSlot<'pool, E, P, WIN, MTU>,
) where
    P: BufferPool + 'pool,
{
    slot.state.with_mut(|s| {
        s.machine.reset();
        s.actions.clear();
        s.timers = TimerSet::new();
        s.closed = false;
        s.error = None;
    });
}

/// Outbound view bound to a single target.
///
/// Drop frees the slot so the endpoint can rebind it to another target.
pub struct EndpointSender<
    'a,
    'pool,
    E,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    const WIN: usize,
    const MTU: usize,
> {
    state: &'a SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    slot_target: &'a Cell<Option<Address>>,
    dtn: &'a SyncRefCell<DtnContext<S, Re>>,
    origin: Address,
    target: Address,
}

impl<
    'a,
    'pool,
    E,
    P: BufferPool + 'pool,
    S: MessageStore,
    Re: Reachable,
    const WIN: usize,
    const MTU: usize,
> Drop for EndpointSender<'a, 'pool, E, P, S, Re, WIN, MTU>
{
    fn drop(&mut self) {
        self.state.with_mut(|s| {
            s.machine.reset();
            s.actions.clear();
            s.timers = TimerSet::new();
            s.closed = false;
            s.error = None;
        });
        self.slot_target.set(None);
    }
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
> EndpointSender<'a, 'pool, E, P, S, Re, WIN, MTU>
{
    /// Returns the bound target.
    pub fn target(&self) -> Address {
        self.target
    }

    /// Sends data to the bound target.
    ///
    /// Stores the message via the DTN store if the target is currently
    /// unreachable; otherwise enters SRSPP for reliable delivery.
    pub async fn send(
        &mut self,
        data: &(impl IntoBytes + Immutable + ?Sized),
    ) -> Result<(), TransportError<E>> {
        let target = self.target;
        let data = data.as_bytes();

        if !self
            .dtn
            .with(|d| d.reachable.is_reachable(self.origin, target))
        {
            self.dtn
                .with_mut(|d| d.store.write(target, data, 0, SysTime::now().seconds()));
            return Ok(());
        }

        poll_fn(|_cx| {
            self.state.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                if s.machine.available_bytes() >= data.len() && s.machine.available_window() > 0 {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        })
        .await?;

        self.state.with_mut(|s| {
            s.machine
                .handle(SenderEvent::SendRequest { target, data }, &mut s.actions)
        })?;
        Ok(())
    }

    /// Sends an end-of-stream packet to the bound target.
    pub async fn send_eos(&mut self) -> Result<(), TransportError<E>> {
        let target = self.target;
        poll_fn(|_cx| {
            self.state.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                if s.machine.available_window() > 0 {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        })
        .await?;

        self.state.with_mut(|s| {
            s.machine
                .handle(SenderEvent::SendEos { target }, &mut s.actions)
        })?;
        Ok(())
    }

    /// Returns true once all queued data has been acknowledged.
    pub fn is_idle(&self) -> bool {
        self.state.with(|s| s.machine.is_idle())
    }

    /// Waits until all queued data has been acknowledged.
    ///
    /// Drop alone abandons in-flight data; call `flush` first if the
    /// caller needs delivery confirmation before releasing the slot.
    pub async fn flush(&mut self) -> Result<(), TransportError<E>> {
        poll_fn(|_cx| {
            self.state.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                if s.machine.is_idle() {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        })
        .await
    }

    /// Returns the buffer space available for new send payloads.
    pub fn available_bytes(&self) -> usize {
        self.state.with(|s| s.machine.available_bytes())
    }

    /// Returns the number of free window slots.
    pub fn available_window(&self) -> usize {
        self.state.with(|s| s.machine.available_window())
    }
}

/// Inbound view bound to a single source.
///
/// Drop releases the source so the endpoint may switch back to listener
/// mode (or accept a new explicit receiver for a different source).
pub struct EndpointReceiver<'a, E, Rb: ReceiverBackend, const MAX_STREAMS: usize> {
    rx_state: &'a SyncRefCell<MultiReceiverState<E, Rb, MAX_STREAMS>>,
    bound_sources: &'a SyncRefCell<Vec<Address, MAX_STREAMS>>,
    mode: &'a Cell<Mode>,
    source: Address,
}

impl<'a, E, Rb: ReceiverBackend, const MAX_STREAMS: usize> Drop
    for EndpointReceiver<'a, E, Rb, MAX_STREAMS>
{
    fn drop(&mut self) {
        let now_empty = self.bound_sources.with_mut(|bs| {
            if let Some(idx) = bs.iter().position(|&a| a == self.source) {
                let _ = bs.swap_remove(idx);
            }
            bs.is_empty()
        });
        if now_empty {
            self.mode.set(Mode::None);
        }
    }
}

impl<'a, E: Clone, Rb: ReceiverBackend, const MAX_STREAMS: usize>
    EndpointReceiver<'a, E, Rb, MAX_STREAMS>
{
    /// Returns the bound source.
    pub fn source(&self) -> Address {
        self.source
    }

    /// Receives the next message from the bound source.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, TransportError<E>> {
        poll_fn(|_cx| {
            self.rx_state.with_mut(|s| {
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

    /// Receives the next message and processes it in-place with `f`.
    pub async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, TransportError<E>>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        poll_fn(|_cx| {
            self.rx_state.with(|s| {
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

        let ret = self.rx_state.with_mut(|s| {
            let stream = s.streams.get_mut(&self.source).unwrap();
            stream.machine.consume_message(f).unwrap()
        });
        Ok(ret)
    }
}

/// Multi-source inbound view.
///
/// Receives messages from any source. Drop releases listener mode.
pub struct EndpointListener<'a, E, Rb: ReceiverBackend, const MAX_STREAMS: usize> {
    rx_state: &'a SyncRefCell<MultiReceiverState<E, Rb, MAX_STREAMS>>,
    mode: &'a Cell<Mode>,
}

impl<'a, E, Rb: ReceiverBackend, const MAX_STREAMS: usize> Drop
    for EndpointListener<'a, E, Rb, MAX_STREAMS>
{
    fn drop(&mut self) {
        self.mode.set(Mode::None);
    }
}

impl<'a, E: Clone, Rb: ReceiverBackend, const MAX_STREAMS: usize>
    EndpointListener<'a, E, Rb, MAX_STREAMS>
{
    /// Receives the next message from any source.
    ///
    /// Returns the source address along with the message length.
    pub async fn recv(
        &mut self,
        buf: &mut [u8],
    ) -> Result<(Address, usize), TransportError<E>> {
        poll_fn(|_cx| {
            self.rx_state.with_mut(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                for (source, stream) in s.streams.iter_mut() {
                    if let Some(msg) = stream.machine.take_message() {
                        let len = msg.len().min(buf.len());
                        buf[..len].copy_from_slice(&msg[..len]);
                        return Poll::Ready(Ok((*source, len)));
                    }
                }
                Poll::Pending
            })
        })
        .await
    }

    /// Receives the next message and processes it in-place with `f`.
    pub async fn recv_with<F, Ret>(
        &mut self,
        f: F,
    ) -> Result<(Address, Ret), TransportError<E>>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        let source = poll_fn(|_cx| {
            self.rx_state.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                for (source, stream) in s.streams.iter() {
                    if stream.machine.has_message() {
                        return Poll::Ready(Ok(*source));
                    }
                }
                Poll::Pending
            })
        })
        .await?;

        let ret = self.rx_state.with_mut(|s| {
            let stream = s.streams.get_mut(&source).unwrap();
            stream.machine.consume_message(f).unwrap()
        });
        Ok((source, ret))
    }
}
