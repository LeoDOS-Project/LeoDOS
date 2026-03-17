use core::future::poll_fn;
use core::task::Poll;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::application::spacecomp::io::writer::MessageSender;
use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
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
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::rto::RtoPolicy;
use crate::utils::cell::SyncRefCell;

use super::TimerSet;

/// Shared mutable state for the sender channel.
pub(super) struct SenderState<E, const WIN: usize, const BUF: usize, const MTU: usize> {
    /// Sender state machine.
    pub(crate) machine: SenderMachine<WIN, BUF, MTU>,
    /// Pending actions produced by the state machine.
    pub(crate) actions: SenderActions,
    /// Retransmission timers for in-flight packets.
    pub(crate) timers: TimerSet<WIN>,
    /// Whether the handle has signaled no more data.
    pub(crate) closed: bool,
    /// First error encountered, propagated to the handle.
    pub(crate) error: Option<TransportError<E>>,
}

// ── Shared free functions used by both SrsppSenderDriver and SrsppNodeDriver ──

/// Sends all pending transmit actions over the link.
pub(super) async fn drive_transmits<
    E: Clone,
    L: NetworkWrite<Error = E> + NetworkRead<Error = E>,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    tx_buf: &mut [u8],
    link: &mut L,
    rto: &P,
) -> Result<(), TransportError<E>> {
    let now = SysTime::now();

    let (transmits, cfg_clone): (heapless::Vec<SequenceCount, WIN>, SenderConfig) =
        state.with(|s| {
            let t = s
                .actions
                .iter()
                .filter_map(|a| match a {
                    SenderAction::Transmit { seq, .. } => Some(*seq),
                    _ => None,
                })
                .collect();
            (t, s.machine.config().clone())
        });

    for seq in transmits {
        let packet_len = state.with(|s| {
            if let Some(info) = s.machine.get_payload(seq) {
                let pkt = SrsppDataPacket::builder()
                    .buffer(tx_buf)
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
                Ok::<_, TransportError<E>>(Some(SrsppDataPacket::HEADER_SIZE + info.payload.len()))
            } else {
                Ok::<_, TransportError<E>>(None)
            }
        })?;

        if let Some(packet_len) = packet_len {
            link.write(&tx_buf[..packet_len])
                .await
                .map_err(TransportError::Network)?;

            let rto_dur = Duration::from_millis(rto.rto_ticks(now.seconds()));

            state.with_mut(|s| {
                s.machine.mark_transmitted(seq);
                s.timers.start(seq, now + SysTime::from(rto_dur));
            });
        }
    }

    state.with_mut(|s| {
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
pub(super) fn drive_ack<E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    packet: &[u8],
) -> Result<(), TransportError<E>> {
    if let Ok(SrsppType::Ack) = SrsppPacket::parse(packet).and_then(|p| p.srspp_type()) {
        if let Ok(ack) = SrsppAckPacket::parse(packet) {
            state.with_mut(|s| {
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
pub(super) async fn drive_sender_timeouts<
    E: Clone,
    L: NetworkWrite<Error = E> + NetworkRead<Error = E>,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    tx_buf: &mut [u8],
    link: &mut L,
    rto: &P,
) -> Result<(), TransportError<E>> {
    let now = SysTime::now();

    for seq in state.with_mut(|s| s.timers.expired(now).collect::<heapless::Vec<_, WIN>>()) {
        state.with_mut(|s| {
            s.machine.handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(seq),
                },
                &mut s.actions,
            )
        })?;

        drive_transmits(state, tx_buf, link, rto).await?;
    }

    Ok(())
}

/// Returns the earliest sender retransmission deadline.
pub(super) fn sender_next_deadline<E, const WIN: usize, const BUF: usize, const MTU: usize>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
) -> Option<SysTime> {
    state.with(|s| s.timers.next_deadline())
}

// ── Channel and driver ──

/// Channel that owns the sender state. Split into handle + driver.
pub struct SrsppSender<
    E,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    /// Interior-mutable sender state shared between handle and driver.
    pub(super) state: SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    /// DTN store and reachability oracle.
    dtn: SyncRefCell<DtnContext<S, R>>,
    /// This sender's own address (for reachability checks).
    origin: Address,
}

pub(super) struct DtnContext<S, R> {
    pub(super) store: S,
    pub(super) reachable: R,
}

/// Alias for a sender without DTN support.
pub type SimpleSender<E, const WIN: usize = 8, const BUF: usize = 4096, const MTU: usize = 512> =
    SrsppSender<E, NoStore, AlwaysReachable, WIN, BUF, MTU>;

impl<E: Clone, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSender<E, S, R, WIN, BUF, MTU>
{
    /// Creates a new sender.
    pub fn new(config: SenderConfig, origin: Address, store: S, reachable: R) -> Self {
        Self {
            state: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
            dtn: SyncRefCell::new(DtnContext { store, reachable }),
            origin,
        }
    }

    /// Splits into a handle for sending and a driver for I/O.
    pub fn split<L: NetworkWrite<Error = E> + NetworkRead<Error = E>, P: RtoPolicy>(
        &self,
        link: L,
        rto_policy: P,
    ) -> (
        SrsppTxHandle<'_, E, S, R, WIN, BUF, MTU>,
        SrsppSenderDriver<'_, L, P, E, S, R, WIN, BUF, MTU>,
    ) {
        (
            SrsppTxHandle {
                sender: &self.state,
                dtn: &self.dtn,
                origin: self.origin,
            },
            SrsppSenderDriver {
                link,
                rto_policy,
                sender: &self.state,
                dtn: &self.dtn,
                origin: self.origin,
                recv_buffer: [0u8; MTU],
                tx_buffer: [0u8; MTU],
            },
        )
    }
}

/// Driver that handles I/O and DTN drain. Runs as a concurrent task.
pub struct SrsppSenderDriver<
    'a,
    L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>,
    P: RtoPolicy,
    E,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    link: L,
    rto_policy: P,
    sender: &'a SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    dtn: &'a SyncRefCell<DtnContext<S, R>>,
    origin: Address,
    recv_buffer: [u8; MTU],
    tx_buffer: [u8; MTU],
}

impl<
    'a,
    L: NetworkWrite + NetworkRead<Error = <L as NetworkWrite>::Error>,
    P: RtoPolicy,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> SrsppSenderDriver<'a, L, P, <L as NetworkWrite>::Error, S, R, WIN, BUF, MTU>
where
    <L as NetworkWrite>::Error: Clone,
{
    /// Run the driver loop.
    pub async fn run(&mut self) -> Result<(), TransportError<<L as NetworkWrite>::Error>> {
        let state = self.sender;
        loop {
            let pending = self.dtn.with(|d| d.store.pending_targets() != 0);
            if state.with(|s| s.closed && s.machine.is_idle()) && !pending {
                return Ok(());
            }

            // Drain stored messages for reachable targets
            self.drain_stored(state).await?;

            if let Err(e) =
                drive_transmits(state, &mut self.tx_buffer, &mut self.link, &self.rto_policy).await
            {
                state.with_mut(|s| s.error = Some(e.clone()));
                return Err(e);
            }

            let timeout = duration_until(sender_next_deadline(state));

            match select_either(self.link.read(&mut self.recv_buffer), sleep(timeout)).await {
                Either::Left(result) => match result {
                    Ok(len) => {
                        let packet = &self.recv_buffer[..len];
                        if let Err(e) = drive_ack(state, packet) {
                            state.with_mut(|s| s.error = Some(e.clone()));
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        let err = TransportError::Network(e);
                        state.with_mut(|s| s.error = Some(err.clone()));
                        return Err(err);
                    }
                },
                Either::Right(()) => {
                    if let Err(e) = drive_sender_timeouts(
                        state,
                        &mut self.tx_buffer,
                        &mut self.link,
                        &self.rto_policy,
                    )
                    .await
                    {
                        state.with_mut(|s| s.error = Some(e.clone()));
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Drain stored messages into the SRSPP state machine.
    async fn drain_stored(
        &mut self,
        state: &SyncRefCell<SenderState<<L as NetworkWrite>::Error, WIN, BUF, MTU>>,
    ) -> Result<(), TransportError<<L as NetworkWrite>::Error>> {
        self.dtn
            .with_mut(|d| d.store.expire(SysTime::now().seconds()));

        let pending = self.dtn.with(|d| d.store.pending_targets());
        if pending == 0 {
            return Ok(());
        }

        for station in 0..16u8 {
            if pending & (1 << station) == 0 {
                continue;
            }
            let target = Address::Ground { station };
            if !self
                .dtn
                .with(|d| d.reachable.is_reachable(self.origin, target))
            {
                continue;
            }

            loop {
                let (bytes, window) =
                    state.with(|s| (s.machine.available_bytes(), s.machine.available_window()));
                if window == 0 {
                    break;
                }
                let Some(size) = self.dtn.with(|d| d.store.peek_size(target)) else {
                    break;
                };
                if size > bytes {
                    break;
                }
                let Some(len) = self
                    .dtn
                    .with_mut(|d| d.store.read(target, &mut self.tx_buffer))
                else {
                    break;
                };
                state.with_mut(|s| {
                    s.machine.handle(
                        SenderEvent::SendRequest {
                            target,
                            data: &self.tx_buffer[..len],
                        },
                        &mut s.actions,
                    )
                })?;
            }

            drive_transmits(state, &mut self.tx_buffer, &mut self.link, &self.rto_policy).await?;
        }

        Ok(())
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
        .unwrap_or(Duration::from_secs(60))
}

/// Handle for sending data over an SRSPP node.
pub struct SrsppTxHandle<
    'a,
    E,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    pub(super) sender: &'a SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    pub(super) dtn: &'a SyncRefCell<DtnContext<S, R>>,
    pub(super) origin: Address,
}

impl<
    'a,
    E: Clone,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> SrsppTxHandle<'a, E, S, R, WIN, BUF, MTU>
{
    /// Sends data to the given target.
    ///
    /// If the destination is unreachable, the message is
    /// stored for later delivery by the driver. If reachable,
    /// it enters SRSPP normally.
    pub async fn send(
        &mut self,
        target: impl Into<Address>,
        data: &(impl IntoBytes + Immutable + ?Sized),
    ) -> Result<(), TransportError<E>> {
        let target = target.into();
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

impl<
    'a,
    E: Clone,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> MessageSender for SrsppTxHandle<'a, E, S, R, WIN, BUF, MTU>
{
    type Error = TransportError<E>;

    async fn send_message(&mut self, target: Address, data: &[u8]) -> Result<(), Self::Error> {
        self.send(target, data).await
    }
}
