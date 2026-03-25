use core::future::poll_fn;
use core::task::Poll;

use zerocopy::Immutable;
use zerocopy::IntoBytes;

use futures::FutureExt;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::time::sleep;

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

#[bon::bon]
impl<E: Clone, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSender<E, S, R, WIN, BUF, MTU>
{
    /// Creates a new sender.
    #[builder]
    pub fn new(
        source_address: Address,
        apid: Apid,
        #[builder(default)] function_code: u8,
        rto_ticks: u32,
        #[builder(default = 3)] max_retransmits: u8,
        #[builder(default = SrsppDataPacket::HEADER_SIZE)] header_overhead: usize,
        store: S,
        reachable: R,
    ) -> Self {
        let config = SenderConfig::builder()
            .source_address(source_address)
            .apid(apid)
            .function_code(function_code)
            .rto_ticks(rto_ticks)
            .max_retransmits(max_retransmits)
            .header_overhead(header_overhead)
            .build();
        Self {
            state: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
            dtn: SyncRefCell::new(DtnContext { store, reachable }),
            origin: source_address,
        }
    }

    /// Splits into a handle for sending and a driver for I/O.
    pub fn split<P: RtoPolicy>(
        &self,
        rto_policy: P,
    ) -> (
        SrsppTxHandle<'_, E, S, R, WIN, BUF, MTU>,
        SrsppSenderDriver<'_, P, E, S, R, WIN, BUF, MTU>,
    ) {
        (
            SrsppTxHandle {
                sender: &self.state,
                dtn: &self.dtn,
                origin: self.origin,
            },
            SrsppSenderDriver::new(rto_policy, &self.state, &self.dtn, self.origin),
        )
    }
}

/// Driver that handles I/O and DTN drain. Runs as a concurrent task.
pub struct SrsppSenderDriver<
    'a,
    P: RtoPolicy,
    E,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    rto_policy: P,
    pub(super) sender: &'a SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    dtn: &'a SyncRefCell<DtnContext<S, R>>,
    origin: Address,
    tx_buffer: [u8; MTU],
}

impl<
    'a,
    P: RtoPolicy,
    E,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> SrsppSenderDriver<'a, P, E, S, R, WIN, BUF, MTU>
{
    pub(super) fn new(
        rto_policy: P,
        sender: &'a SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
        dtn: &'a SyncRefCell<DtnContext<S, R>>,
        origin: Address,
    ) -> Self {
        Self {
            rto_policy,
            sender,
            dtn,
            origin,
            tx_buffer: [0u8; MTU],
        }
    }
}

impl<
    'a,
    P: RtoPolicy,
    E: Clone,
    S: MessageStore,
    R: Reachable,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> SrsppSenderDriver<'a, P, E, S, R, WIN, BUF, MTU>
{
    /// Sends all pending transmit actions over the link.
    pub(super) async fn transmit(
        &mut self,
        link: &mut impl NetworkWrite<Error = E>,
    ) -> Result<(), TransportError<E>> {
        let now = SysTime::now();

        let (transmits, cfg_clone) = self.sender.with(|s| {
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
            let packet_len = self.sender.with(|s| {
                if let Some(info) = s.machine.get_payload(seq) {
                    let pkt = SrsppDataPacket::builder()
                        .buffer(&mut self.tx_buffer)
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
                } else {
                    Ok::<_, TransportError<E>>(None)
                }
            })?;

            if let Some(packet_len) = packet_len {
                link.write(&self.tx_buffer[..packet_len])
                    .await
                    .map_err(TransportError::Network)?;

                let rto_dur = Duration::from_millis(self.rto_policy.rto_ticks(now.seconds()));

                self.sender.with_mut(|s| {
                    s.machine.mark_transmitted(seq);
                    s.timers.start(seq, now + SysTime::from(rto_dur));
                });
            }
        }

        self.sender.with_mut(|s| {
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
    pub(super) fn process_ack(&mut self, packet: &[u8]) -> Result<(), TransportError<E>> {
        if let Ok(SrsppType::Ack) = SrsppPacket::parse(packet).and_then(|p| p.srspp_type()) {
            if let Ok(ack) = SrsppAckPacket::parse(packet) {
                self.sender.with_mut(|s| {
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
    pub(super) async fn handle_timeouts(
        &mut self,
        link: &mut impl NetworkWrite<Error = E>,
    ) -> Result<(), TransportError<E>> {
        let now = SysTime::now();

        for seq in self
            .sender
            .with_mut(|s| s.timers.expired(now).collect::<heapless::Vec<_, WIN>>())
        {
            self.sender.with_mut(|s| {
                s.machine.handle(
                    SenderEvent::RetransmitTimeout {
                        seq: SequenceCount::from(seq),
                    },
                    &mut s.actions,
                )
            })?;

            self.transmit(link).await?;
        }

        Ok(())
    }

    /// Returns the earliest sender retransmission deadline.
    pub(super) fn next_deadline(&self) -> Option<SysTime> {
        self.sender.with(|s| s.timers.next_deadline())
    }

    /// Drain stored messages into the SRSPP state machine.
    pub(super) async fn drain_stored(
        &mut self,
        link: &mut impl NetworkWrite<Error = E>,
    ) -> Result<(), TransportError<E>> {
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
                let (bytes, window) = self
                    .sender
                    .with(|s| (s.machine.available_bytes(), s.machine.available_window()));
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
                self.sender.with_mut(|s| {
                    s.machine.handle(
                        SenderEvent::SendRequest {
                            target,
                            data: &self.tx_buffer[..len],
                        },
                        &mut s.actions,
                    )
                })?;
            }

            self.transmit(link).await?;
        }

        Ok(())
    }

    /// Run the driver loop.
    pub async fn run(
        &mut self,
        link: &mut (impl NetworkWrite<Error = E> + NetworkRead<Error = E>),
    ) -> Result<(), TransportError<E>> {
        let mut recv_buffer = [0u8; MTU];
        loop {
            let pending = self.dtn.with(|d| d.store.pending_targets() != 0);
            if self.sender.with(|s| s.closed && s.machine.is_idle()) && !pending {
                return Ok(());
            }

            self.drain_stored(link).await?;

            if let Err(e) = self.transmit(link).await {
                self.sender.with_mut(|s| s.error = Some(e.clone()));
                return Err(e);
            }

            let timeout = duration_until(self.next_deadline());

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
                    if let Err(e) = self.process_ack(&recv_buffer[..len]) {
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
                    if let Err(e) = self.handle_timeouts(link).await {
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

impl<'a, E, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize>
    Clone for SrsppTxHandle<'a, E, S, R, WIN, BUF, MTU>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, E, S: MessageStore, R: Reachable, const WIN: usize, const BUF: usize, const MTU: usize>
    Copy for SrsppTxHandle<'a, E, S, R, WIN, BUF, MTU>
{
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
