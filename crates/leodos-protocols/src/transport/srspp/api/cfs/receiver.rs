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

pub(super) struct StreamState<R: ReceiverBackend> {
    pub(super) machine: R,
    pub(super) ack_deadline: Option<SysTime>,
    pub(super) progress_deadline: Option<SysTime>,
}

pub(super) struct MultiReceiverState<E, R: ReceiverBackend, const MAX_STREAMS: usize> {
    pub(super) config: ReceiverConfig,
    pub(super) streams: FnvIndexMap<Address, StreamState<R>, MAX_STREAMS>,
    pub(super) actions: ReceiverActions,
    pub(super) ack_delay: Duration,
    pub(super) closed: bool,
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
    link: L,
    channel: &'a SrsppReceiver<L::Error, R, MAX_STREAMS>,
    recv_buffer: [u8; MTU],
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
