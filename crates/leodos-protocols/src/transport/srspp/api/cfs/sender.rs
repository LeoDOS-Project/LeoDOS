use core::future::poll_fn;
use core::task::Poll;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

use futures::FutureExt;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::time::sleep;

use crate::buffer_pool::BufferPool;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::network::spp::Apid;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::api::cfs::TransportError;
use crate::transport::srspp::dtn::AlwaysReachable;
use crate::transport::srspp::dtn::MessageStore;
use crate::transport::srspp::dtn::NoStore;
use crate::transport::srspp::dtn::Reachable;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppEosPacket;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::rto::RtoPolicy;
use crate::utils::cell::SyncRefCell;

use super::TimerSet;

/// Shared mutable state for the sender channel.
pub(super) struct SenderState<'pool, E, P: BufferPool + 'pool, const WIN: usize, const MTU: usize> {
    /// Sender state machine.
    pub(crate) machine: SenderMachine<'pool, P, WIN, MTU>,
    /// Pending actions produced by the state machine.
    pub(crate) actions: SenderActions,
    /// Retransmission timers for in-flight packets.
    pub(crate) timers: TimerSet<WIN>,
    /// Whether the handle has signaled no more data.
    pub(crate) closed: bool,
    /// First error encountered, propagated to the handle.
    pub(crate) error: Option<TransportError<E>>,
}

pub(super) struct DtnContext<S, R> {
    pub(super) store: S,
    pub(super) reachable: R,
}

// ── Helper functions (used by both standalone driver and node driver) ──

/// Sends all pending transmit actions over the link.
pub(super) async fn transmit<'pool, E, P, Rto, const WIN: usize, const MTU: usize>(
    sender: &SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    tx_buffer: &mut [u8],
    rto_policy: &Rto,
    link: &mut impl NetworkWrite<Error = E>,
) -> Result<(), TransportError<E>>
where
    E: Clone,
    P: BufferPool + 'pool,
    Rto: RtoPolicy,
{
    let now = SysTime::now();

    let (transmits, cfg_clone) = sender.with(|s| {
        let t = s
            .actions
            .iter()
            .filter_map(|a| match a {
                SenderAction::Transmit { seq, .. } => Some(*seq),
                _ => None,
            })
            .collect::<heapless::Vec<_, WIN>>();
        (t, s.machine.config().clone())
    });

    for seq in transmits {
        let packet_len = sender.with(|s| {
            if let Some(info) = s.machine.get_payload(seq) {
                if info.is_eos {
                    SrsppEosPacket::builder()
                        .buffer(tx_buffer)
                        .source_address(cfg_clone.source_address)
                        .target(info.target)
                        .apid(cfg_clone.apid)
                        .function_code(cfg_clone.function_code)
                        .sequence_count(seq)
                        .build()
                        .map_err(TransportError::Packet)?;
                    Ok::<_, TransportError<E>>(Some(
                        core::mem::size_of::<SrsppEosPacket>(),
                    ))
                } else {
                    let pkt = SrsppDataPacket::builder()
                        .buffer(tx_buffer)
                        .source_address(cfg_clone.source_address)
                        .target(info.target)
                        .apid(cfg_clone.apid)
                        .function_code(cfg_clone.function_code)
                        .sequence_count(seq)
                        .sequence_flag(info.flags)
                        .payload_len(info.payload.len())
                        .build()
                        .map_err(TransportError::Packet)?;
                    pkt.payload.copy_from_slice(info.payload);
                    Ok::<_, TransportError<E>>(Some(
                        SrsppDataPacket::HEADER_SIZE + info.payload.len(),
                    ))
                }
            } else {
                Ok::<_, TransportError<E>>(None)
            }
        })?;

        if let Some(packet_len) = packet_len {
            link.write(&tx_buffer[..packet_len])
                .await
                .map_err(TransportError::Network)?;

            let rto_dur = Duration::from_millis(rto_policy.rto_ticks(now.seconds()));

            sender.with_mut(|s| {
                s.machine.mark_transmitted(seq);
                s.timers.start(seq, now + SysTime::from(rto_dur));
            });
        }
    }

    sender.with_mut(|s| {
        for action in s.actions.iter() {
            let &SenderAction::StopTimer { seq } = action else {
                continue;
            };
            s.timers.stop(seq);
        }
    });

    Ok(())
}

