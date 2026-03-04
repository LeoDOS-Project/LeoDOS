use core::cell::Ref;
use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use leodos_libcfs::cfe::duration::Duration;
use leodos_libcfs::cfe::time::SysTime;
use leodos_libcfs::runtime::select_either::Either;
use leodos_libcfs::runtime::select_either::select_either;
use leodos_libcfs::runtime::time::sleep;

use crate::network::NetworkLayer;
use crate::network::isl::address::Address;
use crate::network::spp::SequenceCount;
use crate::transport::TransportReceiver;
use crate::transport::srspp::machine::receiver::ReceiverAction;
use crate::transport::srspp::machine::receiver::ReceiverActions;
use crate::transport::srspp::machine::receiver::ReceiverBackend;
use crate::transport::srspp::machine::receiver::ReceiverConfig;
use crate::transport::srspp::machine::receiver::ReceiverEvent;
use crate::transport::srspp::machine::receiver::ReceiverMachine;
use crate::transport::srspp::packet::SrsppAckPacket;
use crate::transport::srspp::packet::SrsppType;
use crate::transport::srspp::packet::parse_data_packet;
use crate::transport::srspp::packet::parse_srspp_type;
use heapless::index_map::FnvIndexMap;

use super::Error;

/// Per-stream receiver state for a single remote sender.
pub(super) struct StreamState<R: ReceiverBackend> {
    /// Receiver state machine for this stream.
    pub(super) machine: R,
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
    pub(super) streams: FnvIndexMap<Address, StreamState<R>, MAX_STREAMS>,
    /// Pending actions produced by stream state machines.
    pub(super) actions: ReceiverActions,
    /// Delayed ACK duration.
    pub(super) ack_delay: Duration,
    /// Whether the handle has signaled no more receives.
    pub(super) closed: bool,
    /// First error encountered, propagated to the handle.
    pub(super) error: Option<Error<E>>,
}

/// Channel that owns the receiver state. Split into handle + driver.
///
/// Supports receiving from multiple senders simultaneously. Each sender is
/// identified by its source address and has independent stream state.
pub struct SrsppReceiver<
    E,
    R: ReceiverBackend = ReceiverMachine<8, 4096, 8192>,
    const MAX_STREAMS: usize = 4,
> {
    /// Interior-mutable receiver state shared between handle and driver.
    state: RefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
}

impl<E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppReceiver<E, R, MAX_STREAMS>
{
    /// Creates a new multi-stream receiver.
    pub fn new(config: ReceiverConfig) -> Self {
        let ack_delay = Duration::from_millis(config.ack_delay_ticks);
        Self {
            state: RefCell::new(MultiReceiverState {
                config,
                streams: FnvIndexMap::new(),
                actions: ReceiverActions::new(),
                ack_delay,
                closed: false,
                error: None,
            }),
        }
    }

    /// Splits into a handle for receiving and a driver for I/O.
    pub fn split<L: NetworkLayer<Error = E>, const MTU: usize>(
        &self,
        link: L,
    ) -> (
        SrsppRxHandle<'_, E, R, MAX_STREAMS>,
        SrsppReceiverDriver<'_, L, R, MTU, MAX_STREAMS>,
    ) {
        (
            SrsppRxHandle { receiver: &self.state },
            SrsppReceiverDriver {
                link,
                channel: self,
                recv_buffer: [0u8; MTU],
                ack_buffer: [0u8; 32],
            },
        )
    }
}

/// Driver that handles I/O. Runs as a concurrent task.
pub struct SrsppReceiverDriver<
    'a,
    L: NetworkLayer,
    R: ReceiverBackend,
    const MTU: usize,
    const MAX_STREAMS: usize,
> {
    /// Network link for sending ACKs and receiving data packets.
    link: L,
    /// Reference to the shared receiver channel.
    channel: &'a SrsppReceiver<L::Error, R, MAX_STREAMS>,
    /// Buffer for receiving data packets from the link.
    recv_buffer: [u8; MTU],
    /// Buffer for building outgoing ACK packets.
    ack_buffer: [u8; 32],
}

