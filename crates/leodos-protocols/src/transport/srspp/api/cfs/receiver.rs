use core::future::poll_fn;
use core::task::Poll;

use futures::FutureExt;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::time::sleep;

use crate::network::NetworkRead;
use crate::network::NetworkWrite;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::srspp::machine::receiver::AckInfo;
use crate::transport::srspp::machine::receiver::AckState;
use crate::transport::srspp::machine::receiver::HandleResult;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::machine::receiver::TimerAction;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppDataPacket;
use crate::transport::srspp::packet::SrsppPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::utils::cell::SyncRefCell;
use heapless::LinearMap;

use super::TransportError;
use super::sender::duration_until;

/// Per-stream receiver state for a single remote sender.
pub(super) struct StreamState<R: ReceiverBackend> {
    /// Receiver backend for this stream.
    pub(super) machine: R,
    /// ACK and timer state for this stream.
    pub(super) ack_state: AckState,
    /// Deadline for the delayed ACK timer.
    pub(super) ack_deadline: Option<SysTime>,
    /// Deadline for the progress (inactivity) timer.
    pub(super) progress_deadline: Option<SysTime>,
}

/// Shared mutable state for the multi-stream receiver channel.
pub(super) struct MultiReceiverState<E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    /// Configuration shared across all streams.
    pub(super) config: ReceiverConfig,
    /// Per-sender stream states keyed by source address.
    pub(super) streams: LinearMap<Address, StreamState<R>, MAX_STREAMS>,
    /// Delayed ACK duration.
    pub(super) ack_delay: Duration,
    /// Whether the handle has signaled no more receives.
    pub(super) closed: bool,
    /// First error encountered, propagated to the handle.
    pub(super) error: Option<TransportError<E>>,
}

// ── Channel and driver ──

/// Channel that owns the receiver state. Split into handle + driver.
pub struct SrsppReceiver<
    E,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const MAX_STREAMS: usize = 1,
> {
    /// Interior-mutable receiver state shared between handle and driver.
    state: SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
}

impl<E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize> SrsppReceiver<E, R, MAX_STREAMS> {
    /// Creates a new multi-stream receiver.
    pub fn new(config: ReceiverConfig) -> Self {
        let ack_delay = Duration::from_millis(config.ack_delay_ticks);
        Self {
            state: SyncRefCell::new(MultiReceiverState {
                config,
                streams: LinearMap::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
        }
    }

    /// Splits into a handle for receiving and a driver for I/O.
    pub fn split(
        &self,
    ) -> (
        SrsppRxHandle<'_, E, R, MAX_STREAMS>,
        SrsppReceiverDriver<'_, E, R, MAX_STREAMS>,
    ) {
        (
            SrsppRxHandle {
                receiver: &self.state,
            },
            SrsppReceiverDriver::new(&self.state),
        )
    }
}

/// Driver that handles I/O. Runs as a concurrent task.
pub struct SrsppReceiverDriver<'a, E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    pub(super) state: &'a SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
    ack_buffer: [u8; 32],
}

impl<'a, E, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppReceiverDriver<'a, E, R, MAX_STREAMS>
{
    pub(super) fn new(state: &'a SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>) -> Self {
        Self {
            state,
            ack_buffer: [0u8; 32],
        }
    }
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppReceiverDriver<'a, E, R, MAX_STREAMS>
{
    /// Processes a received data packet and dispatches to the correct stream.
    pub(super) async fn process_data(
        &mut self,
        packet: &[u8],
        link: &mut impl NetworkWrite<Error = E>,
    ) -> Result<(), TransportError<E>> {
        if let Ok(SrsppType::Data) = SrsppPacket::parse(packet).and_then(|p| p.srspp_type()) {
            if let Ok(data) = SrsppDataPacket::parse(packet) {
                let source_address = data.srspp_header.source_address();
                let seq = data.primary.sequence_count();
                let flags = data.primary.sequence_flag();

                let result =
                    self.state
                        .with_mut(|s| -> Result<HandleResult, TransportError<E>> {
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
                                let outcome =
                                    stream.machine.handle_data(seq, flags, &data.payload)?;
                                Ok(stream.ack_state.on_data(
                                    outcome,
                                    stream.machine.expected_seq(),
                                    stream.machine.recv_bitmap(),
                                ))
                            } else {
                                Ok(HandleResult::default())
                            }
                        })?;

                self.drive_actions(source_address, result, link).await?;
            }
        }

        Ok(())
    }