/// Processes a received ACK packet and updates sender state.
pub(super) fn process_ack<'pool, E, P, const WIN: usize, const MTU: usize>(
    sender: &SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    packet: &[u8],
) -> Result<(), TransportError<E>>
where
    E: Clone,
    P: BufferPool + 'pool,
{
    if let Ok(SrsppType::Ack) = SrsppPacket::parse(packet).and_then(|p| p.srspp_type()) {
        if let Ok(ack) = SrsppAckPacket::parse(packet) {
            sender.with_mut(|s| {
                s.machine.handle(
                    SenderEvent::AckReceived {
                        cumulative_ack: ack.ack_payload.cumulative_ack(),
                        selective_bitmap: ack.ack_payload.selective_ack_bitmap(),
                    },
                    &mut s.actions,
                )?;

                for action in s.actions.iter() {
                    let &SenderAction::StopTimer { seq } = action else {
                        continue;
                    };
                    s.timers.stop(seq);
                }
                Ok::<(), TransportError<E>>(())
            })?;
        }
    }
    Ok(())
}

/// Processes expired retransmission timers and retransmits.
pub(super) async fn handle_timeouts<'pool, E, P, Rto, const WIN: usize, const MTU: usize>(
    sender: &SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    tx_buffer: &mut [u8],
    rto_policy: &Rto,
    link: &mut impl NetworkWrite<Error = E>,
) -> Result<(), TransportError<E>>
where
    E: Clone,
    P: BufferPool + 'pool,
    Rto: RtoPolicy,
{
    let now = SysTime::now();

    for seq in sender.with_mut(|s| s.timers.expired(now).collect::<heapless::Vec<_, WIN>>()) {
        sender.with_mut(|s| {
            s.machine.handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(seq),
                },
                &mut s.actions,
            )
        })?;

        transmit(sender, tx_buffer, rto_policy, link).await?;
    }

    Ok(())
}

/// Returns the earliest sender retransmission deadline.
pub(super) fn next_deadline<'pool, E, P, const WIN: usize, const MTU: usize>(
    sender: &SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
) -> Option<SysTime>
where
    P: BufferPool + 'pool,
{
    sender.with(|s| s.timers.next_deadline())
}

/// Drains stored DTN messages into the SRSPP state machine.
pub(super) async fn drain_stored<
    'pool,
    E,
    P,
    S,
    R,
    Rto,
    const WIN: usize,
    const MTU: usize,
>(
    sender: &SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    dtn: &SyncRefCell<DtnContext<S, R>>,
    tx_buffer: &mut [u8],
    origin: Address,
    rto_policy: &Rto,
    link: &mut impl NetworkWrite<Error = E>,
) -> Result<(), TransportError<E>>
where
    E: Clone,
    P: BufferPool + 'pool,
    S: MessageStore,
    R: Reachable,
    Rto: RtoPolicy,
{
    dtn.with_mut(|d| d.store.expire(SysTime::now().seconds()));

    let pending = dtn.with(|d| d.store.pending_targets());
    if pending == 0 {
        return Ok(());
    }

    for station in 0..16u8 {
        if pending & (1 << station) == 0 {
            continue;
        }
        let target = Address::Ground { station };
        if !dtn.with(|d| d.reachable.is_reachable(origin, target)) {
            continue;
        }

        loop {
            let (bytes, window) =
                sender.with(|s| (s.machine.available_bytes(), s.machine.available_window()));
            if window == 0 {
                break;
            }
            let Some(size) = dtn.with(|d| d.store.peek_size(target)) else {
                break;
            };
            if size > bytes {
                break;
            }
            let Some(len) = dtn.with_mut(|d| d.store.read(target, tx_buffer)) else {
                break;
            };
            sender.with_mut(|s| {
                s.machine.handle(
                    SenderEvent::SendRequest {
                        target,
                        data: &tx_buffer[..len],
                    },
                    &mut s.actions,
                )
            })?;
        }

        transmit(sender, tx_buffer, rto_policy, link).await?;
    }

    Ok(())
}

// ── Channel and standalone driver ──

