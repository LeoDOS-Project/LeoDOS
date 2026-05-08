//! Internal sender state and helpers shared by [`super::endpoint::SrsppEndpoint`].
//!
//! Public entry point is `SrsppEndpoint::sender(target)` in
//! [`super::endpoint`]. Nothing in this file is `pub` to crate
//! consumers — only the endpoint reaches in here.

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;

use crate::buffer_pool::BufferPool;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::api::cfs::TransportError;
use crate::transport::srspp::dtn::MessageStore;
use crate::transport::srspp::dtn::Reachable;
use crate::transport::srspp::machine::sender::SenderAction;
use crate::transport::srspp::machine::sender::SenderActions;
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

/// Shared mutable state for one sender slot.
pub(super) struct SenderState<'pool, E, P: BufferPool + 'pool, const WIN: usize, const MTU: usize> {
    pub(crate) machine: SenderMachine<'pool, P, WIN, MTU>,
    pub(crate) actions: SenderActions,
    pub(crate) timers: TimerSet<WIN>,
    pub(crate) closed: bool,
    pub(crate) error: Option<TransportError<E>>,
}

/// DTN store + reachability oracle shared by all sender slots.
pub(super) struct DtnContext<S, R> {
    pub(super) store: S,
    pub(super) reachable: R,
}

/// Drains pending Transmit actions onto `link`, marking each packet
/// as transmitted and arming its retransmission timer.
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

/// Feeds a received ACK packet into the sender state machine.
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

/// Walks expired retransmission timers and drives the corresponding
/// retransmits through [`transmit`].
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

/// Earliest pending retransmission deadline.
pub(super) fn next_deadline<'pool, E, P, const WIN: usize, const MTU: usize>(
    sender: &SyncRefCell<SenderState<'pool, E, P, WIN, MTU>>,
) -> Option<SysTime>
where
    P: BufferPool + 'pool,
{
    sender.with(|s| s.timers.next_deadline())
}

/// Pulls deferred messages out of the DTN store for any reachable
/// target and feeds them into the sender state machine.
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

/// Converts an optional deadline into a duration from now, capping at
/// 100 ms when no deadline is set so the run loop polls outbound work
/// even without retransmit timers.
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
