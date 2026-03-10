use core::future::poll_fn;
use core::task::Poll;

use zerocopy::{Immutable, IntoBytes};

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::application::spacecomp::io::writer::MessageSender;
use crate::network::{NetworkReader, NetworkWriter};
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
use crate::transport::srspp::machine::sender::SenderConfig;
use crate::transport::srspp::machine::sender::SenderEvent;
use crate::transport::srspp::machine::sender::SenderMachine;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::rto::RtoPolicy;
use crate::utils::cell::SyncRefCell;

use super::Error;
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
    pub(crate) error: Option<Error<E>>,
}

// ── Shared free functions used by both SrsppSenderDriver and SrsppNodeDriver ──

/// Sends all pending transmit actions over the link.
pub(super) async fn drive_transmits<
    E: Clone,
    L: NetworkWriter<Error = E> + NetworkReader<Error = E>,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    tx_buf: &mut [u8],
    link: &mut L,
    rto: &P,
) -> Result<(), Error<E>> {
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
                    .map_err(Error::Packet)?;
                pkt.payload.copy_from_slice(info.payload);
                Ok::<_, Error<E>>(Some(SrsppDataPacket::HEADER_SIZE + info.payload.len()))
            } else {
                Ok::<_, Error<E>>(None)
            }
        })?;

        if let Some(packet_len) = packet_len {
            link.send(&tx_buf[..packet_len])
                .await
                .map_err(Error::Link)?;

            let rto_dur =
                Duration::from_millis(rto.rto_ticks(now.seconds()));

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
pub(super) fn drive_ack<
    E: Clone,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    packet: &[u8],
) -> Result<(), Error<E>> {
    if let Ok(SrsppType::Ack) = SrsppPacket::parse(packet).and_then(|p| p.srspp_type()) {
        if let Ok(ack) = SrsppAckPacket::parse(packet) {
            state.with_mut(|s| {
                let SenderState {
                    machine,
                    actions,
                    timers,
                    ..
                } = s;

                machine.handle(
                    SenderEvent::AckReceived {
                        cumulative_ack: ack.ack_payload.cumulative_ack(),
                        selective_bitmap: ack
                            .ack_payload
                            .selective_ack_bitmap(),
                    },
                    actions,
                )?;

                for action in actions.iter() {
                    let &SenderAction::StopTimer { seq } = action else {
                        continue;
                    };
                    timers.stop(seq);
                }
                Ok::<(), Error<E>>(())
            })?;
        }
    }
    Ok(())
}

/// Processes expired retransmission timers and retransmits.
pub(super) async fn drive_sender_timeouts<
    E: Clone,
    L: NetworkWriter<Error = E> + NetworkReader<Error = E>,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
    tx_buf: &mut [u8],
    link: &mut L,
    rto: &P,
) -> Result<(), Error<E>> {
    let now = SysTime::now();

    let expired: heapless::Vec<SequenceCount, WIN> =
        state.with_mut(|s| s.timers.expired(now).collect());

    for seq in expired {
        state.with_mut(|s| {
            let SenderState {
                machine, actions, ..
            } = s;
            machine.handle(
                SenderEvent::RetransmitTimeout {
                    seq: SequenceCount::from(seq),
                },
                actions,
            )
        })?;

        drive_transmits(state, tx_buf, link, rto).await?;
    }

    Ok(())
}

/// Returns the earliest sender retransmission deadline.
pub(super) fn sender_next_deadline<
    E,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
>(
    state: &SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
) -> Option<SysTime> {
    state.with(|s| s.timers.next_deadline())
}

// ── Channel and driver ──

/// Channel that owns the sender state. Split into handle + driver.
pub struct SrsppSender<E, const WIN: usize = 8, const BUF: usize = 4096, const MTU: usize = 512> {
    /// Interior-mutable sender state shared between handle and driver.
    state: SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
}

impl<E: Clone, const WIN: usize, const BUF: usize, const MTU: usize> SrsppSender<E, WIN, BUF, MTU> {
    /// Creates a new sender with the given configuration.
    pub fn new(config: SenderConfig) -> Self {
        Self {
            state: SyncRefCell::new(SenderState {
                machine: SenderMachine::new(config),
                actions: SenderActions::new(),
                timers: TimerSet::new(),
                closed: false,
                error: None,
            }),
        }
    }