/// Channel that owns the sender state. Split into handle + driver.
pub struct SrsppSender<
    'pool,
    E,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> {
    /// Interior-mutable sender state shared between handle and driver.
    pub(super) state: SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    /// DTN store and reachability oracle.
    dtn: SyncRefCell<DtnContext<S, R>>,
    /// This sender's own address (for reachability checks).
    origin: Address,
    /// Peer address every send goes to. Bound at construction so the
    /// machine's sequence counter is unique to this connection.
    target: Address,
}

/// Alias for a sender without DTN support.
pub type SimpleSender<'pool, E, P, const WIN: usize = 8, const MTU: usize = 512> =
    SrsppSender<'pool, E, NoStore, AlwaysReachable, P, WIN, MTU>;

#[bon::bon]
impl<
    'pool,
    E: Clone,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> SrsppSender<'pool, E, S, R, P, WIN, MTU>
{
    /// Creates a new sender bound to `target`. All sends go to that
    /// peer; the machine's sequence counter is private to this
    /// connection.
    #[builder]
    pub fn new(
        pool: &'pool P,
        buf_size: usize,
        source_address: Address,
        target: Address,
        apid: Apid,
        #[builder(default)] function_code: u8,
        rto_ticks: u32,
        #[builder(default = 3)] max_retransmits: u8,
        #[builder(default = SrsppDataPacket::HEADER_SIZE)] header_overhead: usize,
        store: S,
        reachable: R,
    ) -> Result<Self, P::Error> {
        let config = SenderConfig::builder()
            .source_address(source_address)
            .apid(apid)
            .function_code(function_code)
            .rto_ticks(rto_ticks)
            .max_retransmits(max_retransmits)
            .header_overhead(header_overhead)
            .build();
        Ok(Self {
            state: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(config, pool, buf_size)?,
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
            dtn: SyncRefCell::new(DtnContext { store, reachable }),
            origin: source_address,
            target,
        })
    }

    /// Splits into a handle for sending and a driver for I/O.
    pub fn split<Rto: RtoPolicy>(
        &self,
        rto_policy: Rto,
        pool: &'pool P,
        mtu: usize,
    ) -> Result<(
        SrsppTxHandle<'_, 'pool, E, S, R, P, WIN, MTU>,
        SrsppSenderDriver<'_, 'pool, Rto, E, S, R, P, WIN, MTU>,
    ), P::Error> {
        Ok((
            SrsppTxHandle {
                sender: &self.state,
                dtn: &self.dtn,
                origin: self.origin,
                target: self.target,
            },
            SrsppSenderDriver {
                rto_policy,
                sender: &self.state,
                dtn: &self.dtn,
                origin: self.origin,
                tx_buffer: pool.alloc_bytes(mtu)?,
                recv_buffer: pool.alloc_bytes(mtu)?,
            },
        ))
    }
}

/// Standalone sender driver. Owns its own read loop and recv buffer.
///
/// In combined-node mode, [`SrsppNodeDriver`](super::node::SrsppNodeDriver)
/// drives I/O directly via the free functions in this module — it does
/// not embed this type, so its `recv_buffer` is not allocated when
/// running as part of a node.
pub struct SrsppSenderDriver<
    'a,
    'pool,
    Rto: RtoPolicy,
    E,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> {
    rto_policy: Rto,
    sender: &'a SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    dtn: &'a SyncRefCell<DtnContext<S, R>>,
    origin: Address,
    tx_buffer: P::Buf<'pool>,
    recv_buffer: P::Buf<'pool>,
}