impl<
    'a,
    L: NetworkLayer,
    R: ReceiverBackend,
    const MTU: usize,
    const MAX_STREAMS: usize,
> SrsppReceiverDriver<'a, L, R, MTU, MAX_STREAMS>
where
    L::Error: Clone,
{
    /// Run the driver loop.
    pub async fn run(&mut self) -> Result<(), Error<L::Error>> {
        loop {
            if self.channel.state.borrow().closed {
                return Ok(());
            }

            let timeout = self.duration_until_next_timeout();

            match select_either(self.link.recv(&mut self.recv_buffer), sleep(timeout)).await {
                Either::Left(result) => match result {
                    Ok(len) => {
                        if let Err(e) = self.handle_data(len).await {
                            self.channel.state.borrow_mut().error = Some(e.clone());
                            return Err(e);
                        }
                    }
                    Err(e) => {
                        let err = Error::Link(e);
                        self.channel.state.borrow_mut().error = Some(err.clone());
                        return Err(err);
                    }
                },
                Either::Right(()) => {
                    if let Err(e) = self.handle_timeouts().await {
                        self.channel.state.borrow_mut().error = Some(e.clone());
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Computes the duration until the next ACK or progress timeout.
    fn duration_until_next_timeout(&self) -> Duration {
        let now = SysTime::now();
        let state = self.channel.state.borrow();
        state
            .streams
            .values()
            .flat_map(|s| [s.ack_deadline, s.progress_deadline])
            .flatten()
            .min()
            .map(|deadline| {
                if deadline > now {
                    Duration::from(deadline - now)
                } else {
                    Duration::zero()
                }
            })
            .unwrap_or(Duration::from_secs(60))
    }

    /// Processes a received data packet and dispatches to the correct stream.
    async fn handle_data(&mut self, len: usize) -> Result<(), Error<L::Error>> {
        let packet = &self.recv_buffer[..len];

        if let Ok(SrsppType::Data) = parse_srspp_type(packet) {
            if let Ok(data) = parse_data_packet(packet) {
                let source_address = data.srspp_header.source_address();
                let seq = data.primary.sequence_count();
                let flags = data.primary.sequence_flag();

                {
                    let mut state = self.channel.state.borrow_mut();
                    let MultiReceiverState {
                        config,
                        streams,
                        actions,
                        ..
                    } = &mut *state;

                    if !streams.contains_key(&source_address) {
                        let stream_state = StreamState {
                            machine: R::new(config.clone(), source_address),
                            ack_deadline: None,
                            progress_deadline: None,
                        };
                        let _ = streams.insert(source_address, stream_state);
                    }

                    if let Some(stream) = streams.get_mut(&source_address) {
                        stream.machine.handle(
                            ReceiverEvent::DataReceived {
                                seq,
                                flags,
                                payload: &data.payload,
                            },
                            actions,
                        )?;
                    }
                }

                self.process_actions_for_stream(source_address).await?;
            }
        }

        Ok(())
    }

    /// Processes expired ACK and progress timers across all streams.
    async fn handle_timeouts(&mut self) -> Result<(), Error<L::Error>> {
        let now = SysTime::now();

        let ack_expired: heapless::Vec<Address, MAX_STREAMS> = {
            let state = self.channel.state.borrow();
            state
                .streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream.ack_deadline.filter(|&d| now >= d).map(|_| *source)
                })
                .collect()
        };

        for source in ack_expired {
            {
                let mut state = self.channel.state.borrow_mut();
                let MultiReceiverState {
                    streams, actions, ..
                } = &mut *state;
                if let Some(stream) = streams.get_mut(&source) {
                    stream.ack_deadline = None;
                    stream.machine.handle(ReceiverEvent::AckTimeout, actions)?;
                }
            }
            self.process_actions_for_stream(source).await?;
        }

        let progress_expired: heapless::Vec<Address, MAX_STREAMS> = {
            let state = self.channel.state.borrow();
            state
                .streams
                .iter()
                .filter_map(|(source, stream)| {
                    stream
                        .progress_deadline
                        .filter(|&d| now >= d)
                        .map(|_| *source)
                })
                .collect()
        };

        for source in progress_expired {
            {
                let mut state = self.channel.state.borrow_mut();
                let MultiReceiverState {
                    streams, actions, ..
                } = &mut *state;
                if let Some(stream) = streams.get_mut(&source) {
                    stream.progress_deadline = None;
                    stream
                        .machine
                        .handle(ReceiverEvent::ProgressTimeout, actions)?;
                }
            }
            self.process_actions_for_stream(source).await?;
        }

        Ok(())
    }

    /// Sends ACKs and updates timers for the given stream's pending actions.
    async fn process_actions_for_stream(&mut self, source: Address) -> Result<(), Error<L::Error>> {
        let (ack_to_send, timer_actions, ack_delay) = {
            let state = self.channel.state.borrow();
            let ack = state.actions.iter().find_map(|a| match a {
                ReceiverAction::SendAck {
                    destination,
                    cumulative_ack,
                    selective_bitmap,
                } => Some((
                    state.config.local_address,
                    state.config.apid,
                    state.config.function_code,
                    state.config.message_id,
                    state.config.action_code,
                    *destination,
                    *cumulative_ack,
                    *selective_bitmap,
                )),
                _ => None,
            });

            let timer: heapless::Vec<ReceiverAction, 8> = state
                .actions
                .iter()
                .filter(|a| {
                    matches!(
                        a,
                        ReceiverAction::StartAckTimer { .. }
                            | ReceiverAction::StopAckTimer
                            | ReceiverAction::StartProgressTimer { .. }
                            | ReceiverAction::StopProgressTimer
                    )
                })
                .copied()
                .collect();

            (ack, timer, state.ack_delay)
        };

        if let Some((
            local_address,
            apid,
            function_code,
            message_id,
            action_code,
            destination,
            cumulative_ack,
            selective_bitmap,
        )) = ack_to_send
        {
            let ack = SrsppAckPacket::builder()
                .buffer(&mut self.ack_buffer)
                .source_address(local_address)
                .target(destination)
                .apid(apid)
                .function_code(function_code)
                .message_id(message_id)
                .action_code(action_code)
                .cumulative_ack(cumulative_ack)
                .selective_bitmap(selective_bitmap)
                .sequence_count(SequenceCount::from(0))
                .build()?;

            self.link
                .send(zerocopy::IntoBytes::as_bytes(ack))
                .await
                .map_err(Error::Link)?;
        }

        {
            let mut state = self.channel.state.borrow_mut();
            for action in timer_actions {
                match action {
                    ReceiverAction::StartAckTimer { .. } => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.ack_deadline = Some(SysTime::now() + SysTime::from(ack_delay));
                        }
                    }
                    ReceiverAction::StopAckTimer => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.ack_deadline = None;
                        }
                    }
                    ReceiverAction::StartProgressTimer { ticks } => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            let delay = Duration::from_millis(ticks);
                            entry.progress_deadline = Some(SysTime::now() + SysTime::from(delay));
                        }
                    }
                    ReceiverAction::StopProgressTimer => {
                        if let Some(entry) = state.streams.get_mut(&source) {
                            entry.progress_deadline = None;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

/// Handle for receiving data from an SRSPP receiver.
pub struct SrsppRxHandle<'a, E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    /// Reference to the shared multi-stream receiver state.
    pub(super) receiver: &'a RefCell<MultiReceiverState<E, R, MAX_STREAMS>>,
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    SrsppRxHandle<'a, E, R, MAX_STREAMS>
{
    /// Receives the next message, copying it into `buf`.
    pub async fn recv(&mut self, buf: &mut [u8]) -> Result<(Address, usize), Error<E>> {
        poll_fn(|_cx| {
            let mut state = self.receiver.borrow_mut();
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }
            for (source, stream) in state.streams.iter_mut() {
                if let Some(msg) = stream.machine.take_message() {
                    let len = msg.len().min(buf.len());
                    buf[..len].copy_from_slice(&msg[..len]);
                    return Poll::Ready(Ok((*source, len)));
                }
            }
            Poll::Pending
        })
        .await
    }

    /// Receives the next message as a zero-copy reference
    /// into the receiver's reassembly buffer.
    ///
    /// The returned `Ref` holds an immutable borrow on the
    /// receiver state. Drop it before any `.await` that could
    /// let the driver run.
    pub async fn recv_ref(&mut self) -> Result<(Address, Ref<'a, [u8]>), Error<E>> {
        let (source, len) = poll_fn(|_cx| {
            let mut state = self.receiver.borrow_mut();
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }
            for (source, stream) in state.streams.iter_mut() {
                if let Some(msg) = stream.machine.take_message() {
                    let len = msg.len();
                    return Poll::Ready(Ok((*source, len)));
                }
            }
            Poll::Pending
        })
        .await?;

        let data = Ref::map(self.receiver.borrow(), |state| {
            state.streams.get(&source).unwrap().machine.reassembly_data(len)
        });

        Ok((source, data))
    }

    /// Signal that no more receives are expected.
    /// Driver will exit.
    pub fn close(&mut self) {
        self.receiver.borrow_mut().closed = true;
    }

    /// Check if there's a message ready from any sender.
    pub fn has_message(&self) -> bool {
        let state = self.receiver.borrow();
        state.streams.values().any(|s| s.machine.has_message())
    }

    /// Get the number of active streams.
    pub fn stream_count(&self) -> usize {
        self.receiver.borrow().streams.len()
    }

    /// Wait for a complete message to become available.
    ///
    /// Returns a [`DeliveryToken`] that borrows `&mut self`,
    /// preventing further receives while the token is held.
    /// The driver keeps running — the `RefCell` is **not**
    /// borrowed until [`DeliveryToken::consume`] is called.
    pub async fn wait_for_message(
        &mut self,
    ) -> Result<DeliveryToken<'_, 'a, E, R, MAX_STREAMS>, Error<E>> {
        let (source, msg_len) = poll_fn(|_cx| {
            let state = self.receiver.borrow();
            if let Some(ref e) = state.error {
                return Poll::Ready(Err(e.clone()));
            }
            for (source, stream) in state.streams.iter() {
                if let Some(len) = stream.machine.message_len() {
                    return Poll::Ready(Ok((*source, len)));
                }
            }
            Poll::Pending
        })
        .await?;
        Ok(DeliveryToken {
            rx: self,
            source,
            msg_len,
        })
    }
}

/// Zero-copy delivery token returned by
/// [`SrsppRxHandle::wait_for_message`].
///
/// Holds `&mut SrsppRxHandle`, preventing another receive while
/// the token is alive.  The `RefCell` is **not** borrowed — the
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
    /// The `RefCell` is borrowed only for the duration of `f`.
    pub fn consume<F, Ret>(self, f: F) -> Ret
    where
        F: FnOnce(&[u8]) -> Ret,
    {
        let mut state = self.rx.receiver.borrow_mut();
        let stream = state.streams.get_mut(&self.source).unwrap();
        stream.machine.consume_message(f).unwrap()
    }
}

impl<'a, E: Clone, R: ReceiverBackend, const MAX_STREAMS: usize>
    TransportReceiver for SrsppRxHandle<'a, E, R, MAX_STREAMS>
{
    type Error = Error<E>;

    async fn recv(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let (_, len) = self.recv(buf).await?;
        Ok(len)
    }
}