    /// Splits into a handle for sending and a driver for I/O.
    pub fn split<L: NetworkWriter<Error = E> + NetworkReader<Error = E>, P: RtoPolicy>(
        &self,
        link: L,
        rto_policy: P,
    ) -> (
        SrsppTxHandle<'_, E, WIN, BUF, MTU>,
        SrsppSenderDriver<'_, L, P, WIN, BUF, MTU>,
    ) {
        (
            SrsppTxHandle {
                sender: &self.state,
            },
            SrsppSenderDriver {
                link,
                rto_policy,
                channel: self,
                recv_buffer: [0u8; MTU],
                tx_buffer: [0u8; MTU],
            },
        )
    }
}

/// Driver that handles I/O. Runs as a concurrent task.
pub struct SrsppSenderDriver<
    'a,
    L: NetworkWriter + NetworkReader<Error = <L as NetworkWriter>::Error>,
    P: RtoPolicy,
    const WIN: usize,
    const BUF: usize,
    const MTU: usize,
> {
    /// Network link for sending and receiving packets.
    link: L,
    /// Policy for computing retransmission timeouts.
    rto_policy: P,
    /// Reference to the shared sender channel.
    channel: &'a SrsppSender<<L as NetworkWriter>::Error, WIN, BUF, MTU>,
    /// Buffer for receiving ACK packets from the link.
    recv_buffer: [u8; MTU],
    /// Buffer for building outgoing data packets.
    tx_buffer: [u8; MTU],
}

impl<'a, L: NetworkWriter + NetworkReader<Error = <L as NetworkWriter>::Error>, P: RtoPolicy, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppSenderDriver<'a, L, P, WIN, BUF, MTU>
where
    <L as NetworkWriter>::Error: Clone,
{
    /// Run the driver loop.
    pub async fn run(&mut self) -> Result<(), Error<<L as NetworkWriter>::Error>> {
        let state = &self.channel.state;
        loop {
            if state.with(|s| s.closed && s.machine.is_idle()) {
                return Ok(());
            }

            if let Err(e) = drive_transmits(
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

            let timeout = duration_until(sender_next_deadline(state));

            match select_either(
                self.link.recv(&mut self.recv_buffer),
                sleep(timeout),
            )
            .await
            {
                Either::Left(result) => match result {
                    Ok(len) => {
                        let packet = &self.recv_buffer[..len];
                        if let Err(e) = drive_ack(state, packet) {
                            state.with_mut(|s| s.error = Some(e.clone()));
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        let err = Error::Link(e);
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
pub struct SrsppTxHandle<'a, E, const WIN: usize, const BUF: usize, const MTU: usize> {
    /// Reference to the shared sender state.
    pub(super) sender: &'a SyncRefCell<SenderState<E, WIN, BUF, MTU>>,
}

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize>
    SrsppTxHandle<'a, E, WIN, BUF, MTU>
{
    /// Sends data to the given target, waiting for buffer space.
    pub async fn send(
        &mut self,
        target: impl Into<Address>,
        data: &(impl IntoBytes + Immutable + ?Sized),
    ) -> Result<(), Error<E>> {
        let data = data.as_bytes();
        poll_fn(|_cx| {
            self.sender.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                if s.machine.available_bytes() >= data.len()
                    && s.machine.available_window() > 0
                {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            })
        })
        .await?;

        self.sender.with_mut(|s| {
            let SenderState {
                machine, actions, ..
            } = s;
            machine.handle(
                SenderEvent::SendRequest {
                    target: target.into(),
                    data,
                },
                actions,
            )
        })?;
        Ok(())
    }

    /// Signal that no more data will be sent.
    /// Driver will exit once all pending data is acknowledged.
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

impl<'a, E: Clone, const WIN: usize, const BUF: usize, const MTU: usize> MessageSender
    for SrsppTxHandle<'a, E, WIN, BUF, MTU>
{
    type Error = Error<E>;

    async fn send_message(&mut self, target: Address, data: &[u8]) -> Result<(), Self::Error> {
        self.send(target, data).await
    }
}