impl<
    'a,
    'pool,
    Rto: RtoPolicy,
    E: Clone,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> SrsppSenderDriver<'a, 'pool, Rto, E, S, R, P, WIN, MTU>
{
    /// Run the standalone driver loop.
    pub async fn run(
        &mut self,
        link: &mut (impl NetworkWrite<Error = E> + NetworkRead<Error = E>),
    ) -> Result<(), TransportError<E>> {
        loop {
            let pending = self.dtn.with(|d| d.store.pending_targets() != 0);
            if self.sender.with(|s| s.closed && s.machine.is_idle()) && !pending {
                return Ok(());
            }

            drain_stored(
                self.sender,
                self.dtn,
                &mut self.tx_buffer[..],
                self.origin,
                &self.rto_policy,
                link,
            )
            .await?;

            if let Err(e) = transmit(
                self.sender,
                &mut self.tx_buffer[..],
                &self.rto_policy,
                link,
            )
            .await
            {
                self.sender.with_mut(|s| s.error = Some(e.clone()));
                return Err(e);
            }

            let timeout = duration_until(next_deadline(self.sender));

            let event = {
                let read_fut = link.read(&mut self.recv_buffer[..]).fuse();
                let sleep_fut = sleep(timeout).fuse();
                pin_utils::pin_mut!(read_fut, sleep_fut);
                futures::select_biased! {
                    r = read_fut => Some(r),
                    _ = sleep_fut => None,
                }
            };

            match event {
                Some(Ok(len)) => {
                    if let Err(e) = process_ack(self.sender, &self.recv_buffer[..len]) {
                        self.sender.with_mut(|s| s.error = Some(e.clone()));
                        return Err(e);
                    }
                }
                Some(Err(e)) => {
                    let err = TransportError::Network(e);
                    self.sender.with_mut(|s| s.error = Some(err.clone()));
                    return Err(err);
                }
                None => {
                    if let Err(e) = handle_timeouts(
                        self.sender,
                        &mut self.tx_buffer[..],
                        &self.rto_policy,
                        link,
                    )
                    .await
                    {
                        self.sender.with_mut(|s| s.error = Some(e.clone()));
                        return Err(e);
                    }
                }
            }
        }
    }
}

/// Converts an optional deadline into a duration from now.
pub(super) fn duration_until(deadline: Option<SysTime>) -> Duration {
    let now = SysTime::now();
    deadline
        .map(|d| {
            if d > now {
                Duration::from(d - now)
            } else {
                Duration::zero()
            }
        })
        .unwrap_or(Duration::from_millis(100))
}

/// Handle for sending data over an SRSPP node.
pub struct SrsppTxHandle<
    'a,
    'pool,
    E,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> {
    pub(super) sender: &'a SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
    pub(super) dtn: &'a SyncRefCell<DtnContext<S, R>>,
    pub(super) origin: Address,
    pub(super) target: Address,
}

impl<
    'a,
    'pool,
    E,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> Clone for SrsppTxHandle<'a, 'pool, E, S, R, P, WIN, MTU>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<
    'a,
    'pool,
    E,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> Copy for SrsppTxHandle<'a, 'pool, E, S, R, P, WIN, MTU>
{
}

impl<
    'a,
    'pool,
    E: Clone,
    S: MessageStore,
    R: Reachable,
    P: BufferPool + 'pool,
    const WIN: usize,
    const MTU: usize,
> SrsppTxHandle<'a, 'pool, E, S, R, P, WIN, MTU>
{
    /// Sends data to the bound target.
    ///
    /// If the destination is unreachable, the message is
    /// stored for later delivery by the driver. If reachable,
    /// it enters SRSPP normally.
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

        // Normal SRSPP path
        poll_fn(|_cx| {
            self.sender.with(|s| {
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

        self.sender.with_mut(|s| {
            s.machine
                .handle(SenderEvent::SendRequest { target, data }, &mut s.actions)
        })?;
        Ok(())
    }

    /// Send an end-of-stream packet to the bound target. Allocates a
    /// window slot for an EOS packet (own seq, no payload) and queues
    /// it for transmission. The peer's receiver discards its
    /// per-stream state once the cumulative ACK covers the EOS
    /// sequence number, so the next message from this source restarts
    /// cleanly at seq=0.
    pub async fn send_eos(&mut self) -> Result<(), TransportError<E>> {
        let target = self.target;
        poll_fn(|_cx| {
            self.sender.with(|s| {
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

        self.sender.with_mut(|s| {
            s.machine
                .handle(SenderEvent::SendEos { target }, &mut s.actions)
        })?;
        Ok(())
    }

    /// Signal that no more data will be sent.
    /// Driver will exit once all pending data is acknowledged
    /// and the store is drained.
    pub fn close(&mut self) {
        self.sender.with_mut(|s| s.closed = true);
    }

    /// Check available buffer space in bytes.
    pub fn available_bytes(&self) -> usize {
        self.sender.with(|s| s.machine.available_bytes())
    }

    /// Check available window slots.
    pub fn available_window(&self) -> usize {
        self.sender.with(|s| s.machine.available_window())
    }

    /// Check if all data has been acknowledged.
    pub fn is_idle(&self) -> bool {
        self.sender.with(|s| s.machine.is_idle())
    }
}