    /// Processes expired ACK and progress timers across all streams.
    pub(super) async fn handle_timeouts(
        &mut self,
        link: &mut impl NetworkWrite<Error = E>,
    ) -> Result<(), TransportError<E>> {
        let now = SysTime::now();

        let ack_expired = self.state.with(|s| {
            s.streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream.ack_deadline.filter(|&d| now >= d).map(|_| *source)
                })
                .collect::<heapless::Vec<_, MAX_STREAMS>>()
        });

        for source in ack_expired {
            let result = self.state.with_mut(|s| {
                if let Some(stream) = s.streams.get_mut(&source) {
                    stream.ack_deadline = None;
                    stream
                        .ack_state
                        .on_ack_timeout(stream.machine.expected_seq(), stream.machine.recv_bitmap())
                } else {
                    HandleResult::default()
                }
            });
            self.drive_actions(source, result, link).await?;
        }

        let progress_expired = self.state.with(|s| {
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
            let result = self
                .state
                .with_mut(|s| -> Result<HandleResult, TransportError<E>> {
                    if let Some(stream) = s.streams.get_mut(&source) {
                        stream.progress_deadline = None;
                        let outcome = stream.machine.skip_gap()?;
                        Ok(stream.ack_state.on_gap_skip(outcome))
                    } else {
                        Ok(HandleResult::default())
                    }
                })?;
            self.drive_actions(source, result, link).await?;
        }

        Ok(())
    }

    /// Returns the earliest receiver deadline (ACK or progress).
    pub(super) fn next_deadline(&self) -> Option<SysTime> {
        self.state.with(|s| {
            s.streams
                .iter()
                .map(|(_, s)| s)
                .flat_map(|s| [s.ack_deadline, s.progress_deadline])
                .flatten()
                .min()
        })
    }

