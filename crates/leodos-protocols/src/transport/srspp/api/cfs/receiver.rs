//! Internal receive state and helpers shared by [`super::endpoint::SrsppEndpoint`].
//!
//! Public entry points are `SrsppEndpoint::receiver(source)` and
//! `SrsppEndpoint::listener()` in [`super::endpoint`]. Nothing in
//! this file is `pub` to crate consumers — only the endpoint reaches
//! in here.

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;

use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::AckInfo;
use crate::transport::srspp::machine::receiver::AckState;
use crate::transport::srspp::machine::receiver::HandleResult;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::TimerAction;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::utils::cell::SyncRefCell;
use heapless::LinearMap;

use super::TransportError;

/// Buffer size for outgoing ACK packets.
pub(super) const ACK_BUFFER_SIZE: usize = 32;

/// Per-stream receiver state for a single remote sender.
pub(super) struct StreamState<R: ReceiverBackend> {
    pub(super) machine: R,
    pub(super) ack_state: AckState,
    pub(super) ack_deadline: Option<SysTime>,
    pub(super) progress_deadline: Option<SysTime>,
}

/// Multi-source receive state shared by listener and explicit receiver views.
pub(super) struct MultiReceiverState<E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    pub(super) config: ReceiverConfig,
    pub(super) streams: LinearMap<Address, StreamState<R>, MAX_STREAMS>,
    pub(super) ack_delay: Duration,
    pub(super) error: Option<TransportError<E>>,
}

/// Feeds an incoming DATA packet into the matching per-source stream
/// (creating one if needed) and emits any ACK or timer updates the
/// state machine asks for.
pub(super) async fn process_data<E, R, const MAX_STREAMS: usize>(
    state: &SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    ack_buffer: &mut [u8],
    packet: &[u8],
    link: &mut impl NetworkWrite<Error = E>,
) -> Result<(), TransportError<E>>
where
    E: Clone,
    R: ReceiverBackend,
{
    if let Ok(SrsppType::Data) = SrsppPacket::parse(packet).and_then(|p| p.srspp_type()) {
        if let Ok(data) = SrsppDataPacket::parse(packet) {
            let source_address = data.srspp_header.source_address();
            let seq = data.primary.sequence_count();
            let flags = data.primary.sequence_flag();

            let result =
                state.with_mut(|s| -> Result<HandleResult, TransportError<E>> {
                    if !s.streams.contains_key(&source_address) {
                        let _ = s.streams.insert(
                            source_address,
                            StreamState {
                                machine: R::new(),
                                ack_state: AckState::new(&s.config, source_address),
                                ack_deadline: None,
                                progress_deadline: None,
                            },
                        );
                    }
                    if let Some(stream) = s.streams.get_mut(&source_address) {
                        let outcome = stream.machine.handle_data(seq, flags, &data.payload)?;
                        Ok(stream.ack_state.on_data(
                            outcome,
                            stream.machine.expected_seq(),
                            stream.machine.recv_bitmap(),
                        ))
                    } else {
                        Ok(HandleResult::default())
                    }
                })?;

            drive_actions(state, ack_buffer, source_address, result, link).await?;
        }
    }

    Ok(())
}

/// Walks expired ACK and progress timers across all streams.
pub(super) async fn handle_timeouts<E, R, const MAX_STREAMS: usize>(
    state: &SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    ack_buffer: &mut [u8],
    link: &mut impl NetworkWrite<Error = E>,
) -> Result<(), TransportError<E>>
where
    E: Clone,
    R: ReceiverBackend,
{
    let now = SysTime::now();

    let ack_expired = state.with(|s| {
        s.streams
            .iter()
            .filter_map(|(source, stream)| {
                stream.ack_deadline.filter(|&d| now >= d).map(|_| *source)
            })
            .collect::<heapless::Vec<_, MAX_STREAMS>>()
    });

    for source in ack_expired {
        let result = state.with_mut(|s| {
            if let Some(stream) = s.streams.get_mut(&source) {
                stream.ack_deadline = None;
                stream
                    .ack_state
                    .on_ack_timeout(stream.machine.expected_seq(), stream.machine.recv_bitmap())
            } else {
                HandleResult::default()
            }
        });
        drive_actions(state, ack_buffer, source, result, link).await?;
    }

    let progress_expired = state.with(|s| {
        s.streams
            .iter()
            .filter_map(|(source, stream)| {
                stream
                    .progress_deadline
                    .filter(|&d| now >= d)
                    .map(|_| *source)
            })
            .collect::<heapless::Vec<_, MAX_STREAMS>>()
    });

    for source in progress_expired {
        let result = state
            .with_mut(|s| -> Result<HandleResult, TransportError<E>> {
                if let Some(stream) = s.streams.get_mut(&source) {
                    stream.progress_deadline = None;
                    let outcome = stream.machine.skip_gap()?;
                    Ok(stream.ack_state.on_gap_skip(outcome))
                } else {
                    Ok(HandleResult::default())
                }
            })?;
        drive_actions(state, ack_buffer, source, result, link).await?;
    }

    Ok(())
}

/// Earliest deadline across all streams (ACK or progress).
pub(super) fn next_deadline<E, R, const MAX_STREAMS: usize>(
    state: &SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
) -> Option<SysTime>
where
    R: ReceiverBackend,
{
    state.with(|s| {
        s.streams
            .iter()
            .map(|(_, s)| s)
            .flat_map(|s| [s.ack_deadline, s.progress_deadline])
            .flatten()
            .min()
    })
}

async fn drive_actions<E, R, const MAX_STREAMS: usize>(
    state: &SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    ack_buffer: &mut [u8],
    source: Address,
    result: HandleResult,
    link: &mut impl NetworkWrite<Error = E>,
) -> Result<(), TransportError<E>>
where
    E: Clone,
    R: ReceiverBackend,
{
    if let Some(AckInfo {
        destination,
        cumulative_ack,
        selective_bitmap,
    }) = result.ack
    {
        let (local_address, apid, function_code) = state.with(|s| {
            (
                s.config.local_address,
                s.config.apid,
                s.config.function_code,
            )
        });
        let ack = SrsppAckPacket::builder()
            .buffer(ack_buffer)
            .source_address(local_address)
            .target(destination)
            .apid(apid)
            .function_code(function_code)
            .cumulative_ack(cumulative_ack)
            .selective_bitmap(selective_bitmap)
            .sequence_count(SequenceCount::from(0))
            .build()?;
        link.write(zerocopy::IntoBytes::as_bytes(ack))
            .await
            .map_err(TransportError::Network)?;
    }

    let ack_delay = state.with(|s| s.ack_delay);
    state.with_mut(|s| {
        if let Some(action) = result.ack_timer {
            if let Some(entry) = s.streams.get_mut(&source) {
                entry.ack_deadline = match action {
                    TimerAction::Start { .. } => {
                        Some(SysTime::now() + SysTime::from(ack_delay))
                    }
                    TimerAction::Stop => None,
                };
            }
        }
        if let Some(action) = result.progress_timer {
            if let Some(entry) = s.streams.get_mut(&source) {
                entry.progress_deadline = match action {
                    TimerAction::Start { ticks } => {
                        let delay = Duration::from_millis(ticks);
                        Some(SysTime::now() + SysTime::from(delay))
                    }
                    TimerAction::Stop => None,
                };
            }
        }
    });

    Ok(())
}