    /// Sends ACK and updates timers based on a state machine result.
    async fn drive_actions(
        &mut self,
        source: Address,
        result: HandleResult,
        link: &mut impl NetworkWrite<Error = E>,
    ) -> Result<(), TransportError<E>> {
        if let Some(AckInfo {
            destination,
            cumulative_ack,
            selective_bitmap,
        }) = result.ack
        {
            let (local_address, apid, function_code) = self.state.with(|s| {
                (
                    s.config.local_address,
                    s.config.apid,
                    s.config.function_code,
                )
            });
            let ack = SrsppAckPacket::builder()
                .buffer(&mut self.ack_buffer)
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

        let ack_delay = self.state.with(|s| s.ack_delay);
        self.state.with_mut(|s| {
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

    /// Run the driver loop.
    pub async fn run<const MTU: usize>(
        &mut self,
        link: &mut (impl NetworkWrite<Error = E> + NetworkRead<Error = E>),
    ) -> Result<(), TransportError<E>> {
        let mut recv_buffer = [0u8; MTU];
        loop {
            if self.state.with(|s| s.closed) {
                return Ok(());
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
                    if let Err(e) = self.process_data(&recv_buffer[..len], link).await {
                        self.state.with_mut(|s| s.error = Some(e.clone()));
                        return Err(e);
                    }
                }
                Some(Err(e)) => {
                    let err = TransportError::Network(e);
                    self.state.with_mut(|s| s.error = Some(err.clone()));
                    return Err(err);
                }
                None => {
                    if let Err(e) = self.handle_timeouts(link).await {
                        self.state.with_mut(|s| s.error = Some(e.clone()));
                        return Err(e);
                    }
                }
            }
        }
    }
}

/// Handle for receiving data from an SRSPP receiver.
pub struct SrsppRxHandle<'a, E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    /// Reference to the shared multi-stream receiver state.
    pub(super) receiver: &'a SyncRefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppRxHandle<'a, E, R, MAX_STREAMS>
{
    /// Receives the next message, copying it into `buf`.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), TransportError<E>> {
        poll_fn(|_cx| {
            self.receiver.with_mut(|s| {
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

    /// Signal that no more receives are expected.
    /// Driver will exit.
    pub fn close(&mut self) {
        self.receiver.with_mut(|s| s.closed = true);
    }

    /// Check if there's a message ready from any sender.
    pub fn has_message(&self) -> bool {
        self.receiver
            .with(|s| s.streams.iter().any(|(_, s)| s.machine.has_message()))
    }

    /// Get the number of active streams.
    pub fn stream_count(&self) -> usize {
        self.receiver.with(|s| s.streams.len())
    }

    /// Wait for a complete message to become available.
    ///
    /// Returns a [`DeliveryToken`] that borrows `&mut self`,
    /// preventing further receives while the token is held.
    /// The driver keeps running — the cell is **not** borrowed
    /// until [`DeliveryToken::consume`] is called.
    pub async fn wait_for_message(
        &mut self,
    ) -> Result<DeliveryToken<'_, 'a, E, R, MAX_STREAMS>, TransportError<E>> {
        let (source, msg_len) = poll_fn(|_cx| {
            self.receiver.with(|s| {
                if let Some(ref e) = s.error {
                    return Poll::Ready(Err(e.clone()));
                }
                for (source, stream) in s.streams.iter() {
                    if let Some(len) = stream.machine.message_len() {
                        return Poll::Ready(Ok((*source, len)));
                    }
                }
                Poll::Pending
            })
        })
        .await?;
        Ok(DeliveryToken {
            rx: self,
            source,
            msg_len,
        })
    }

    /// Wait for a message and process it in-place with a closure.
    ///
    /// Equivalent to `wait_for_message().await?.consume(f)` but
    /// more concise when you don't need the [`DeliveryToken`]
    /// metadata (source address, length).
    pub async fn recv_with<F, Ret>(&mut self, f: F) -> Result<Ret, TransportError<E>>
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        let token = self.wait_for_message().await?;
        Ok(token.consume(f))
    }
}

/// Zero-copy delivery token returned by
/// [`SrsppRxHandle::wait_for_message`].
///
/// Holds `&mut SrsppRxHandle`, preventing another receive while
/// the token is alive.  The cell is **not** borrowed — the
/// driver freely delivers new segments in the background.
///
/// Call [`consume`](Self::consume) with a synchronous closure to
/// read the message and release the token in one step.
pub struct DeliveryToken<'a, 'rx, E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    rx: &'a mut SrsppRxHandle<'rx, E, R, MAX_STREAMS>,
    source: Address,
    msg_len: usize,
}

impl<'a, 'rx, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    DeliveryToken<'a, 'rx, E, R, MAX_STREAMS>
{
    /// Byte length of the pending message.
    pub fn len(&self) -> usize {
        self.msg_len
    }

    /// Source address of the sender that produced this message.
    pub fn source(&self) -> Address {
        self.source
    }

    /// Pass the message data to `f`, consume the token, and
    /// return whatever `f` returns.
    ///
    /// The cell is borrowed only for the duration of `f`.
    pub fn consume<F, Ret>(self, f: F) -> Ret
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        self.rx.receiver.with_mut(|s| {
            let stream = s.streams.get_mut(&self.source).unwrap();
            stream.machine.consume_message(f).unwrap()
        })
    }
}
